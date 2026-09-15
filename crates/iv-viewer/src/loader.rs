//! 后台解码线程：主线程发命令，线程解码并回传结果。
//!
//! 单线程 + 队列足够：解码是 CPU 密集任务，顺序消费命令可自然
//! 处理"快速切图时丢弃过期请求"（App 侧只认当前路径的结果）。

use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use crossbeam_channel::{Receiver, Sender};
use iv_core::decode::{decode_path, DecodedImage};

pub enum Cmd {
    /// 解码并显示（App 等待结果）
    Load(PathBuf),
    /// 预读（结果只进缓存，不切换显示）
    Prefetch(PathBuf),
}

pub enum Msg {
    /// 解码完成（Load / Prefetch 都会回，App 按路径区分用途）
    Ready(Result<(PathBuf, Arc<DecodedImage>), (PathBuf, String)>),
}

pub struct Loader {
    tx: Sender<Cmd>,
    rx: Receiver<Msg>,
    #[allow(dead_code)]
    handle: thread::JoinHandle<()>,
}

impl Loader {
    pub fn spawn() -> Self {
        let (tx_cmd, rx_cmd) = crossbeam_channel::bounded::<Cmd>(64);
        let (tx_msg, rx_msg) = crossbeam_channel::bounded::<Msg>(64);
        let handle = thread::Builder::new()
            .name("iv-loader".into())
            .spawn(move || {
                while let Ok(cmd) = rx_cmd.recv() {
                    let path = match cmd {
                        Cmd::Load(p) => p,
                        Cmd::Prefetch(p) => p,
                    };
                    let started = std::time::Instant::now();
                    crate::perf::mark("decode_start", Some(&path), 0.0);
                    let result = decode_path(&path).map_err(|e| (path.clone(), e.to_string()));
                    crate::perf::mark("decode_ready", Some(&path), started.elapsed().as_secs_f64()*1000.0);
                    let msg = Msg::Ready(result.map(|img| (path, Arc::new(img))));
                    if tx_msg.send(msg).is_err() {
                        break; // 主线程已退出
                    }
                }
            })
            .expect("创建解码线程失败");
        Self {
            tx: tx_cmd,
            rx: rx_msg,
            handle,
        }
    }

    pub fn load(&self, path: PathBuf) {
        let _ = self.tx.send(Cmd::Load(path));
    }

    pub fn prefetch(&self, path: PathBuf) {
        let _ = self.tx.send(Cmd::Prefetch(path));
    }

    /// 非阻塞收取一条消息。
    pub fn poll(&self) -> Option<Msg> {
        self.rx.try_recv().ok()
    }
}
