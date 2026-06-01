use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "txdx",
    version,
    about = "Convert mixed text/LaTeX math documents to DOCX with inline formula images",
    long_about = "Converts a document containing plain text and LaTeX math formulas (delimited by $) into a Word .docx file, rendering formulas as high-quality PNG images embedded inline."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub convert: ConvertArgs,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Start the HTTP API server
    Serve(ServeArgs),
}

#[derive(Parser, Debug, Clone)]
pub struct ConvertArgs {
    /// Input file (text with $formula$ markup). Reads from stdin if omitted.
    pub input: Option<String>,

    /// Output DOCX file path
    #[arg(short = 'o', long, default_value = "output.docx")]
    pub output: String,

    /// DPI for formula image rendering
    #[arg(short = 'd', long, default_value = "200")]
    pub dpi: u32,

    /// Footer text appended at the end of the document
    #[arg(short = 'f', long)]
    pub footer: Option<String>,

    /// Font size for body text (in half-points, 24 = 12pt)
    #[arg(short = 's', long, default_value = "24")]
    pub font_size: u16,

    /// Scale factor for formula image size (1.0 = natural size, 0.8 = 80%)
    #[arg(long, default_value = "1.0")]
    pub formula_scale: f64,

    /// Path to a JSON cache file for formula images (speeds up re-runs)
    #[arg(long)]
    pub cache: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct ServeArgs {
    /// Host address to bind
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to listen on
    #[arg(short = 'p', long, default_value = "3000")]
    pub port: u16,
}
