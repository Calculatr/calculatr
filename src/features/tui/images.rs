use std::collections::HashMap;

use image::{DynamicImage, Rgba, RgbaImage};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};

use super::theme::{Theme, rgb};
use super::typst::markup::FONT_SIZE_PT;
use super::typst::renderer::TypstRenderer;

pub const FADE_LEVELS: u8 = 12;

pub fn pixel_per_pt(cell_height_px: u32) -> f32 {
    (cell_height_px.max(1) as f32 * 0.75) / FONT_SIZE_PT
}

pub fn fade_level(brightness: f32) -> u8 {
    ((brightness * FADE_LEVELS as f32).round() as u8).clamp(1, FADE_LEVELS)
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    source: String,
    level: u8,
    selected: bool,
}

const MAX_PROTOCOLS: usize = 600;

#[derive(Default)]
pub struct ImageCache {
    scale: f32,
    base: HashMap<String, Option<RgbaImage>>,
    protocols: HashMap<Key, StatefulProtocol>,
}

impl ImageCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_scale(&mut self, scale: f32) {
        if (self.scale - scale).abs() > f32::EPSILON {
            self.scale = scale;
            self.base.clear();
            self.protocols.clear();
        }
    }

    fn base(&mut self, source: &str) -> Option<&RgbaImage> {
        if !self.base.contains_key(source) {
            let image = TypstRenderer::new(source.to_string()).render(self.scale);
            if let Err(e) = &image {
                let _ = std::fs::write(std::env::temp_dir().join("calculatr-typst.log"), e);
            }
            self.base.insert(source.to_string(), image.ok());
        }
        self.base.get(source)?.as_ref()
    }

    pub fn size(&mut self, source: &str) -> Option<(u32, u32)> {
        self.base(source).map(|img| (img.width(), img.height()))
    }

    pub fn protocol(
        &mut self,
        picker: &Picker,
        theme: &Theme,
        source: &str,
        level: u8,
        selected: bool,
    ) -> Option<&mut StatefulProtocol> {
        let key = Key {
            source: source.to_string(),
            level: if selected { FADE_LEVELS } else { level },
            selected,
        };
        if !self.protocols.contains_key(&key) {
            if self.protocols.len() > MAX_PROTOCOLS {
                self.protocols.clear();
            }
            let brightness = key.level as f32 / FADE_LEVELS as f32;
            let variant = recolor(self.base(source)?, theme, brightness, selected);
            let proto = picker.new_resize_protocol(DynamicImage::ImageRgba8(variant));
            self.protocols.insert(key.clone(), proto);
        }
        self.protocols.get_mut(&key)
    }
}

fn recolor(base: &RgbaImage, theme: &Theme, brightness: f32, selected: bool) -> RgbaImage {
    let mut out = base.clone();
    if selected {
        let fg = rgb(theme.highlight_fg);
        let bg = rgb(theme.highlight_bg);
        for p in out.pixels_mut() {
            let a = p[3] as f32 / 255.0;
            let mix = |f: u8, b: u8| (f as f32 * a + b as f32 * (1.0 - a)).round() as u8;
            *p = Rgba([mix(fg.0, bg.0), mix(fg.1, bg.1), mix(fg.2, bg.2), 255]);
        }
    } else if brightness < 1.0 {
        for p in out.pixels_mut() {
            let f = |v: u8| (v as f32 * brightness).round() as u8;
            *p = Rgba([f(p[0]), f(p[1]), f(p[2]), p[3]]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn fade_levels_are_quantised() {
        assert_eq!(fade_level(1.0), FADE_LEVELS);
        assert_eq!(fade_level(0.0), 1);
        assert_eq!(fade_level(0.5), FADE_LEVELS / 2);
    }

    #[test]
    fn pixels_fade_like_text() {
        let theme = Theme::cyberpunk();
        let mut base = RgbaImage::new(2, 1);
        base.put_pixel(0, 0, Rgba([0, 200, 100, 255]));
        base.put_pixel(1, 0, Rgba([0, 0, 0, 0]));
        let faded = recolor(&base, &theme, 0.5, false);
        assert_eq!(faded.get_pixel(0, 0), &Rgba([0, 100, 50, 255]));
        assert_eq!(faded.get_pixel(1, 0)[3], 0, "transparency is preserved");
        let text = crate::features::tui::theme::fade(Color::Rgb(0, 200, 100), 0.5);
        assert_eq!(text, Color::Rgb(0, 100, 50));
    }

    #[test]
    fn selected_rows_use_highlight_colours_on_opaque_background() {
        let theme = Theme::cyberpunk();
        let mut base = RgbaImage::new(2, 1);
        base.put_pixel(0, 0, Rgba([5, 5, 5, 255]));
        base.put_pixel(1, 0, Rgba([5, 5, 5, 0]));
        let sel = recolor(&base, &theme, 1.0, true);
        assert_eq!(sel.get_pixel(0, 0), &Rgba([255, 255, 0, 255]));
        assert_eq!(sel.get_pixel(1, 0), &Rgba([50, 50, 80, 255]));
    }
}
