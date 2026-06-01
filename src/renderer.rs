use crate::error::{Result, TxdxError};
use image::GenericImageView;
use ratex_layout::{layout, to_display_list, LayoutOptions};
use ratex_parser::parser::parse;
use ratex_render::{render_to_png, RenderOptions};
use ratex_types::color::Color;
use ratex_types::math_style::MathStyle;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FormulaMode {
    Inline,
    Display,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormulaImage {
    pub data: Vec<u8>,
    pub width_pt: f64,
    pub height_pt: f64,
}

pub trait FormulaRenderer {
    fn render(&mut self, formula: &str, mode: FormulaMode) -> Result<FormulaImage>;
}

pub struct RatexRenderer {
    dpi: u32,
    font_size: u16,
}

impl RatexRenderer {
    pub fn new(dpi: u32, font_size: u16) -> Self {
        Self { dpi, font_size }
    }

    fn dpr(&self) -> f32 {
        (self.dpi as f32 / 96.0).clamp(0.25, 8.0)
    }

    fn font_size_px(&self) -> f32 {
        let pt = self.font_size as f32 / 2.0;
        pt * 96.0 / 72.0
    }

    fn px_to_pt(&self, px: u32) -> f64 {
        px as f64 / self.dpr() as f64 * 72.0 / 96.0
    }
}

impl Default for RatexRenderer {
    fn default() -> Self {
        Self::new(200, 24)
    }
}

impl FormulaRenderer for RatexRenderer {
    fn render(&mut self, formula: &str, mode: FormulaMode) -> Result<FormulaImage> {
        let ast = parse(formula).map_err(|e| TxdxError::FormulaRender {
            formula: formula.to_string(),
            message: format!("RaTeX parse error: {e}"),
        })?;

        let style = match mode {
            FormulaMode::Inline => MathStyle::Text,
            FormulaMode::Display => MathStyle::Display,
        };
        let layout_options = LayoutOptions::default().with_style(style);
        let layout_box = layout(&ast, &layout_options);
        let display_list = to_display_list(&layout_box);

        let render_options = RenderOptions {
            font_size: self.font_size_px(),
            padding: 1.0,
            background_color: Color::new(0.0, 0.0, 0.0, 0.0),
            font_dir: String::new(),
            device_pixel_ratio: self.dpr(),
        };

        let data = render_to_png(&display_list, &render_options).map_err(|e| {
            TxdxError::FormulaRender {
                formula: formula.to_string(),
                message: format!("RaTeX render error: {e}"),
            }
        })?;

        let image = image::load_from_memory(&data).map_err(|e| TxdxError::FormulaRender {
            formula: formula.to_string(),
            message: format!("PNG decode error: {e}"),
        })?;
        let (width_px, height_px) = image.dimensions();

        Ok(FormulaImage {
            data,
            width_pt: self.px_to_pt(width_px),
            height_pt: self.px_to_pt(height_px),
        })
    }
}

pub struct CachedRenderer<R: FormulaRenderer> {
    inner: R,
    cache: HashMap<String, FormulaImage>,
    dpi: u32,
    font_size: u16,
}

impl<R: FormulaRenderer> CachedRenderer<R> {
    pub fn new(inner: R, dpi: u32, font_size: u16) -> Self {
        Self {
            inner,
            cache: HashMap::new(),
            dpi,
            font_size,
        }
    }

    pub fn warm(&mut self, key: &str, image: FormulaImage) {
        self.cache.insert(key.to_string(), image);
    }
}

impl<R: FormulaRenderer> FormulaRenderer for CachedRenderer<R> {
    fn render(&mut self, formula: &str, mode: FormulaMode) -> Result<FormulaImage> {
        let key = cache_key(formula, mode, self.dpi, self.font_size);
        if let Some(image) = self.cache.get(&key) {
            return Ok(image.clone());
        }

        let result = self.inner.render(formula, mode)?;
        self.cache.insert(key, result.clone());
        Ok(result)
    }
}

pub fn cache_key(formula: &str, mode: FormulaMode, dpi: u32, font_size: u16) -> String {
    let mode = match mode {
        FormulaMode::Inline => "inline",
        FormulaMode::Display => "display",
    };
    format!("ratex:dpi={dpi}:font_size={font_size}:mode={mode}:{formula}")
}
