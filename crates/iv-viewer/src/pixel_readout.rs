//! 底栏像素读数的纯展示逻辑；不修改源像素、通道渲染或完整检查器。
use crate::render::ChannelMode;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PixelFormat {
    Normalized,
    Bytes,
    #[default]
    Hex,
}

impl PixelFormat {
    pub const ALL: [Self; 3] = [Self::Normalized, Self::Bytes, Self::Hex];
    pub const STORAGE_KEY: &'static str = "iv-pixel-format";

    pub fn from_key(value: Option<&str>) -> Self {
        match value {
            Some("normalized") => Self::Normalized,
            Some("bytes") => Self::Bytes,
            _ => Self::Hex, // 旧偏好、缺失或未知值保留原来的 HEX 显示。
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Normalized => "normalized",
            Self::Bytes => "bytes",
            Self::Hex => "hex",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Normalized => "RGBA（0–1）",
            Self::Bytes => "RGBA（0–255）",
            Self::Hex => "十六进制",
        }
    }

    fn component(self, value: u8) -> String {
        match self {
            Self::Normalized => format!("{:.3}", value as f32 / 255.0)
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_owned(),
            Self::Bytes => value.to_string(),
            Self::Hex => format!("#{value:02X}"),
        }
    }

    pub fn color_text(self, rgba: [u8; 4], channel: ChannelMode) -> String {
        if let Some((name, index)) = single_channel(channel) {
            return format!("{name} {}", self.component(rgba[index]));
        }
        let opaque = channel == ChannelMode::RgbOpaque;
        let count = if opaque { 3 } else { 4 };
        if self == Self::Hex {
            let components = rgba[..count]
                .iter()
                .map(|v| format!("{v:02X}"))
                .collect::<String>();
            format!("#{components}")
        } else {
            let label = if opaque { "RGB" } else { "RGBA" };
            let values = rgba[..count]
                .iter()
                .map(|&v| self.component(v))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{label} ({values})")
        }
    }

    pub fn text(self, position: (u32, u32), rgba: [u8; 4], channel: ChannelMode) -> String {
        format!(
            "({}, {})   {}",
            position.0,
            position.1,
            self.color_text(rgba, channel)
        )
    }

    /// 同一格式/通道/Mip 内预留最大读数槽位，避免移动光标或动画换帧时底栏跳动。
    pub fn width_sample(self, dimensions: (u32, u32), channel: ChannelMode) -> String {
        let widest_value = match self {
            Self::Normalized => 128, // 0.502：最长归一化分量为 5 个 ASCII 字符。
            Self::Bytes | Self::Hex => 255,
        };
        self.text(dimensions, [widest_value; 4], channel)
    }
}

pub fn single_channel(channel: ChannelMode) -> Option<(&'static str, usize)> {
    match channel {
        ChannelMode::R => Some(("R", 0)),
        ChannelMode::G => Some(("G", 1)),
        ChannelMode::B => Some(("B", 2)),
        ChannelMode::A => Some(("A", 3)),
        _ => None,
    }
}

/// 单通道与画布一样按不透明灰度显示，Alpha=0 时也能检查隐藏的 RGB。
pub fn swatch(rgba: [u8; 4], channel: ChannelMode) -> [u8; 4] {
    if let Some((_, index)) = single_channel(channel) {
        let v = rgba[index];
        [v, v, v, 255]
    } else if channel == ChannelMode::RgbOpaque {
        [rgba[0], rgba[1], rgba[2], 255]
    } else {
        rgba
    }
}

pub fn placeholder(channel: ChannelMode) -> String {
    match single_channel(channel) {
        Some((name, _)) => format!("{name} · 移入图片取样"),
        None => "移入图片取样".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_legacy_default() {
        for f in PixelFormat::ALL {
            assert_eq!(PixelFormat::from_key(Some(f.key())), f);
        }
        for value in [None, Some(""), Some("future-format")] {
            assert_eq!(PixelFormat::from_key(value), PixelFormat::Hex);
        }
    }

    #[test]
    fn coordinates_and_full_rgba_formats() {
        let px = [244, 199, 123, 253];
        assert_eq!(
            PixelFormat::Hex.text((742, 914), px, ChannelMode::Rgb),
            "(742, 914)   #F4C77BFD"
        );
        assert_eq!(
            PixelFormat::Bytes.text((742, 914), px, ChannelMode::Rgb),
            "(742, 914)   RGBA (244, 199, 123, 253)"
        );
        assert_eq!(
            PixelFormat::Normalized.text((0, 0), [0, 255, 128, 0], ChannelMode::Rgb),
            "(0, 0)   RGBA (0, 1, 0.502, 0)"
        );
    }

    #[test]
    fn normalized_bytes_are_unambiguous_without_gamma_or_alpha_multiplication() {
        let mut prior = -1.0_f32;
        for value in 0..=255_u8 {
            let text = PixelFormat::Normalized.component(value);
            let parsed: f32 = text.parse().unwrap();
            assert!((parsed - value as f32 / 255.0).abs() <= 0.000501);
            assert!(parsed > prior);
            assert!(text.len() <= 5);
            prior = parsed;
        }
    }

    #[test]
    fn each_channel_is_one_original_component_in_every_format() {
        let px = [17, 88, 199, 0];
        for (channel, name, index) in [
            (ChannelMode::R, "R", 0),
            (ChannelMode::G, "G", 1),
            (ChannelMode::B, "B", 2),
            (ChannelMode::A, "A", 3),
        ] {
            for f in PixelFormat::ALL {
                let text = f.color_text(px, channel);
                assert_eq!(text, format!("{name} {}", f.component(px[index])));
                assert!(!text.contains(',') && !text.contains("RGBA"));
            }
        }
    }

    #[test]
    fn swatch_matches_selected_channel_and_does_not_change_source() {
        let px = [17, 88, 199, 0];
        assert_eq!(swatch(px, ChannelMode::Rgb), px);
        assert_eq!(swatch(px, ChannelMode::R), [17, 17, 17, 255]);
        assert_eq!(swatch(px, ChannelMode::G), [88, 88, 88, 255]);
        assert_eq!(swatch(px, ChannelMode::B), [199, 199, 199, 255]);
        assert_eq!(swatch(px, ChannelMode::A), [0, 0, 0, 255]);
        assert_eq!(swatch(px, ChannelMode::RgbOpaque), [17, 88, 199, 255]);
        assert_eq!(px, [17, 88, 199, 0]);
    }

    #[test]
    fn ignoring_alpha_only_shows_original_rgb() {
        let px = [0, 128, 255, 7];
        assert_eq!(
            PixelFormat::Hex.color_text(px, ChannelMode::RgbOpaque),
            "#0080FF"
        );
        assert_eq!(
            PixelFormat::Bytes.color_text(px, ChannelMode::RgbOpaque),
            "RGB (0, 128, 255)"
        );
        assert_eq!(
            PixelFormat::Normalized.color_text(px, ChannelMode::RgbOpaque),
            "RGB (0, 0.502, 1)"
        );
    }

    #[test]
    fn width_sample_bounds_all_byte_values_and_large_coordinates() {
        for dimensions in [(1, 1), (320, 240), (16384, 16384), (u32::MAX, u32::MAX)] {
            for channel in [
                ChannelMode::Rgb,
                ChannelMode::RgbOpaque,
                ChannelMode::R,
                ChannelMode::G,
                ChannelMode::B,
                ChannelMode::A,
            ] {
                for f in PixelFormat::ALL {
                    let budget = f.width_sample(dimensions, channel).len();
                    for v in 0..=255_u8 {
                        let text = f.text((dimensions.0 - 1, dimensions.1 - 1), [v; 4], channel);
                        assert!(text.len() <= budget, "{text}");
                    }
                }
            }
        }
    }
}
