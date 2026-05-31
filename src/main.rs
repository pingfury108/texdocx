mod cli;
mod docx;
mod error;
mod renderer;
mod tokenizer;

use crate::error::Result;
use crate::renderer::{CachedRenderer, FormulaRenderer, PdflatexRenderer};
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
        .filter(|t| matches!(t, tokenizer::Token::InlineFormula(_) | tokenizer::Token::DisplayFormula(_)))
        .count();

    eprintln!("解析完成: {} 个 token, {} 个公式", tokens.len(), formula_count);

    let cache = load_cache(&cli.cache)?;
    let pdflatex = PdflatexRenderer::new(cli.dpi, cli.font_size);
    let mut renderer = CachedRenderer::new(pdflatex);

    for (key, data) in &cache {
        renderer.warm(key, data.clone());
    }

    let mut new_cache: HashMap<String, Vec<u8>> = HashMap::new();
    let mut images: Vec<(String, Vec<u8>, f64, f64)> = Vec::new();

    for token in &tokens {
        let formula = match token {
            tokenizer::Token::InlineFormula(f) | tokenizer::Token::DisplayFormula(f) => f,
            _ => continue,
        };

        if images.iter().any(|(f, _, _, _)| f == formula) {
            continue;
        }

        eprintln!("渲染公式: ${formula}$");
        let (png_data, w_pt, h_pt) = renderer.render(formula)?;
        images.push((formula.clone(), png_data.clone(), w_pt, h_pt));
        new_cache.insert(formula.clone(), png_data);
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

fn load_cache(cache_path: &Option<String>) -> Result<HashMap<String, Vec<u8>>> {
    let Some(path) = cache_path else {
        return Ok(HashMap::new());
    };

    if !std::path::Path::new(path).exists() {
        return Ok(HashMap::new());
    }

    let data = std::fs::read(path)?;
    let map: HashMap<String, String> = serde_json::from_slice(&data)
        .map_err(|e| anyhow::anyhow!("缓存文件解析失败: {e}"))?;

    Ok(map
        .into_iter()
        .map(|(k, v)| {
            use base64::Engine;
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(&v)
                .unwrap_or_default();
            (k, decoded)
        })
        .collect())
}

fn save_cache(cache_path: &Option<String>, cache: &HashMap<String, Vec<u8>>) -> Result<()> {
    let Some(path) = cache_path else {
        return Ok(());
    };

    use base64::Engine;
    let map: HashMap<String, String> = cache
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                base64::engine::general_purpose::STANDARD.encode(v),
            )
        })
        .collect();

    let data = serde_json::to_vec_pretty(&map)?;
    std::fs::write(path, data)?;

    eprintln!("缓存已保存到: {path}");
    Ok(())
}
