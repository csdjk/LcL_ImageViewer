//! 有界、可取消的后台目录索引。只记录路径，不解码图片；绝不向父目录扩展。
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use eframe::egui;
use iv_core::format::has_supported_ext;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    pub root: PathBuf,
    pub recursive: bool,
}

impl Scope {
    /// 只在显式打开或用户切换模式时调用；普通导航保留已有 Scope。
    pub fn for_image(path: &Path, recursive: bool) -> Option<Self> {
        Some(Self { root: path.parent()?.to_path_buf(), recursive })
    }

    pub fn contains(&self, path: &Path) -> bool {
        path.starts_with(&self.root) && path != self.root
            && !path.components().any(|p| matches!(p, std::path::Component::ParentDir))
            && (self.recursive || path.parent() == Some(self.root.as_path()))
    }
}

#[derive(Clone, Debug, Default)]
pub struct ScanStats {
    pub files: usize,
    pub entries: usize,
    pub directories: usize,
    pub skipped_links: usize,
    pub skipped_errors: usize,
    pub skipped_depth: usize,
    pub limit: Option<&'static str>,
    pub elapsed_ms: u64,
}

impl ScanStats {
    pub fn partial(&self) -> bool {
        self.limit.is_some() || self.skipped_errors > 0 || self.skipped_depth > 0
    }
    pub fn description(&self) -> String {
        let mut text = format!("已找到 {} 张图片，检查 {} 个目录", self.files, self.directories);
        if let Some(limit) = self.limit {
            text.push_str(&format!("\n达到{limit}，当前仅包含部分结果；请打开更小的子文件夹继续浏览。"));
        }
        if self.skipped_links > 0 { text.push_str(&format!("\n已跳过 {} 个链接或重解析点，防止越界和循环。", self.skipped_links)); }
        if self.skipped_errors > 0 { text.push_str(&format!("\n{} 个项目不可访问或已被移走。", self.skipped_errors)); }
        if self.skipped_depth > 0 { text.push_str(&format!("\n已跳过 {} 个过深的子目录。", self.skipped_depth)); }
        text
    }
}

pub struct ScanResult {
    pub id: u64,
    /// None 是限频进度，Some 是最终列表；邮箱最多保存一个结果，不累积快照。
    pub files: Option<Vec<PathBuf>>,
    pub stats: ScanStats,
}

#[derive(Clone, Copy)]
struct Limits {
    files: usize,
    index_bytes: usize,
    entries: usize,
    directories: usize,
    depth: usize,
    elapsed: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self { files: 50_000, index_bytes: 32 * 1024 * 1024, entries: 250_000,
            directories: 10_000, depth: 64, elapsed: Duration::from_secs(15) }
    }
}

struct Job { id: u64, scope: Scope, anchor: Option<PathBuf> }
#[derive(Default)]
struct State { requested: Option<Job>, result: Option<ScanResult>, stopped: bool }
struct Shared { state: Mutex<State>, signal: Condvar, generation: AtomicU64 }
pub struct Scanner { shared: Arc<Shared> }

impl Scanner {
    pub fn spawn(wake: egui::Context) -> Self {
        let shared = Arc::new(Shared { state: Mutex::new(State::default()),
            signal: Condvar::new(), generation: AtomicU64::new(0) });
        let worker = shared.clone();
        std::thread::Builder::new().name("iv-directory".into()).spawn(move || loop {
            let job = {
                let mut state = worker.state.lock().unwrap();
                while state.requested.is_none() && !state.stopped { state = worker.signal.wait(state).unwrap(); }
                if state.stopped { break; }
                state.requested.take().unwrap()
            };
            let cancelled = || worker.generation.load(Ordering::Acquire) != job.id;
            let publish = |files, stats| {
                let mut state = worker.state.lock().unwrap();
                if !state.stopped && !cancelled() {
                    state.result = Some(ScanResult { id: job.id, files, stats });
                    wake.request_repaint();
                }
            };
            if let Some((files, stats)) = scan(&job.scope, job.anchor.as_deref(), Limits::default(),
                cancelled, |stats| publish(None, stats)) {
                crate::perf::mark("directory_ready", job.anchor.as_deref(), stats.elapsed_ms as f64);
                publish(Some(files), stats);
            }
        }).expect("创建目录扫描线程失败");
        Self { shared }
    }

    pub fn request(&self, id: u64, scope: Scope, anchor: Option<PathBuf>) {
        self.shared.generation.store(id, Ordering::Release);
        let mut state = self.shared.state.lock().unwrap();
        state.result = None;
        state.requested = Some(Job { id, scope, anchor });
        self.shared.signal.notify_one();
    }
    pub fn cancel(&self, id: u64) {
        self.shared.generation.store(id, Ordering::Release);
        let mut state = self.shared.state.lock().unwrap();
        state.requested = None;
        state.result = None;
    }
    pub fn poll(&self) -> Option<ScanResult> { self.shared.state.lock().unwrap().result.take() }
}
impl Drop for Scanner {
    fn drop(&mut self) {
        self.shared.generation.fetch_add(1, Ordering::Release);
        self.shared.state.lock().unwrap().stopped = true;
        self.shared.signal.notify_all();
        // 不在界面线程等待可能被磁盘/网络 I/O 阻塞的线程。
    }
}

fn is_link(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() { return true; }
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_REPARSE_POINT：还涵盖目录 junction 和挂载点。
        if metadata.file_attributes() & 0x400 != 0 { return true; }
    }
    false
}

/// 对路径、Vec 槽位及排序临时键做保守记账，并非操作系统进程总内存上限。
fn path_cost(path: &Path) -> usize { path.as_os_str().len().saturating_mul(4).saturating_add(128) }

fn scan(scope: &Scope, anchor: Option<&Path>, limits: Limits,
    cancelled: impl Fn() -> bool, mut progress: impl FnMut(ScanStats)) -> Option<(Vec<PathBuf>, ScanStats)> {
    let started = Instant::now();
    let mut stats = ScanStats::default();
    let mut files = Vec::new();
    let mut charged = 0usize;
    if cancelled() { return None; }
    let canonical_root = match std::fs::canonicalize(&scope.root) {
        Ok(path) => path,
        Err(_) => { stats.skipped_errors += 1; return Some((files, stats)); }
    };
    // 保留用户明确打开的图片，即便列表达到上限也不丢失当前位置。
    if let Some(path) = anchor.filter(|p| scope.contains(p) && p.is_file()) {
        let cost = path_cost(path);
        if limits.files > 0 && cost <= limits.index_bytes {
            charged += cost;
            files.push(path.to_path_buf());
        }
    }
    let mut stack = vec![(scope.root.clone(), 0usize)];
    let mut queued_directories = 1usize;
    let mut last_progress = Instant::now();
    'walk: while let Some((folder, depth)) = stack.pop() {
        if cancelled() { return None; }
        if started.elapsed() >= limits.elapsed { stats.limit = Some("扫描时间上限（15秒）"); break; }
        // 每个子目录在使用前再次检查，拒绝扫描到根外或跟随 junction。
        if depth > 0 {
            let meta = match std::fs::symlink_metadata(&folder) { Ok(m) => m, Err(_) => { stats.skipped_errors += 1; continue; } };
            if is_link(&meta) { stats.skipped_links += 1; continue; }
        }
        match std::fs::canonicalize(&folder) {
            Ok(path) if path.starts_with(&canonical_root) => {},
            _ => { stats.skipped_errors += 1; continue; }
        }
        let entries = match std::fs::read_dir(&folder) { Ok(entries) => entries, Err(_) => { stats.skipped_errors += 1; continue; } };
        stats.directories += 1;
        for entry in entries {
            if cancelled() { return None; }
            if started.elapsed() >= limits.elapsed { stats.limit = Some("扫描时间上限（15秒）"); break 'walk; }
            if stats.entries >= limits.entries { stats.limit = Some("目录条目上限（25万项）"); break 'walk; }
            stats.entries += 1;
            if last_progress.elapsed() >= Duration::from_millis(100) {
                stats.files = files.len(); stats.elapsed_ms = started.elapsed().as_millis() as u64;
                progress(stats.clone()); last_progress = Instant::now();
            }
            let entry = match entry { Ok(e) => e, Err(_) => { stats.skipped_errors += 1; continue; } };
            let path = entry.path();
            let ft = match entry.file_type() { Ok(ft) => ft, Err(_) => { stats.skipped_errors += 1; continue; } };
            if ft.is_symlink() { stats.skipped_links += 1; continue; }
            let file = ft.is_file() && has_supported_ext(&path);
            let dir = ft.is_dir() && scope.recursive;
            if !file && !dir { continue; }
            let meta = match entry.metadata() { Ok(m) => m, Err(_) => { stats.skipped_errors += 1; continue; } };
            if is_link(&meta) { stats.skipped_links += 1; continue; }
            if dir {
                if depth >= limits.depth { stats.skipped_depth += 1; continue; }
                if queued_directories >= limits.directories { stats.limit = Some("子目录数量上限（1万个）"); break 'walk; }
            } else {
                if Some(path.as_path()) == anchor { continue; }
                if files.len() >= limits.files { stats.limit = Some("图片数量上限（5万张）"); break 'walk; }
            }
            let cost = path_cost(&path);
            if charged.saturating_add(cost) > limits.index_bytes { stats.limit = Some("路径索引预算（32 MiB）"); break 'walk; }
            charged += cost;
            if dir { stack.push((path, depth + 1)); queued_directories += 1; }
            else { files.push(path); }
        }
    }
    if cancelled() { return None; }
    // 后台排序一次，按相对路径排列；发布时保留当前图片而不是重置到第一张。
    files.sort_by_cached_key(|path| path.strip_prefix(&scope.root).unwrap_or(path).to_string_lossy().to_lowercase());
    if cancelled() { return None; }
    stats.files = files.len(); stats.elapsed_ms = started.elapsed().as_millis() as u64;
    Some((files, stats))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("iv-scope-{}-{}", std::process::id(),
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
            std::fs::create_dir_all(p.join("selected/child/deep")).unwrap();
            for name in ["parent.png", "selected/00.png", "selected/01.jpg", "selected/child/02.webp", "selected/child/deep/03.psd", "selected/readme.txt"] {
                std::fs::write(p.join(name), []).unwrap();
            }
            Self(p)
        }
        fn scope(&self, recursive: bool) -> Scope { Scope::for_image(&self.0.join("selected/00.png"), recursive).unwrap() }
    }
    impl Drop for Fixture { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
    fn result(scope: &Scope, limits: Limits) -> (Vec<PathBuf>, ScanStats) { scan(scope, None, limits, || false, |_| {}).unwrap() }
    #[test]
    fn scan_filters_and_sorts_without_recursing() {
        let f = Fixture::new(); let (files, stats) = result(&f.scope(false), Limits::default());
        assert_eq!(files, [f.0.join("selected/00.png"), f.0.join("selected/01.jpg")]); assert!(!stats.partial());
    }
    #[test]
    fn recursive_scope_includes_descendants_but_never_parents() {
        let f = Fixture::new(); let scope = f.scope(true); let (files, stats) = result(&scope, Limits::default());
        assert_eq!(files.len(), 4); assert!(files.iter().all(|p| scope.contains(p)));
        assert!(!files.contains(&f.0.join("parent.png"))); assert_eq!(stats.directories, 3);
        assert!(!scope.contains(&scope.root.join("../parent.png")));
    }
    #[test]
    fn inaccessible_directory_is_reported() {
        let scope = Scope { root: PathBuf::from("__iv_missing_directory__"), recursive: true };
        let (files, stats) = result(&scope, Limits::default()); assert!(files.is_empty()); assert!(stats.partial());
    }
    #[test]
    fn file_budget_retains_the_explicit_image_and_reports_partial_results() {
        let f = Fixture::new(); let anchor = f.0.join("selected/01.jpg");
        let (files, stats) = scan(&f.scope(true), Some(&anchor), Limits { files: 1, ..Limits::default() }, || false, |_| {}).unwrap();
        assert_eq!(files, [anchor]); assert!(stats.limit.is_some());
    }
    #[test]
    fn memory_entry_depth_directory_and_time_limits_are_enforced() {
        let f = Fixture::new(); let scope = f.scope(true);
        for limits in [Limits { index_bytes: 1, ..Limits::default() }, Limits { entries: 1, ..Limits::default() },
            Limits { directories: 1, ..Limits::default() }, Limits { elapsed: Duration::ZERO, ..Limits::default() }] {
            assert!(result(&scope, limits).1.limit.is_some());
        }
        let (files, stats) = result(&scope, Limits { depth: 0, ..Limits::default() });
        assert_eq!(files.len(), 2); assert!(stats.skipped_depth > 0);
    }
    #[test]
    fn cancelled_work_does_not_publish_a_list() {
        let f = Fixture::new(); let checks = std::cell::Cell::new(0usize);
        let out = scan(&f.scope(true), None, Limits::default(), || { checks.set(checks.get()+1); checks.get() > 3 }, |_| {});
        assert!(out.is_none());
    }
    #[test]
    fn latest_request_replaces_pending_progress_and_old_results() {
        let f = Fixture::new(); let scanner = Scanner::spawn(egui::Context::default());
        for id in 1..=100 { scanner.request(id, f.scope(id < 100), None); }
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(result) = scanner.poll() {
                assert_eq!(result.id, 100);
                if let Some(files) = result.files { assert_eq!(files.len(), 2); break; }
            }
            assert!(Instant::now() < until); std::thread::sleep(Duration::from_millis(5));
        }
        scanner.cancel(101); assert!(scanner.poll().is_none());
    }
    #[test]
    #[cfg(windows)]
    fn junction_to_parent_and_outside_is_not_followed() {
        let f = Fixture::new(); let link = f.0.join("selected").join("escape");
        let status = std::process::Command::new("cmd").args(["/C", "mklink", "/J"])
            .arg(&link).arg(&f.0).output().unwrap();
        assert!(status.status.success(), "测试目录 junction 创建失败");
        let (files, stats) = result(&f.scope(true), Limits::default());
        // 单独移除本测试建立的目录链接，不递归进入目标。
        std::fs::remove_dir(&link).unwrap();
        assert_eq!(files.len(), 4); assert!(stats.skipped_links > 0);
    }
}
