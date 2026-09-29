//! Sequential editing primitives. Coordinates and output refer to the CURRENT image.
//! Quarter turns and mirrors are byte-exact; arbitrary angles use alpha-weighted bilinear.
use crate::image_edit::{self, Crop, Filter, Pixels, Plan, Source};

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Crop(Crop),
    Resize(u32, u32, Filter),
    Quarter(i32),
    FlipHorizontal,
    FlipVertical,
    Rotate(f64),
}

pub fn rotated_size(size: (u32, u32), degrees: f64) -> Result<(u32, u32), String> {
    if !degrees.is_finite() || degrees.abs() > 360.0 {
        return Err("旋转角度需为 -360° 到 360° 的有限数值".into());
    }
    let a = degrees.to_radians();
    let (s, c) = (a.sin().abs(), a.cos().abs());
    // Epsilon avoids an extra row/column at exact multiples of 90 degrees.
    let w = (size.0 as f64 * c + size.1 as f64 * s - 1e-9)
        .ceil()
        .max(1.0) as u32;
    let h = (size.0 as f64 * s + size.1 as f64 * c - 1e-9)
        .ceil()
        .max(1.0) as u32;
    image_edit::pixel_bytes(w, h)?;
    Ok((w, h))
}

pub fn apply(source: &Source, op: Operation) -> Result<Source, String> {
    let (w, h) = source.size;
    let pixels = match op {
        Operation::Crop(crop) => {
            let mut p = Plan::full(source.size);
            p.set_crop(crop);
            source.render(p)?
        }
        Operation::Resize(width, height, filter) => {
            let mut p = Plan::full(source.size);
            p.width = width;
            p.height = height;
            p.filter = filter;
            source.render(p)?
        }
        Operation::Rotate(deg) if (deg / 90.0 - (deg / 90.0).round()).abs() < 1e-9 => {
            rotated_size(source.size, deg)?;
            return apply(source, Operation::Quarter((deg / 90.0).round() as i32));
        }
        Operation::Rotate(degrees) => {
            let (ow, oh) = rotated_size(source.size, degrees)?;
            let src = source.rgba()?;
            let mut data = allocate(ow, oh)?;
            let (s, c) = degrees.to_radians().sin_cos();
            for y in 0..oh {
                for x in 0..ow {
                    let (dx, dy) = (
                        x as f64 + 0.5 - ow as f64 / 2.0,
                        y as f64 + 0.5 - oh as f64 / 2.0,
                    );
                    let fx = c * dx + s * dy + w as f64 / 2.0 - 0.5;
                    let fy = -s * dx + c * dy + h as f64 / 2.0 - 0.5;
                    let (x0, y0) = (fx.floor() as i64, fy.floor() as i64);
                    let (tx, ty) = (fx - fx.floor(), fy - fy.floor());
                    let mut sum = [0.0; 4];
                    for (sx, sy, weight) in [
                        (x0, y0, (1.0 - tx) * (1.0 - ty)),
                        (x0 + 1, y0, tx * (1.0 - ty)),
                        (x0, y0 + 1, (1.0 - tx) * ty),
                        (x0 + 1, y0 + 1, tx * ty),
                    ] {
                        if sx < 0 || sy < 0 || sx >= w as i64 || sy >= h as i64 {
                            continue;
                        }
                        let i = (sy as usize * w as usize + sx as usize) * 4;
                        let a = src[i + 3] as f64 * weight;
                        sum[3] += a;
                        for n in 0..3 {
                            sum[n] += src[i + n] as f64 * a;
                        }
                    }
                    let i = (y as usize * ow as usize + x as usize) * 4;
                    data[i + 3] = sum[3].round().clamp(0.0, 255.0) as u8;
                    for n in 0..3 {
                        data[i + n] = if sum[3] > 1e-8 {
                            (sum[n] / sum[3]).round().clamp(0.0, 255.0) as u8
                        } else {
                            0
                        };
                    }
                }
            }
            Pixels {
                width: ow,
                height: oh,
                data,
            }
        }
        op => {
            let turns = if let Operation::Quarter(n) = op {
                n.rem_euclid(4)
            } else {
                0
            };
            let (ow, oh) = if turns % 2 == 1 { (h, w) } else { (w, h) };
            let mut data = allocate(ow, oh)?;
            let src = source.rgba()?;
            for y in 0..oh {
                for x in 0..ow {
                    let (sx, sy) = match op {
                        Operation::FlipHorizontal => (w - 1 - x, y),
                        Operation::FlipVertical => (x, h - 1 - y),
                        _ => match turns {
                            1 => (y, h - 1 - x),
                            2 => (w - 1 - x, h - 1 - y),
                            3 => (w - 1 - y, x),
                            _ => (x, y),
                        },
                    };
                    let dst = (y as usize * ow as usize + x as usize) * 4;
                    let si = (sy as usize * w as usize + sx as usize) * 4;
                    data[dst..dst + 4].copy_from_slice(&src[si..si + 4]);
                }
            }
            Pixels {
                width: ow,
                height: oh,
                data,
            }
        }
    };
    Source::from_pixels(pixels, source.note.clone())
}
fn allocate(w: u32, h: u32) -> Result<Vec<u8>, String> {
    let n = image_edit::pixel_bytes(w, h)?;
    let mut data = Vec::new();
    data.try_reserve_exact(n)
        .map_err(|_| "内存不足，请减小尺寸")?;
    data.resize(n, 0);
    Ok(data)
}

#[derive(Clone)]
pub struct Snapshot {
    pub id: u64,
    pub source: Source,
}
pub struct Document {
    pub current: Snapshot,
    original: Snapshot,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    next_id: u64,
}
impl Document {
    pub const HISTORY_BUDGET: usize = 128 * 1024 * 1024;
    pub fn new(source: Source) -> Self {
        let s = Snapshot { id: 0, source };
        Self {
            current: s.clone(),
            original: s,
            undo: vec![],
            redo: vec![],
            next_id: 1,
        }
    }
    pub fn commit(&mut self, source: Source) {
        self.undo.push(self.current.clone());
        self.redo.clear();
        self.current = Snapshot {
            id: self.next_id,
            source,
        };
        self.next_id += 1;
        self.trim_history();
    }
    fn trim_history(&mut self) {
        while self.undo.len() + self.redo.len() > 32
            || self
                .undo
                .iter()
                .chain(&self.redo)
                .map(|s| s.source.byte_len())
                .sum::<usize>()
                > Self::HISTORY_BUDGET
        {
            if !self.undo.is_empty() {
                self.undo.remove(0);
            } else if !self.redo.is_empty() {
                self.redo.remove(0);
            } else {
                break;
            }
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo(&mut self) -> bool {
        if let Some(s) = self.undo.pop() {
            self.redo.push(self.current.clone());
            self.current = s;
            self.trim_history();
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self) -> bool {
        if let Some(s) = self.redo.pop() {
            self.undo.push(self.current.clone());
            self.current = s;
            self.trim_history();
            true
        } else {
            false
        }
    }
    pub fn reset(&mut self) {
        if self.current.id != self.original.id {
            self.undo.push(self.current.clone());
            self.redo.clear();
            self.current = self.original.clone();
            self.trim_history();
        }
    }
}

/// View-only scale/offset; these are never used to size an export.
#[derive(Clone, Copy, Debug)]
pub struct CanvasView {
    pub scale: f32,
    pub offset: [f32; 2],
}
impl Default for CanvasView {
    fn default() -> Self {
        Self {
            scale: 1.0,
            offset: [0.0, 0.0],
        }
    }
}
impl CanvasView {
    pub fn fit(&mut self, size: (u32, u32), viewport: [f32; 2]) {
        self.scale = ((viewport[0] - 48.0) / size.0 as f32)
            .min((viewport[1] - 48.0) / size.1 as f32)
            .clamp(0.01, 32.0);
        self.center(size, viewport);
    }
    pub fn center(&mut self, size: (u32, u32), viewport: [f32; 2]) {
        self.offset = [
            (viewport[0] - size.0 as f32 * self.scale) * 0.5,
            (viewport[1] - size.1 as f32 * self.scale) * 0.5,
        ];
    }
    pub fn zoom(&mut self, point: [f32; 2], factor: f32) {
        let old = self.scale;
        self.scale = (old * factor).clamp(0.01, 32.0);
        for (i, p) in point.iter().enumerate() {
            self.offset[i] = p - (p - self.offset[i]) * self.scale / old;
        }
    }
}

/// Fixed-opposite-edge crop resizing, with 8 handles (clockwise from top-left).
pub fn resize_crop(
    c: Crop,
    handle: usize,
    point: [f32; 2],
    size: (u32, u32),
    ratio: Option<(u32, u32)>,
) -> Crop {
    let left = matches!(handle, 0 | 6 | 7);
    let right = matches!(handle, 2 | 3 | 4);
    let top = matches!(handle, 0 | 1 | 2);
    let bottom = matches!(handle, 4 | 5 | 6);
    let (mut x0, mut y0, mut x1, mut y1) = (
        c.x as f32,
        c.y as f32,
        (c.x + c.w) as f32,
        (c.y + c.h) as f32,
    );
    if left {
        x0 = point[0].clamp(0.0, x1 - 1.0);
    }
    if right {
        x1 = point[0].clamp(x0 + 1.0, size.0 as f32);
    }
    if top {
        y0 = point[1].clamp(0.0, y1 - 1.0);
    }
    if bottom {
        y1 = point[1].clamp(y0 + 1.0, size.1 as f32);
    }
    if let Some((rw, rh)) = ratio {
        let aspect = rw as f32 / rh.max(1) as f32;
        let mut w = x1 - x0;
        let mut h = y1 - y0;
        if left || right {
            h = w / aspect;
        } else {
            w = h * aspect;
        }
        let ax = if left {
            x1
        } else if right {
            x0
        } else {
            (x0 + x1) / 2.0
        };
        let ay = if top {
            y1
        } else if bottom {
            y0
        } else {
            (y0 + y1) / 2.0
        };
        let maxw = if left {
            ax
        } else if right {
            size.0 as f32 - ax
        } else {
            2.0 * ax.min(size.0 as f32 - ax)
        };
        let maxh = if top {
            ay
        } else if bottom {
            size.1 as f32 - ay
        } else {
            2.0 * ay.min(size.1 as f32 - ay)
        };
        let k = (maxw / w).min(maxh / h).min(1.0);
        w = (w * k).max(1.0);
        h = (h * k).max(1.0);
        x0 = if left {
            ax - w
        } else if right {
            ax
        } else {
            ax - w / 2.0
        };
        x1 = x0 + w;
        y0 = if top {
            ay - h
        } else if bottom {
            ay
        } else {
            ay - h / 2.0
        };
        y1 = y0 + h;
    }
    Crop::from_points([x0.round(), y0.round()], [x1.round(), y1.round()], size)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Source {
        Source::from_pixels(
            Pixels {
                width: 3,
                height: 2,
                data: (0..6).flat_map(|n| [n, n + 20, n + 40, n * 40]).collect(),
            },
            "test".into(),
        )
        .unwrap()
    }
    fn red(s: &Source) -> Vec<u8> {
        s.rgba().unwrap().chunks_exact(4).map(|p| p[0]).collect()
    }
    #[test]
    fn exact_quarter_turns_and_mirrors() {
        let s = sample();
        let cw = apply(&s, Operation::Quarter(1)).unwrap();
        assert_eq!(cw.size, (2, 3));
        assert_eq!(red(&cw), vec![3, 0, 4, 1, 5, 2]);
        assert_eq!(
            red(&apply(&s, Operation::Quarter(-1)).unwrap()),
            vec![2, 5, 1, 4, 0, 3]
        );
        assert_eq!(
            red(&apply(&s, Operation::FlipHorizontal).unwrap()),
            vec![2, 1, 0, 5, 4, 3]
        );
        assert_eq!(
            red(&apply(&s, Operation::FlipVertical).unwrap()),
            vec![3, 4, 5, 0, 1, 2]
        );
        let mut t = s.clone();
        for _ in 0..4 {
            t = apply(&t, Operation::Quarter(1)).unwrap();
        }
        assert_eq!(t.rgba().unwrap(), s.rgba().unwrap());
    }
    #[test]
    fn sequential_rotate_crop_resize() {
        let s = sample();
        let s = apply(&s, Operation::Quarter(1)).unwrap();
        let s = apply(
            &s,
            Operation::Crop(Crop {
                x: 0,
                y: 1,
                w: 2,
                h: 1,
            }),
        )
        .unwrap();
        assert_eq!(red(&s), vec![4, 1]);
        let s = apply(&s, Operation::Resize(4, 1, Filter::Nearest)).unwrap();
        assert_eq!(red(&s), vec![4, 4, 1, 1]);
    }
    #[test]
    fn arbitrary_rotation_dimensions_alpha_and_invalid() {
        let s = sample();
        assert_eq!(rotated_size((3, 2), 90.0).unwrap(), (2, 3));
        assert_eq!(rotated_size((3, 2), 45.0).unwrap(), (4, 4));
        assert!(rotated_size((3, 2), f64::NAN).is_err());
        assert!(rotated_size((3, 2), f64::INFINITY).is_err());
        assert_eq!(
            apply(&s, Operation::Rotate(180.0)).unwrap().rgba().unwrap(),
            apply(&s, Operation::Quarter(2)).unwrap().rgba().unwrap()
        );
        let t = apply(&s, Operation::Rotate(45.0)).unwrap();
        assert_eq!(t.size, (4, 4));
        assert!(t.rgba().unwrap().chunks_exact(4).any(|p| p[3] == 0));
    }
    #[test]
    fn rotated_alpha_ignores_hidden_color() {
        let s = Source::from_pixels(
            Pixels {
                width: 2,
                height: 1,
                data: vec![255, 0, 0, 255, 0, 0, 255, 0],
            },
            "".into(),
        )
        .unwrap();
        let r = apply(&s, Operation::Rotate(30.0)).unwrap();
        for p in r.rgba().unwrap().chunks_exact(4) {
            if p[3] > 0 {
                assert_eq!(&p[..3], &[255, 0, 0]);
            }
        }
    }
    #[test]
    fn history_tracks_saved_revision_and_sequence() {
        let s = sample();
        let raw = s.rgba().unwrap().to_vec();
        let mut d = Document::new(s);
        d.commit(apply(&d.current.source, Operation::Quarter(1)).unwrap());
        let a = d.current.id;
        d.commit(apply(&d.current.source, Operation::FlipVertical).unwrap());
        assert!(d.undo());
        assert_eq!(d.current.id, a);
        assert!(d.undo());
        assert_eq!(d.current.id, 0);
        assert_eq!(d.current.source.rgba().unwrap(), raw);
        assert!(d.redo());
        d.commit(apply(&d.current.source, Operation::Quarter(2)).unwrap());
        assert!(!d.can_redo());
        d.reset();
        assert_eq!(d.current.source.rgba().unwrap(), raw);
        for _ in 0..50 {
            d.commit(d.current.source.clone());
        }
        assert_eq!(d.undo.len(), 32);
    }
    #[test]
    fn view_zoom_anchor_does_not_modify_pixels() {
        let mut v = CanvasView::default();
        v.fit((100, 50), [800.0, 500.0]);
        let point = [300.0, 200.0];
        let before = [
            (point[0] - v.offset[0]) / v.scale,
            (point[1] - v.offset[1]) / v.scale,
        ];
        v.zoom(point, 2.0);
        for i in 0..2 {
            assert!(((point[i] - v.offset[i]) / v.scale - before[i]).abs() < 0.001);
        }
    }
    #[test]
    fn crop_handles_clamp_and_keep_ratio() {
        let c = Crop {
            x: 10,
            y: 10,
            w: 40,
            h: 20,
        };
        for handle in 0..8 {
            for p in [[-900.0, -500.0], [900.0, 400.0], [30.0, 25.0]] {
                let r = resize_crop(c, handle, p, (100, 100), None);
                r.validate((100, 100)).unwrap();
                let r = resize_crop(c, handle, p, (100, 100), Some((2, 1)));
                r.validate((100, 100)).unwrap();
                assert!((r.w as i32 - 2 * r.h as i32).abs() <= 2);
            }
        }
    }
}
