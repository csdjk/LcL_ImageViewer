//! Directory enumeration never blocks the UI/decoder. Keep only the newest queued scan.
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;
use crossbeam_channel::Receiver;
use eframe::egui;
use iv_core::format::has_supported_ext;

pub struct ScanResult {
    pub id: u64,
    pub files: Vec<PathBuf>,
}

#[derive(Default)]
struct State {
    requested: Option<(u64, PathBuf)>,
    stopped: bool,
}

pub struct Scanner {
    state: Arc<(Mutex<State>, Condvar)>,
    results: Receiver<ScanResult>,
}

impl Scanner {
    pub fn spawn(wake: egui::Context) -> Self {
        let state = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let worker = state.clone();
        let (tx, results) = crossbeam_channel::bounded(1);
        std::thread::Builder::new().name("iv-directory".into()).spawn(move || {
            loop {
                let (id, path) = {
                    let (mutex, signal) = &*worker;
                    let mut state = mutex.lock().unwrap();
                    while state.requested.is_none() && !state.stopped {
                        state = signal.wait(state).unwrap();
                    }
                    if state.stopped { break; }
                    state.requested.take().unwrap()
                };
                let started = Instant::now();
                let files = scan_files(&path);
                crate::perf::mark("directory_ready", Some(&path), started.elapsed().as_secs_f64()*1000.0);
                if tx.send(ScanResult { id, files }).is_err() { break; }
                wake.request_repaint();
            }
        }).expect("创建目录扫描线程失败");
        Self { state, results }
    }

    pub fn request(&self, id: u64, path: PathBuf) {
        let (mutex, signal) = &*self.state;
        mutex.lock().unwrap().requested = Some((id, path));
        signal.notify_one();
    }

    pub fn poll(&self) -> Option<ScanResult> { self.results.try_recv().ok() }
}

impl Drop for Scanner {
    fn drop(&mut self) {
        let (mutex, signal) = &*self.state;
        mutex.lock().unwrap().stopped = true;
        signal.notify_all();
        // Do not join a thread doing filesystem I/O on the UI thread.
    }
}

pub fn scan_files(path: &Path) -> Vec<PathBuf> {
    let Some(parent) = path.parent() else { return Vec::new(); };
    let Ok(entries) = std::fs::read_dir(parent) else { return Vec::new(); };
    let mut files: Vec<PathBuf> = entries.filter_map(Result::ok).filter_map(|entry| {
        let path = entry.path();
        // Reject unrelated extensions before any filesystem metadata request.
        if !has_supported_ext(&path) { return None; }
        let file_type = entry.file_type().ok()?;
        if file_type.is_file() || (file_type.is_symlink() && path.is_file()) { Some(path) } else { None }
    }).collect();
    // Each case-folded filename is computed once, rather than for every comparison.
    files.sort_by_cached_key(|p| p.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_lowercase());
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scan_filters_and_sorts_without_recursing() {
        let folder = std::env::temp_dir().join(format!("iv-dir-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&folder).unwrap();
        for name in ["B.png", "a.jpg", "中.dds", "notes.txt"] { std::fs::write(folder.join(name), []).unwrap(); }
        std::fs::create_dir(folder.join("fake.png")).unwrap();
        let actual: Vec<_> = scan_files(&folder.join("B.png")).iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(actual, ["a.jpg", "B.png", "中.dds"]);
        std::fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn inaccessible_directory_is_an_empty_list() {
        assert!(scan_files(Path::new("__iv_missing_directory__/none.png")).is_empty());
    }
}
