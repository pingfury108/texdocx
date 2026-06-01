pub mod docx;
pub mod error;
pub mod renderer;
pub mod tokenizer;

use crate::docx::RenderedFormula;
use crate::error::Result;
use crate::renderer::{CachedRenderer, FormulaImage, FormulaMode, FormulaRenderer, RatexRenderer};
use crate::tokenizer::tokenize;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ConvertOptions {
    pub dpi: u32,
    pub font_size: u16,
    pub formula_scale: f64,
    pub footer: Option<String>,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            dpi: 200,
            font_size: 24,
            formula_scale: 1.0,
            footer: None,
        }
    }
}

pub fn convert_to_docx(input: &str, options: &ConvertOptions) -> Result<Vec<u8>> {
    convert_to_docx_with_cache(input, options, HashMap::new()).map(|(docx, _)| docx)
}

pub fn convert_to_docx_with_cache(
    input: &str,
    options: &ConvertOptions,
    cache: HashMap<String, FormulaImage>,
) -> Result<(Vec<u8>, HashMap<String, FormulaImage>)> {
    if input.trim().is_empty() {
        return Err(anyhow::anyhow!("输入内容为空").into());
    }

    let tokens = tokenize(input);
    let ratex = RatexRenderer::new(options.dpi, options.font_size);
    let mut renderer = CachedRenderer::new(ratex, options.dpi, options.font_size);

    for (key, image) in &cache {
        renderer.warm(key, image.clone());
    }

    let mut new_cache: HashMap<String, FormulaImage> = HashMap::new();
    let mut images: Vec<RenderedFormula> = Vec::new();

    for token in &tokens {
        let (formula, mode) = match token {
            tokenizer::Token::InlineFormula(f) => (f, FormulaMode::Inline),
            tokenizer::Token::DisplayFormula(f) => (f, FormulaMode::Display),
            _ => continue,
        };

        let key = renderer::cache_key(formula, mode, options.dpi, options.font_size);
        if images.iter().any(|item| {
            renderer::cache_key(&item.formula, item.mode, options.dpi, options.font_size) == key
        }) {
            continue;
        }

        let image = renderer.render(formula, mode)?;
        images.push(RenderedFormula {
            formula: formula.clone(),
            mode,
            image: image.clone(),
        });
        new_cache.insert(key, image);
    }

    let docx = docx::build_docx_from_tokens(
        &tokens,
        &images,
        options.dpi,
        options.font_size,
        options.formula_scale,
        options.footer.as_deref(),
    )?;

    Ok((docx, new_cache))
}
