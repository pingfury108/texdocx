use thiserror::Error;

#[derive(Error, Debug)]
pub enum TxdxError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("LaTeX compilation failed for `{formula}`:\n{stderr}")]
    LatexCompile {
        formula: String,
        stderr: String,
    },

    #[allow(dead_code)]
    #[error("PDF to PNG conversion failed: {0}")]
    PdfConversion(String),

    #[allow(dead_code)]
    #[error("No PDF to PNG converter found. Install pdftoppm (poppler-utils), ghostscript, or use macOS built-in sips.")]
    NoPdfConverter,

    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, TxdxError>;
