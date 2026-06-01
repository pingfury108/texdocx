mod cli;
mod docx;
mod error;
mod renderer;
mod tokenizer;

use crate::docx::RenderedFormula;
use crate::error::Result;
use crate::renderer::{
    cache_key, CachedRenderer, FormulaImage, FormulaMode, FormulaRenderer, RatexRenderer,
};
use crate::tokenizer::tokenize;
use clap::Parser;
use std::collections::HashMap;
use std::io::Read;

fn main() {
    if let Err(e) = run() {
        eprintln!("错误: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = cli::Cli::parse();

    let input = read_input(&cli.input)?;

    if input.trim().is_empty() {
        return Err(anyhow::anyhow!("输入内容为空").into());
    }

    let tokens = tokenize(&input);

    let formula_count = tokens
        .iter()
        .filter(|t| {
            matches!(
                t,
                tokenizer::Token::InlineFormula(_) | tokenizer::Token::DisplayFormula(_)
            )
        })
        .count();

    eprintln!(
        "解析完成: {} 个 token, {} 个公式",
        tokens.len(),
        formula_count
    );

    let cache = load_cache(&cli.cache)?;
    let ratex = RatexRenderer::new(cli.dpi, cli.font_size);
    let mut renderer = CachedRenderer::new(ratex, cli.dpi, cli.font_size);

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

        let key = cache_key(formula, mode, cli.dpi, cli.font_size);
        if images
            .iter()
            .any(|item| cache_key(&item.formula, item.mode, cli.dpi, cli.font_size) == key)
        {
            continue;
        }

        eprintln!("渲染公式: ${formula}$");
        let image = renderer.render(formula, mode)?;
        images.push(RenderedFormula {
            formula: formula.clone(),
            mode,
            image: image.clone(),
        });
        new_cache.insert(key, image);
    }

    eprintln!("构建 DOCX 文档...");
    let docx_data = docx::build_docx_from_tokens(
        &tokens,
        &images,
        cli.dpi,
        cli.font_size,
        cli.formula_scale,
        cli.footer.as_deref(),
    )?;

    std::fs::write(&cli.output, docx_data)?;

    save_cache(&cli.cache, &new_cache)?;

    eprintln!("完成! 输出文件: {}", cli.output);
    Ok(())
}

fn read_input(path: &Option<String>) -> Result<String> {
    match path {
        Some(p) if p != "-" => Ok(std::fs::read_to_string(p)?),
        _ => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        }
    }
}

fn load_cache(cache_path: &Option<String>) -> Result<HashMap<String, FormulaImage>> {
    let Some(path) = cache_path else {
        return Ok(HashMap::new());
    };

    if !std::path::Path::new(path).exists() {
        return Ok(HashMap::new());
    }

    let data = std::fs::read(path)?;
    let map: HashMap<String, CachedFormula> = match serde_json::from_slice(&data) {
        Ok(map) => map,
        Err(_) => return Ok(HashMap::new()),
    };

    Ok(map
        .into_iter()
        .filter_map(|(k, v)| {
            use base64::Engine;
            let data = base64::engine::general_purpose::STANDARD
                .decode(&v.data)
                .ok()?;
            Some((
                k,
                FormulaImage {
                    data,
                    width_pt: v.width_pt,
                    height_pt: v.height_pt,
                },
            ))
        })
        .collect())
}

fn save_cache(cache_path: &Option<String>, cache: &HashMap<String, FormulaImage>) -> Result<()> {
    let Some(path) = cache_path else {
        return Ok(());
    };

    use base64::Engine;
    let map: HashMap<String, CachedFormula> = cache
        .iter()
        .map(|(k, image)| {
            (
                k.clone(),
                CachedFormula {
                    data: base64::engine::general_purpose::STANDARD.encode(&image.data),
                    width_pt: image.width_pt,
                    height_pt: image.height_pt,
                },
            )
        })
        .collect();

    let data = serde_json::to_vec_pretty(&map)?;
    std::fs::write(path, data)?;

    eprintln!("缓存已保存到: {path}");
    Ok(())
}

#[derive(serde::Deserialize, serde::Serialize)]
struct CachedFormula {
    data: String,
    width_pt: f64,
    height_pt: f64,
}
