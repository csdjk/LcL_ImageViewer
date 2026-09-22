//! Observe only the opened file. Unchanged metadata never wakes the UI.
//! Polling supports atomic replacements without a directory-wide watcher dependency.
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime};

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const SETTLE_TIME: Duration = Duration::from_millis(300);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileState {
    Present {
        len: u64,
        modified: SystemTime,
        created: Option<SystemTime>,
    },
    Unavailable(String),
}

impl FileState {
    pub(crate) fn read(path: &Path) -> Self {
        let metadata = (|| {
            let meta = std::fs::metadata(path)?;
            if !meta.is_file() {
                return Err(std::io::Error::other("路径已不再是文件"));
            }
            Ok(Self::Present {
                len: meta.len(),
                modified: meta.modified()?,
                created: meta.created().ok(),
            })
        })();
        metadata.unwrap_or_else(|error: std::io::Error| Self::Unavailable(error.to_string()))
    }
}

pub struct Change {
    pub path: PathBuf,
    pub state: FileState,
}

struct Target {
    path: PathBuf,
    baseline: FileState,
    candidate: Option<(FileState, Instant)>,
}

impl Target {
    fn observe(&mut self, state: FileState, now: Instant) -> Option<Change> {
        if state == self.baseline {
            self.candidate = None;
            return None;
        }
        match &self.candidate {
            Some((previous, since)) if *previous == state => {
                if now.duration_since(*since) < SETTLE_TIME {
                    return None;
                }
                self.baseline = state.clone();
                self.candidate = None;
                Some(Change {
                    path: self.path.clone(),
                    state,
                })
            }
            _ => {
                self.candidate = Some((state, now));
                None
            }
        }
    }
}

#[derive(Default)]
struct State {
    revision: u64,
    target: Option<Target>,
    pending: Option<Change>,
    stopped: bool,
}

pub struct FileWatcher {
    shared: Arc<(Mutex<State>, Condvar)>,
}

impl FileWatcher {
    pub fn spawn(wake: eframe::egui::Context) -> Self {
        let shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("iv-file-watch".into())
            .spawn(move || {
                let (mutex, signal) = &*worker;
                loop {
                    let (revision, path) = {
                        let mut state = mutex.lock().unwrap();
                        while !state.stopped && state.target.is_none() {
                            state = signal.wait(state).unwrap();
                        }
                        if state.stopped {
                            break;
                        }
                        let (state, timeout) = signal.wait_timeout(state, POLL_INTERVAL).unwrap();
                        if state.stopped {
                            break;
                        }
                        if !timeout.timed_out() {
                            continue;
                        }
                        let Some(target) = &state.target else {
                            continue;
                        };
                        (state.revision, target.path.clone())
                    };
                    // File I/O does not hold the UI-facing mutex.
                    let observed = FileState::read(&path);
                    let mut state = mutex.lock().unwrap();
                    if state.stopped {
                        break;
                    }
                    if state.revision != revision {
                        continue;
                    }
                    if let Some(change) = state
                        .target
                        .as_mut()
                        .and_then(|target| target.observe(observed, Instant::now()))
                    {
                        // A single latest observation, never an unbounded event queue.
                        state.pending = Some(change);
                        drop(state);
                        wake.request_repaint();
                    }
                }
            })
            .expect("创建文件检查线程失败");
        Self { shared }
    }

    pub fn set_path(&self, path: Option<PathBuf>) {
        // A single initial stat at open/toggle, not a recurring UI-thread check.
        let target = path.map(|path| Target {
            baseline: FileState::read(&path),
            path,
            candidate: None,
        });
        let mut state = self.shared.0.lock().unwrap();
        state.revision += 1;
        state.target = target;
        state.pending = None;
        self.shared.1.notify_one();
    }

    pub fn poll(&self) -> Option<Change> {
        self.shared.0.lock().unwrap().pending.take()
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        self.shared.0.lock().unwrap().stopped = true;
        self.shared.1.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp(len: u64) -> FileState {
        FileState::Present {
            len,
            modified: SystemTime::UNIX_EPOCH,
            created: None,
        }
    }

    #[test]
    fn unchanged_and_burst_saves_only_deliver_the_settled_latest_version() {
        let mut target = Target {
            path: "image.png".into(),
            baseline: stamp(1),
            candidate: None,
        };
        let now = Instant::now();
        assert!(target.observe(stamp(1), now).is_none());
        assert!(target.observe(stamp(2), now).is_none());
        assert!(target.observe(stamp(3), now + SETTLE_TIME).is_none());
        assert_eq!(
            target
                .observe(stamp(3), now + SETTLE_TIME * 2)
                .unwrap()
                .state,
            stamp(3)
        );
        assert!(target.observe(stamp(3), now + SETTLE_TIME * 3).is_none());
    }

    #[test]
    fn disappearance_is_reported_and_reappearance_recovers() {
        let mut target = Target {
            path: "image.png".into(),
            baseline: stamp(1),
            candidate: None,
        };
        let now = Instant::now();
        let missing = FileState::Unavailable("missing".into());
        assert!(target.observe(missing.clone(), now).is_none());
        assert_eq!(
            target
                .observe(missing.clone(), now + SETTLE_TIME)
                .unwrap()
                .state,
            missing
        );
        assert!(target.observe(stamp(1), now + SETTLE_TIME * 2).is_none());
        assert_eq!(
            target
                .observe(stamp(1), now + SETTLE_TIME * 3)
                .unwrap()
                .state,
            stamp(1)
        );
    }

    #[test]
    fn changing_target_or_disabling_discards_old_pending_events() {
        let watcher = FileWatcher::spawn(eframe::egui::Context::default());
        watcher.shared.0.lock().unwrap().pending = Some(Change {
            path: "old".into(),
            state: stamp(1),
        });
        watcher.set_path(Some("new".into()));
        assert!(watcher.poll().is_none());
        watcher.set_path(None);
        assert!(watcher.shared.0.lock().unwrap().target.is_none());
    }
}
