use crate::error::{Result, TxdxError};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

pub trait FormulaRenderer {
    fn render(&mut self, formula: &str) -> Result<(Vec<u8>, f64, f64)>;
}

pub struct PdflatexRenderer {
    dpi: u32,
    font_size: u16,
}

impl PdflatexRenderer {
    pub fn new(dpi: u32, font_size: u16) -> Self {
        Self { dpi, font_size }
    }

    fn hash_formula(formula: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(formula.as_bytes());
        format!("{:x}", hasher.finalize())[..16].to_string()
    }

    fn latex_pt(&self) -> u16 {
        self.font_size / 2
    }
}

impl Default for PdflatexRenderer {
    fn default() -> Self {
        Self::new(300, 24)
    }
}

impl FormulaRenderer for PdflatexRenderer {
    fn render(&mut self, formula: &str) -> Result<(Vec<u8>, f64, f64)> {
        let tmp = TempDir::new().map_err(TxdxError::Io)?;
        let base = tmp.path().join(Self::hash_formula(formula));

        let tex_path = base.with_extension("tex");
        let pdf_path = base.with_extension("pdf");
        let png_path = base.with_extension("png");

        let tex_content = format!(
            r#"\documentclass[preview, border=1pt, {pt}pt]{{standalone}}
\usepackage{{amsmath,amssymb,amsfonts,mathrsfs}}
\usepackage{{bm}}
\usepackage{{upgreek}}
\begin{{document}}
$\displaystyle {formula}$
\end{{document}}"#,
            pt = self.latex_pt(),
            formula = formula,
        );

        std::fs::write(&tex_path, tex_content)?;

        let output = Command::new("pdflatex")
            .args([
                "-interaction=nonstopmode",
                "-halt-on-error",
                &tex_path.to_string_lossy(),
            ])
            .current_dir(tmp.path())
            .output()
            .map_err(TxdxError::Io)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(TxdxError::LatexCompile {
                formula: formula.to_string(),
                stderr: format!("stdout:\n{stdout}\nstderr:\n{stderr}"),
            });
        }

        if !pdf_path.exists() {
            return Err(TxdxError::LatexCompile {
                formula: formula.to_string(),
                stderr: "PDF was not generated".to_string(),
            });
        }

        let (width_pt, height_pt) = get_pdf_dimensions(&pdf_path)?;

        pdf_to_png(&pdf_path, &png_path, self.dpi)?;

        let png_bytes = std::fs::read(&png_path)?;
        Ok((png_bytes, width_pt, height_pt))
    }
}

fn get_pdf_dimensions(pdf: &Path) -> Result<(f64, f64)> {
    let output = Command::new("pdfinfo")
        .arg(pdf)
        .output();

    if let Ok(o) = output {
        if o.status.success() {
            let stdout = String::from_utf8_lossy(&o.stdout);
            for line in stdout.lines() {
                if let Some(rest) = line.strip_prefix("Page size:") {
                    let parts: Vec<&str> = rest.trim().split_whitespace().collect();
                    if parts.len() >= 3 {
                        let w: f64 = parts[0].parse().unwrap_or(0.0);
                        let h: f64 = parts[2].parse().unwrap_or(0.0);
                        if w > 0.0 && h > 0.0 {
                            return Ok((w, h));
                        }
                    }
                }
            }
        }
    }

    let data = std::fs::read(pdf)?;
    for i in 0..data.len().saturating_sub(20) {
        if &data[i..i+9] == b"/MediaBox" {
            let slice = &data[i..];
            if let Some(start) = slice.iter().position(|&b| b == b'[') {
                let inner = &slice[start+1..];
                if let Some(end) = inner.iter().position(|&b| b == b']') {
                    let nums: Vec<f64> = std::str::from_utf8(&inner[..end])
                        .unwrap_or("")
                        .split_whitespace()
                        .filter_map(|s| s.parse().ok())
                        .collect();
                    if nums.len() >= 4 {
                        return Ok((nums[2] - nums[0], nums[3] - nums[1]));
                    }
                }
            }
        }
    }

    Err(TxdxError::Anyhow(anyhow::anyhow!(
        "无法读取 PDF 页面尺寸，请安装 pdfinfo (poppler-utils)"
    )))
}

fn pdf_to_png(pdf: &Path, png: &Path, dpi: u32) -> Result<()> {
    let stem = png.with_extension("");

    // pdftoppm — best quality, respects DPI
    let output = Command::new("pdftoppm")
        .args([
            "-png",
            "-r",
            &dpi.to_string(),
            "-singlefile",
            &pdf.to_string_lossy(),
            &stem.to_string_lossy(),
        ])
        .output();

    if let Ok(o) = output {
        if o.status.success() {
            return Ok(());
        }
    }

    // Ghostscript — good quality, respects DPI
    let output = Command::new("gs")
        .args([
            "-dSAFER",
            "-dBATCH",
            "-dNOPAUSE",
            "-dTextAlphaBits=4",
            "-dGraphicsAlphaBits=4",
            "-sDEVICE=png16m",
            &format!("-r{dpi}"),
            &format!("-sOutputFile={}", png.to_string_lossy()),
            &pdf.to_string_lossy(),
        ])
        .output();

    if let Ok(o) = output {
        if o.status.success() {
            return Ok(());
        }
    }

    // sips — macOS built-in, 72 DPI only, low quality fallback
    #[cfg(target_os = "macos")]
    {
        let result = Command::new("sips")
            .args([
                "-s", "format", "png",
                &pdf.to_string_lossy(),
                "--out", &png.to_string_lossy(),
            ])
            .output();

        if let Ok(output) = result {
            if output.status.success() {
                return Ok(());
            }
        }
    }

    Err(TxdxError::NoPdfConverter)
}

pub struct CachedRenderer<R: FormulaRenderer> {
    inner: R,
    cache: HashMap<String, (Vec<u8>, f64, f64)>,
}

impl<R: FormulaRenderer> CachedRenderer<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            cache: HashMap::new(),
        }
    }

    pub fn warm(&mut self, key: &str, data: Vec<u8>) {
        self.cache
            .insert(key.to_string(), (data, 100.0, 20.0));
    }
}

impl<R: FormulaRenderer> FormulaRenderer for CachedRenderer<R> {
    fn render(&mut self, formula: &str) -> Result<(Vec<u8>, f64, f64)> {
        let key = formula.to_string();
        if let Some(data) = self.cache.get(&key) {
            return Ok(data.clone());
        }

        let result = self.inner.render(formula)?;
        self.cache.insert(key, result.clone());
        Ok(result)
    }
}
