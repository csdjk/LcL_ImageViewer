//! Bounded, latest-request-first decoder. Submitting a request never waits for decoding.
//! One decode is allowed to finish; queued obsolete loads/prefetches are discarded.
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;
use crossbeam_channel::Receiver;
use eframe::egui;
use iv_core::decode::{decode_path, DecodeError, DecodedImage};

pub enum Msg {
    Ready(Result<(PathBuf, Arc<DecodedImage>, crate::file_watch::FileState), (PathBuf, String)>),
    Outdated(PathBuf),
}

impl Msg {
    fn path(&self) -> &Path {
        match self {
            Self::Ready(Ok((p, _, _))) | Self::Ready(Err((p, _))) | Self::Outdated(p) => p,
        }
    }
}

#[derive(Default)]
struct Requests {
    generation: u64,
    foreground: Option<PathBuf>,
    prefetch: VecDeque<PathBuf>,
    inflight: Option<PathBuf>,
    undelivered: HashSet<PathBuf>,
    stopped: bool,
}

impl Requests {
    fn invalidate(&mut self) {
        self.generation += 1;
        self.foreground = None;
        self.prefetch.clear();
        self.inflight = None;
        self.undelivered.clear();
    }

    fn load(&mut self, path: PathBuf) {
        self.prefetch.clear();
        self.foreground = if self.inflight.as_ref() == Some(&path) || self.undelivered.contains(&path) {
            None
        } else { Some(path) };
    }

    fn prefetch(&mut self, paths: impl IntoIterator<Item=PathBuf>) {
        self.prefetch.clear();
        for path in paths {
            if self.foreground.as_ref() != Some(&path) && self.inflight.as_ref() != Some(&path)
                && !self.undelivered.contains(&path) && !self.prefetch.contains(&path) {
                self.prefetch.push_back(path);
                if self.prefetch.len() == 4 { break; }
            }
        }
    }

    fn next(&mut self) -> Option<PathBuf> {
        let path = self.foreground.take().or_else(|| self.prefetch.pop_front())?;
        self.inflight = Some(path.clone());
        Some(path)
    }
}

pub struct Loader {
    requests: Arc<(Mutex<Requests>, Condvar)>,
    wake: Arc<Mutex<Option<egui::Context>>>,
    results: Receiver<(u64, crate::file_watch::FileState, Msg)>,
}

impl Loader {
    pub fn spawn() -> Self { Self::with_decoder(decode_path) }

    fn with_decoder(decode: impl Fn(&Path) -> Result<DecodedImage, DecodeError> + Send + 'static) -> Self {
        let requests = Arc::new((Mutex::new(Requests::default()), Condvar::new()));
        let worker = requests.clone();
        let wake = Arc::new(Mutex::new(None::<egui::Context>));
        let wake_worker = wake.clone();
        // Only two decoded results may wait for the UI, not 64 full-resolution images.
        let (tx, results) = crossbeam_channel::bounded(2);
        std::thread::Builder::new().name("iv-loader".into()).spawn(move || {
            loop {
                let (generation, path) = {
                    let (mutex, signal) = &*worker;
                    let mut state = mutex.lock().unwrap();
                    while !state.stopped && state.foreground.is_none() && state.prefetch.is_empty() {
                        state = signal.wait(state).unwrap();
                    }
                    if state.stopped { break; }
                    (state.generation, state.next().unwrap())
                };
                let started = Instant::now();
                let source_version = crate::file_watch::FileState::read(&path);
                crate::perf::mark("decode_start", Some(&path), 0.0);
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| decode(&path)))
                    .unwrap_or_else(|_| Err(DecodeError::Decode("解码器异常，请检查图片文件".into())))
                    .map_err(|e| (path.clone(), e.to_string()));
                crate::perf::mark("decode_ready", Some(&path), started.elapsed().as_secs_f64()*1000.0);
                {
                    let mut state = worker.0.lock().unwrap();
                    if state.stopped { break; }
                    if state.generation != generation { continue; }
                    state.inflight = None;
                    state.undelivered.insert(path.clone());
                }
                let ready = result.map(|img| (path, Arc::new(img), source_version.clone()));
                if tx.send((generation, source_version, Msg::Ready(ready))).is_err() { break; }
                if let Some(ctx) = &*wake_worker.lock().unwrap() { ctx.request_repaint(); }
            }
        }).expect("创建解码线程失败");
        Self { requests, wake, results }
    }

    pub fn set_waker(&self, ctx: egui::Context) { *self.wake.lock().unwrap() = Some(ctx); }

    pub fn load(&self, path: PathBuf) {
        self.requests.0.lock().unwrap().load(path);
        self.requests.1.notify_one();
    }

    pub fn prefetch(&self, paths: impl IntoIterator<Item=PathBuf>) {
        self.requests.0.lock().unwrap().prefetch(paths);
        self.requests.1.notify_one();
    }

    /// A new version of the same path must not reuse in-flight/queued old pixels.
    pub fn reload(&self, path: PathBuf) {
        let mut state = self.requests.0.lock().unwrap();
        state.invalidate();
        state.load(path);
        self.requests.1.notify_one();
    }

    pub fn cancel_queued(&self) {
        let mut state = self.requests.0.lock().unwrap();
        state.invalidate();
    }

    pub fn poll(&self) -> Option<Msg> {
        while let Ok((generation, source_version, msg)) = self.results.try_recv() {
            let mut state = self.requests.0.lock().unwrap();
            if generation != state.generation { continue; }
            state.undelivered.remove(msg.path());
            drop(state);
            // A -> B -> edited A may reuse in-flight A. Validate before delivery,
            // including results already queued while another file was displayed.
            if source_version != crate::file_watch::FileState::read(msg.path()) {
                return Some(Msg::Outdated(msg.path().to_path_buf()));
            }
            return Some(msg);
        }
        None
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        self.requests.0.lock().unwrap().stopped = true;
        self.requests.1.notify_all();
        // No blocking join on UI exit; dropping results disconnects a blocked sender.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(name: &str) -> PathBuf { name.into() }

    #[test]
    fn newest_load_replaces_old_requests_and_prefetch() {
        let mut q = Requests::default();
        q.prefetch([p("old-a"), p("old-b")]);
        for i in 0..1000 { q.load(p(&i.to_string())); }
        assert_eq!(q.next(), Some(p("999")));
        assert!(q.next().is_none());
    }
    #[test]
    fn foreground_has_priority_and_prefetch_is_bounded_and_deduplicated() {
        let mut q=Requests::default();
        q.load(p("current"));
        q.prefetch(["current","a","a","b","c","d","e"].map(p));
        assert_eq!(q.prefetch.len(),4);
        assert_eq!(q.next(),Some(p("current")));
        assert_eq!(q.next(),Some(p("a")));
    }
    #[test]
    fn inflight_and_undelivered_images_are_not_decoded_twice() {
        let mut q=Requests::default();
        q.inflight=Some(p("image")); q.load(p("image"));
        assert!(q.foreground.is_none());
        q.undelivered.insert(p("ready")); q.load(p("ready"));
        q.prefetch([p("image"),p("ready")]);
        assert!(q.next().is_none());
    }
    #[test]
    fn worker_finishes_current_work_then_loads_latest_not_backlog() {
        use std::time::Duration;
        let (started_tx,started_rx)=crossbeam_channel::unbounded();
        let (release_tx,release_rx)=crossbeam_channel::bounded(1);
        let loader=Loader::with_decoder(move |path| {
            started_tx.send(path.to_path_buf()).unwrap();
            if path==Path::new("first") { release_rx.recv_timeout(Duration::from_secs(3)).unwrap(); }
            Err(DecodeError::Truncated)
        });
        loader.load(p("first"));
        assert_eq!(started_rx.recv_timeout(Duration::from_secs(3)).unwrap(),p("first"));
        loader.prefetch([p("background-a"),p("background-b")]);
        loader.load(p("obsolete")); loader.load(p("last"));
        release_tx.send(()).unwrap();
        assert_eq!(started_rx.recv_timeout(Duration::from_secs(3)).unwrap(),p("last"));
        assert!(started_rx.recv_timeout(Duration::from_millis(30)).is_err());
    }

    #[test]
    fn reload_same_path_discards_inflight_and_queued_old_versions() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Duration;
        let (started_tx, started_rx) = crossbeam_channel::unbounded();
        let (release_tx, release_rx) = crossbeam_channel::bounded(1);
        let sequence = AtomicUsize::new(0);
        let loader = Loader::with_decoder(move |_| {
            let n = sequence.fetch_add(1, Ordering::SeqCst);
            started_tx.send(n).unwrap();
            if n == 0 { release_rx.recv_timeout(Duration::from_secs(3)).unwrap(); }
            Err(DecodeError::Decode(format!("version-{n}")))
        });
        loader.load(p("same"));
        assert_eq!(started_rx.recv_timeout(Duration::from_secs(3)).unwrap(), 0);
        loader.reload(p("same"));
        release_tx.send(()).unwrap();
        assert_eq!(started_rx.recv_timeout(Duration::from_secs(3)).unwrap(), 1);
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(Msg::Ready(Err((path, error)))) = loader.poll() {
                assert_eq!(path, p("same"));
                assert!(error.contains("version-1"));
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        loader.load(p("queued"));
        assert_eq!(started_rx.recv_timeout(Duration::from_secs(3)).unwrap(), 2);
        while loader.requests.0.lock().unwrap().undelivered.is_empty() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        loader.reload(p("queued"));
        assert_eq!(started_rx.recv_timeout(Duration::from_secs(3)).unwrap(), 3);
        loop {
            if let Some(Msg::Ready(Err((_, error)))) = loader.poll() {
                assert!(error.contains("version-3"));
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn navigating_back_to_an_edited_inflight_file_never_delivers_old_pixels() {
        use std::time::{Duration, SystemTime};
        let path = std::env::temp_dir().join(format!("iv-source-version-{}-{}",
            std::process::id(), SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::write(&path, b"v1").unwrap();
        let (started_tx, started_rx) = crossbeam_channel::unbounded();
        let (release_tx, release_rx) = crossbeam_channel::bounded(1);
        let decoder_path = path.clone();
        let loader = Loader::with_decoder(move |p| {
            started_tx.send(p.to_path_buf()).unwrap();
            if p == decoder_path { release_rx.recv_timeout(Duration::from_secs(3)).unwrap(); }
            Err(DecodeError::Truncated)
        });
        loader.load(path.clone());
        assert_eq!(started_rx.recv_timeout(Duration::from_secs(3)).unwrap(), path);
        loader.load(p("other"));
        std::fs::write(&path, b"version-2").unwrap();
        loader.load(path.clone());
        release_tx.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(message) = loader.poll() {
                assert!(matches!(message, Msg::Outdated(ref p) if *p == path));
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        std::fs::remove_file(path).unwrap();
    }
}
