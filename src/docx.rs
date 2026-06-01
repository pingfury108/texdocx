use crate::error::Result;
use crate::renderer::{cache_key, FormulaImage, FormulaMode};
use crate::tokenizer::Token;
use docx_rs::{AlignmentType, BreakType, Docx, Paragraph, Pic, Run, RunFonts};
use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use zip::write::FileOptions;
use zip::{ZipArchive, ZipWriter};

const INLINE_FORMULA_MARKER: &str = "txdx_formula_inline";
const INLINE_FORMULA_POSITION: i16 = -6;

pub struct RenderedFormula {
    pub formula: String,
    pub mode: FormulaMode,
    pub image: FormulaImage,
}

enum ParagraphRun {
    Text(String),
    InlineImage(FormulaImage),
    LineBreak,
}

struct DocxBuilder {
    docx: Docx,
    font_size: u16,
    formula_scale: f64,
    footer_text: Option<String>,
    inline_image_counter: usize,
}

impl DocxBuilder {
    fn new(_dpi: u32) -> Self {
        Self {
            docx: Docx::new().default_fonts(body_fonts()).default_size(24),
            font_size: 24,
            formula_scale: 1.0,
            footer_text: None,
            inline_image_counter: 0,
        }
    }

    fn with_font_size(mut self, size: u16) -> Self {
        self.font_size = size;
        self.docx = self.docx.default_size(size as usize);
        self
    }

    fn with_formula_scale(mut self, scale: f64) -> Self {
        self.formula_scale = scale;
        self
    }

    fn pt_to_emu(&self, pt: f64) -> u32 {
        (pt * 12700.0 * self.formula_scale).round() as u32
    }

    fn text_run(&self, text: String) -> Run {
        Run::new()
            .add_text(text)
            .fonts(body_fonts())
            .size(self.font_size as usize)
    }

    fn image_run(&mut self, image: FormulaImage, inline: bool) -> Run {
        let mut pic = Pic::new(&image.data).size(
            self.pt_to_emu(image.width_pt),
            self.pt_to_emu(image.height_pt),
        );

        if inline {
            self.inline_image_counter += 1;
            pic = pic.id(format!(
                "{INLINE_FORMULA_MARKER}_{}",
                self.inline_image_counter
            ));
        }

        Run::new().add_image(pic)
    }

    fn add_mixed_paragraph(&mut self, runs: Vec<ParagraphRun>) {
        if runs.iter().all(|r| matches!(r, ParagraphRun::LineBreak)) {
            return;
        }

        let mut paragraph = Paragraph::new();
        for run in runs {
            paragraph = match run {
                ParagraphRun::Text(text) => paragraph.add_run(self.text_run(text)),
                ParagraphRun::LineBreak => {
                    paragraph.add_run(Run::new().add_break(BreakType::TextWrapping))
                }
                ParagraphRun::InlineImage(image) => paragraph.add_run(self.image_run(image, true)),
            };
        }

        self.docx = std::mem::take(&mut self.docx).add_paragraph(paragraph);
    }

    fn add_display_formula_paragraph(&mut self, image: FormulaImage) {
        let paragraph = Paragraph::new()
            .align(AlignmentType::Center)
            .add_run(self.image_run(image, false));
        self.docx = std::mem::take(&mut self.docx).add_paragraph(paragraph);
    }

    fn add_footer(&mut self, text: &str) {
        self.footer_text = Some(text.to_string());
    }

    fn build(mut self) -> Result<Vec<u8>> {
        if let Some(text) = self.footer_text {
            let paragraph = Paragraph::new().add_run(
                Run::new()
                    .add_text(text)
                    .fonts(body_fonts())
                    .size(16)
                    .color("808080"),
            );
            self.docx = self.docx.add_paragraph(paragraph);
        }

        let mut buf = Cursor::new(Vec::new());
        self.docx
            .build()
            .pack(&mut buf)
            .map_err(|e| anyhow::anyhow!("DOCX 打包失败: {e}"))?;
        patch_inline_formula_position(buf.into_inner(), INLINE_FORMULA_POSITION)
    }
}

fn body_fonts() -> RunFonts {
    RunFonts::new().east_asia("SimSun")
}

pub fn build_docx_from_tokens(
    tokens: &[Token],
    images: &[RenderedFormula],
    dpi: u32,
    font_size: u16,
    formula_scale: f64,
    footer: Option<&str>,
) -> Result<Vec<u8>> {
    let mut builder = DocxBuilder::new(dpi)
        .with_font_size(font_size)
        .with_formula_scale(formula_scale);

    if let Some(footer_text) = footer {
        builder.add_footer(footer_text);
    }

    let image_map: HashMap<String, FormulaImage> = images
        .iter()
        .map(|item| {
            (
                cache_key(&item.formula, item.mode, dpi, font_size),
                item.image.clone(),
            )
        })
        .collect();

    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Token::DisplayFormula(formula) => {
                let key = cache_key(formula, FormulaMode::Display, dpi, font_size);
                if let Some(image) = image_map.get(&key) {
                    builder.add_display_formula_paragraph(image.clone());
                }
                i += 1;
            }
            Token::Text(_) | Token::InlineFormula(_) => {
                let mut runs: Vec<ParagraphRun> = Vec::new();
                while i < tokens.len() {
                    match &tokens[i] {
                        Token::Text(t) => {
                            let paras: Vec<&str> = t.split("\n\n").collect();
                            if paras.len() > 1 {
                                for (pi, para) in paras.iter().enumerate() {
                                    if pi > 0 && !runs.is_empty() {
                                        builder.add_mixed_paragraph(std::mem::take(&mut runs));
                                    }
                                    append_text_with_linebreaks(&mut runs, para);
                                }
                            } else {
                                append_text_with_linebreaks(&mut runs, t);
                            }
                            i += 1;
                        }
                        Token::InlineFormula(formula) => {
                            let key = cache_key(formula, FormulaMode::Inline, dpi, font_size);
                            if let Some(image) = image_map.get(&key) {
                                runs.push(ParagraphRun::InlineImage(image.clone()));
                            }
                            i += 1;
                        }
                        Token::DisplayFormula(_) => break,
                    }
                }
                if !runs.is_empty() {
                    builder.add_mixed_paragraph(runs);
                }
            }
        }
    }

    builder.build()
}

fn append_text_with_linebreaks(runs: &mut Vec<ParagraphRun>, text: &str) {
    if text.is_empty() {
        return;
    }
    let lines: Vec<&str> = text.split('\n').collect();
    for (li, line) in lines.iter().enumerate() {
        if li > 0 {
            runs.push(ParagraphRun::LineBreak);
        }
        if !line.is_empty() {
            runs.push(ParagraphRun::Text(line.to_string()));
        }
    }
}

fn patch_inline_formula_position(docx: Vec<u8>, position: i16) -> Result<Vec<u8>> {
    let mut input =
        ZipArchive::new(Cursor::new(docx)).map_err(|e| anyhow::anyhow!("DOCX 读取失败: {e}"))?;
    let mut output = Cursor::new(Vec::new());

    {
        let mut writer = ZipWriter::new(&mut output);
        for i in 0..input.len() {
            let mut file = input
                .by_index(i)
                .map_err(|e| anyhow::anyhow!("DOCX 条目读取失败: {e}"))?;
            let name = file.name().to_string();
            let options = FileOptions::<()>::default().compression_method(file.compression());

            if file.is_dir() {
                writer
                    .add_directory(name, options)
                    .map_err(|e| anyhow::anyhow!("DOCX 目录写入失败: {e}"))?;
                continue;
            }

            let mut data = Vec::new();
            file.read_to_end(&mut data)?;

            if name == "word/document.xml" {
                let xml = String::from_utf8(data)
                    .map_err(|e| anyhow::anyhow!("document.xml 不是有效 UTF-8: {e}"))?;
                data = patch_document_xml(&xml, position).into_bytes();
            }

            writer
                .start_file(name, options)
                .map_err(|e| anyhow::anyhow!("DOCX 条目写入失败: {e}"))?;
            writer.write_all(&data)?;
        }

        writer
            .finish()
            .map_err(|e| anyhow::anyhow!("DOCX 写入完成失败: {e}"))?;
    }

    Ok(output.into_inner())
}

fn patch_document_xml(xml: &str, position: i16) -> String {
    let mut output = String::with_capacity(xml.len() + 64);
    let mut cursor = 0;

    while let Some(marker_offset) = xml[cursor..].find(INLINE_FORMULA_MARKER) {
        let marker = cursor + marker_offset;
        let Some(run_start_rel) = xml[..marker].rfind("<w:r") else {
            break;
        };
        let Some(run_end_rel) = xml[marker..].find("</w:r>") else {
            break;
        };
        let run_end = marker + run_end_rel + "</w:r>".len();

        if run_start_rel < cursor {
            cursor = marker + INLINE_FORMULA_MARKER.len();
            continue;
        }

        output.push_str(&xml[cursor..run_start_rel]);
        output.push_str(&patch_run_position(&xml[run_start_rel..run_end], position));
        cursor = run_end;
    }

    output.push_str(&xml[cursor..]);
    output
}

fn patch_run_position(run_xml: &str, position: i16) -> String {
    if run_xml.contains("<w:position ") {
        return run_xml.to_string();
    }

    let position_xml = format!(r#"<w:position w:val="{position}"/>"#);

    if let Some(rpr_start) = run_xml.find("<w:rPr") {
        let Some(start_tag_end_rel) = run_xml[rpr_start..].find('>') else {
            return run_xml.to_string();
        };
        let start_tag_end = rpr_start + start_tag_end_rel;

        if run_xml[..=start_tag_end].ends_with("/>") {
            let mut patched = String::with_capacity(run_xml.len() + position_xml.len() + 8);
            patched.push_str(&run_xml[..start_tag_end - 1]);
            patched.push('>');
            patched.push_str(&position_xml);
            patched.push_str("</w:rPr>");
            patched.push_str(&run_xml[start_tag_end + 1..]);
            return patched;
        }

        let mut patched = String::with_capacity(run_xml.len() + position_xml.len());
        patched.push_str(&run_xml[..start_tag_end + 1]);
        patched.push_str(&position_xml);
        patched.push_str(&run_xml[start_tag_end + 1..]);
        return patched;
    }

    let Some(run_start_end) = run_xml.find('>') else {
        return run_xml.to_string();
    };

    let mut patched = String::with_capacity(run_xml.len() + position_xml.len() + 16);
    patched.push_str(&run_xml[..run_start_end + 1]);
    patched.push_str("<w:rPr>");
    patched.push_str(&position_xml);
    patched.push_str("</w:rPr>");
    patched.push_str(&run_xml[run_start_end + 1..]);
    patched
}
