//! Opt-in timing only. Enable LCL_IV_PERF=1 and capture stderr; no normal-user disk writes.
use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();
static ENABLED: OnceLock<bool> = OnceLock::new();

pub fn init() {
    START.get_or_init(Instant::now);
    ENABLED.get_or_init(|| std::env::var("LCL_IV_PERF").as_deref() == Ok("1"));
    mark("main_start", None, 0.0);
}

pub fn mark(event: &str, path: Option<&Path>, stage_ms: f64) {
    if !*ENABLED.get_or_init(|| false) { return; }
    let name = path.and_then(Path::file_name).unwrap_or_default().to_string_lossy()
        .replace(['\t', '\r', '\n'], " ");
    let elapsed = START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0;
    eprintln!("IVPERF\t{elapsed:.3}\t{event}\t{name}\t{stage_ms:.3}");
}
