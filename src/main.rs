mod cli;
mod server;

use crate::cli::{Command, ConvertArgs};
use clap::Parser;
use std::collections::HashMap;
use std::io::Read;
use txdx::error::Result;
use txdx::renderer::FormulaImage;
use txdx::{convert_to_docx_with_cache, ConvertOptions};

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("错误: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = cli::Cli::parse();

    match cli.command {
        Some(Command::Serve(args)) => {
            server::serve(args.host, args.port).await?;
        }
        None => {
            convert_file(cli.convert)?;
        }
    }

    Ok(())
}

fn convert_file(args: ConvertArgs) -> Result<()> {
    let input = read_input(&args.input)?;
    let cache = load_cache(&args.cache)?;
    let options = ConvertOptions {
        dpi: args.dpi,
        font_size: args.font_size,
        formula_scale: args.formula_scale,
        footer: args.footer,
    };

    let (docx_data, new_cache) = convert_to_docx_with_cache(&input, &options, cache)?;
    std::fs::write(&args.output, docx_data)?;
    save_cache(&args.cache, &new_cache)?;

    eprintln!("完成! 输出文件: {}", args.output);
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
