use thiserror::Error;

#[derive(Error, Debug)]
pub enum TxdxError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Formula render failed for `{formula}`:\n{message}")]
    FormulaRender { formula: String, message: String },

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, TxdxError>;
