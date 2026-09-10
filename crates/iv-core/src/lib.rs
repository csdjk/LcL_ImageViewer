//! iv-core：ImageView 的纯逻辑核心（无 GUI 依赖）。
//!
//! 设计约束：
//! - 解码统一输出 RGBA（8bit LDR 或 32bit float HDR），查看逻辑与格式解耦
//! - 对畸形文件必须返回 `Err`，绝不 panic（该库会跑在 Explorer 缩略图进程里）

pub mod dds;
pub mod decode;
pub mod format;
pub mod psd_composite;

pub use decode::{decode_bytes, decode_path, AnimatedFrame, DecodedImage, DecodeError, ImageKind, MipLevel, PixelData};
