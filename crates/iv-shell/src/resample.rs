//! 简单 box（面积平均）缩放 —— 缩略图质量足够，零依赖。

/// RGBA8 缩放到 (dw, dh)。要求 src 长度 == sw*sh*4，且 dw<=sw、dh<=dh（放大调用方不做）。
pub fn downscale_box(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    debug_assert_eq!(src.len(), sw * sh * 4);
    let mut dst = vec![0u8; dw * dh * 4];
    for dy in 0..dh {
        // 源矩形 [sy0, sy1) × [sx0, sx1)（整数 box，边界略有重叠，视觉可接受）
        let sy0 = dy * sh / dh;
        let sy1 = ((dy + 1) * sh + dh - 1) / dh;
        for dx in 0..dw {
            let sx0 = dx * sw / dw;
            let sx1 = ((dx + 1) * sw + dw - 1) / dw;
            let mut acc = [0u64; 4];
            let mut n = 0u64;
            for sy in sy0..sy1 {
                let row = sy * sw;
                for sx in sx0..sx1 {
                    let i = (row + sx) * 4;
                    acc[0] += src[i] as u64;
                    acc[1] += src[i + 1] as u64;
                    acc[2] += src[i + 2] as u64;
                    acc[3] += src[i + 3] as u64;
                    n += 1;
                }
            }
            let o = (dy * dw + dx) * 4;
            dst[o] = (acc[0] / n) as u8;
            dst[o + 1] = (acc[1] / n) as u8;
            dst[o + 2] = (acc[2] / n) as u8;
            dst[o + 3] = (acc[3] / n) as u8;
        }
    }
    dst
}
