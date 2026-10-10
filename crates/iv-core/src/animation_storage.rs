//! Lossless animation storage. Small sequences stay in RAM; larger ones spill to
//! an exclusively owned, delete-on-close temporary file and become read-only maps.
//! Mapping is lazy: reading/uploading one frame does not allocate every frame again.
use crate::decode::{AnimatedFrame, DecodeError, PixelData};
use memmap2::Mmap;
use std::{
    fmt,
    fs::{File, OpenOptions},
    io::Write,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) const MEMORY_BUDGET: usize = 64 * 1024 * 1024;
pub(crate) const MAX_FRAME_BYTES: usize = 256 * 1024 * 1024;
#[cfg(target_pointer_width = "64")]
pub(crate) const STORAGE_BUDGET: usize = 8 * 1024 * 1024 * 1024;
#[cfg(not(target_pointer_width = "64"))]
pub(crate) const STORAGE_BUDGET: usize = 1024 * 1024 * 1024;
pub(crate) const MAX_FRAMES: usize = 4096;
const PROCESS_DISK_BUDGET: u64 = 16 * 1024 * 1024 * 1024;
static DISK_BYTES: AtomicU64 = AtomicU64::new(0);
static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn check_storage(
    frame_bytes: usize,
    count: usize,
    limit: usize,
) -> Result<(), DecodeError> {
    if count == 0 || count > MAX_FRAMES {
        return Err(DecodeError::ResourceLimit(
            "动画帧数超过安全限制（最多 4096 帧）".into(),
        ));
    }
    if frame_bytes == 0 || frame_bytes > MAX_FRAME_BYTES {
        return Err(DecodeError::ResourceLimit(
            "动画单帧像素数据超过 256 MiB 安全限制".into(),
        ));
    }
    if frame_bytes.checked_mul(count).is_none_or(|n| n > limit) {
        return Err(DecodeError::ResourceLimit(format!(
            "动画展开数据超过 {:.2} GiB 缓存安全上限",
            limit as f64 / (1024.0 * 1024.0 * 1024.0)
        )));
    }
    Ok(())
}

// There is never an exposed writable file handle after publication of the map.
// Map drops BEFORE its file owner, so Windows deletes the cache after unmapping.
struct MappedStore {
    map: Mmap,
    _owner: Spool,
}

/// Immutable frame-sized slice of a private temporary animation cache.
/// Clones retain ownership, not a second copy of the pixels.
#[derive(Clone)]
pub struct MappedPixels {
    store: Arc<MappedStore>,
    offset: usize,
    len: usize,
}
impl MappedPixels {
    pub fn as_slice(&self) -> &[u8] {
        &self.store.map[self.offset..self.offset + self.len]
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
impl fmt::Debug for MappedPixels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MappedPixels")
            .field("offset", &self.offset)
            .field("len", &self.len)
            .finish()
    }
}

struct Spool {
    file: Option<File>,
    bytes: u64,
}
impl Spool {
    fn new() -> Result<Self, DecodeError> {
        let dir = std::env::temp_dir();
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for _ in 0..64 {
            let path = dir.join(format!(
                "lcl-iv-animation-{}-{epoch}-{}.tmp",
                std::process::id(),
                NEXT_FILE.fetch_add(1, Ordering::Relaxed)
            ));
            let mut options = OpenOptions::new();
            options.read(true).write(true).create_new(true);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                // No sharing: nobody can open/modify/truncate the mapped backing file.
                // DELETE_ON_CLOSE also cleans it on process termination.
                options
                    .share_mode(0)
                    .custom_flags(0x0400_0000 | 0x0000_0100);
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(file) => {
                    #[cfg(unix)]
                    {
                        // Remove the name before writing/mapping. Only this handle survives.
                        std::fs::remove_file(&path).map_err(cache_io)?;
                    }
                    return Ok(Self {
                        file: Some(file),
                        bytes: 0,
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(cache_io(e)),
            }
        }
        Err(DecodeError::Io(
            "无法创建私有动画缓存，请检查系统临时目录权限".into(),
        ))
    }
    fn write(&mut self, pixels: &[u8]) -> Result<(), DecodeError> {
        let len = pixels.len() as u64;
        DISK_BYTES
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                n.checked_add(len).filter(|&v| v <= PROCESS_DISK_BUDGET)
            })
            .map_err(|_| {
                DecodeError::ResourceLimit(
                    "当前动画临时缓存总量超过 16 GiB，请先关闭其他大动画或重新打开查看器".into(),
                )
            })?;
        self.bytes += len; // Release on every error/drop path, even a partially failed write.
        self.file
            .as_mut()
            .expect("unsealed spool")
            .write_all(pixels)
            .map_err(cache_io)
    }
    fn seal(mut self) -> Result<Arc<MappedStore>, DecodeError> {
        let file = self.file.as_mut().expect("unsealed spool");
        file.flush().map_err(cache_io)?;
        // SAFETY: exclusively created private file; Windows denies sharing and uses
        // delete-on-close, Unix unlinks immediately. No mutable mapping/handle escapes,
        // no writes/truncation occur after this point. Map and owner share one lifetime.
        let map = unsafe { Mmap::map(&*file) }.map_err(cache_io)?;
        Ok(Arc::new(MappedStore { map, _owner: self }))
    }
}
impl Drop for Spool {
    fn drop(&mut self) {
        drop(self.file.take());
        DISK_BYTES.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}
fn cache_io(e: std::io::Error) -> DecodeError {
    DecodeError::Io(format!(
        "动画临时缓存不可用：{e}。请检查系统临时目录的空间和权限，原图未修改"
    ))
}

pub(crate) struct AnimationBuilder {
    memory_limit: usize,
    storage_limit: usize,
    frame_bytes: Option<usize>,
    delays: Vec<u32>,
    memory: Vec<AnimatedFrame>,
    spool: Option<Spool>,
}
impl AnimationBuilder {
    pub(crate) fn new(memory_limit: usize) -> Self {
        Self {
            memory_limit,
            storage_limit: STORAGE_BUDGET,
            frame_bytes: None,
            delays: Vec::new(),
            memory: Vec::new(),
            spool: None,
        }
    }
    pub(crate) fn push(&mut self, pixels: Vec<u8>, delay_ms: u32) -> Result<(), DecodeError> {
        let count = self.delays.len() + 1;
        check_storage(pixels.len(), count, self.storage_limit)?;
        if self.frame_bytes.is_some_and(|n| n != pixels.len()) {
            return Err(DecodeError::Decode("动画帧像素长度不一致".into()));
        }
        self.frame_bytes = Some(pixels.len());
        // Include the legacy first-mip copy for small in-memory sequences.
        let in_memory = pixels
            .len()
            .checked_mul(count + 1)
            .is_some_and(|n| n <= self.memory_limit);
        if self.spool.is_none() && !in_memory {
            let mut spool = Spool::new()?;
            for old in self.memory.drain(..) {
                spool.write(old.data.as_rgba8().expect("animation RGBA8"))?;
            }
            self.memory.shrink_to_fit();
            self.spool = Some(spool);
        }
        if let Some(spool) = &mut self.spool {
            spool.write(&pixels)?;
        } else {
            self.memory.push(AnimatedFrame {
                data: PixelData::Rgba8(pixels),
                delay_ms,
            });
        }
        self.delays.push(delay_ms);
        Ok(())
    }
    pub(crate) fn finish(self) -> Result<Vec<AnimatedFrame>, DecodeError> {
        if self.delays.is_empty() {
            return Err(DecodeError::Decode("动画无有效帧".into()));
        }
        if let Some(spool) = self.spool {
            let store = spool.seal()?;
            let len = self.frame_bytes.expect("validated frame length");
            Ok(self
                .delays
                .into_iter()
                .enumerate()
                .map(|(index, delay_ms)| AnimatedFrame {
                    data: PixelData::Rgba8Mapped(MappedPixels {
                        store: store.clone(),
                        offset: index * len,
                        len,
                    }),
                    delay_ms,
                })
                .collect())
        } else {
            Ok(self.memory)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spill_keeps_all_frames_bytes_delays_and_reverse_access() {
        let mut b = AnimationBuilder::new(48);
        for i in 0..10 {
            b.push(vec![i; 16], 10 + u32::from(i)).unwrap();
        }
        let frames = b.finish().unwrap();
        assert_eq!(frames.len(), 10);
        for i in (0..10).rev() {
            assert!(matches!(frames[i].data, PixelData::Rgba8Mapped(_)));
            assert_eq!(frames[i].data.as_rgba8().unwrap(), &[i as u8; 16]);
            assert_eq!(frames[i].delay_ms, 10 + i as u32);
        }
    }
    #[test]
    fn small_frames_stay_in_memory_and_threshold_is_inclusive() {
        let mut b = AnimationBuilder::new(48);
        b.push(vec![1; 16], 40).unwrap();
        b.push(vec![2; 16], 50).unwrap();
        assert!(b
            .finish()
            .unwrap()
            .iter()
            .all(|f| matches!(f.data, PixelData::Rgba8(_))));
        let mut b = AnimationBuilder::new(47);
        b.push(vec![1; 16], 40).unwrap();
        b.push(vec![2; 16], 50).unwrap();
        assert!(b
            .finish()
            .unwrap()
            .iter()
            .all(|f| matches!(f.data, PixelData::Rgba8Mapped(_))));
    }
    #[test]
    fn clones_keep_mapping_alive_until_the_last_consumer_drops() {
        let mut b = AnimationBuilder::new(0);
        b.push(vec![9, 8, 7, 6], 40).unwrap();
        let frames = b.finish().unwrap();
        let PixelData::Rgba8Mapped(p) = &frames[0].data else {
            panic!()
        };
        let weak = Arc::downgrade(&p.store);
        let copy = frames[0].data.clone();
        drop(frames);
        assert!(weak.upgrade().is_some());
        assert_eq!(copy.rgba8_at(1, 1, 0, 0), Some([9, 8, 7, 6]));
        assert_eq!(copy.rgba_f32_at(1, 1, 0, 0).unwrap()[3], 6.0 / 255.0);
        drop(copy);
        assert!(weak.upgrade().is_none());
    }
    #[test]
    fn malformed_or_excessive_input_still_fails_without_publishing_partial_frames() {
        let mut b = AnimationBuilder::new(0);
        b.storage_limit = 7;
        b.push(vec![0; 4], 1).unwrap();
        assert!(b.push(vec![0; 4], 1).is_err());
        let mut b = AnimationBuilder::new(0);
        b.push(vec![0; 4], 1).unwrap();
        assert!(b.push(vec![0; 8], 1).is_err());
        assert!(check_storage(usize::MAX, 2, usize::MAX).is_err());
        assert!(check_storage(4, MAX_FRAMES + 1, STORAGE_BUDGET).is_err());
        assert!(check_storage(4, 0, STORAGE_BUDGET).is_err());
    }
}
