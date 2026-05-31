use crate::error::Result;
use crate::tokenizer::Token;
use std::io::{Cursor, Write};
use zip::write::FileOptions;
use zip::ZipWriter;

pub enum ParagraphRun {
    Text(String),
    Image {
        data: Vec<u8>,
        width_pt: f64,
        height_pt: f64,
    },
    LineBreak,
}

struct ImageEntry {
    data: Vec<u8>,
    filename: String,
    rid: String,
}

pub struct DocxBuilder {
    body_xml: String,
    images: Vec<ImageEntry>,
    image_counter: usize,
    font_size: u16,
    formula_scale: f64,
    has_footer: bool,
    footer_text: String,
}

impl DocxBuilder {
    pub fn new(_dpi: u32) -> Self {
        Self {
            body_xml: String::new(),
            images: Vec::new(),
            image_counter: 0,
            font_size: 24,
            formula_scale: 1.0,
            has_footer: false,
            footer_text: String::new(),
        }
    }

    pub fn with_font_size(mut self, size: u16) -> Self {
        self.font_size = size;
        self
    }

    pub fn with_formula_scale(mut self, scale: f64) -> Self {
        self.formula_scale = scale;
        self
    }

    fn position_offset(&self) -> i16 {
        -((self.font_size as i16) / 6)
    }

    fn pt_to_emu(&self, pt: f64) -> u32 {
        (pt * 12700.0 * self.formula_scale) as u32
    }

    pub fn add_mixed_paragraph(&mut self, runs: Vec<ParagraphRun>) {
        if runs.iter().all(|r| matches!(r, ParagraphRun::LineBreak)) {
            return;
        }
        self.body_xml.push_str("    <w:p>\n");

        for run in runs {
            match run {
                ParagraphRun::Text(text) => {
                    let escaped = escape_xml(&text);
                    self.body_xml.push_str(&format!(
                        r#"      <w:r>
        <w:rPr>
          <w:rFonts w:eastAsia="SimSun"/>
          <w:sz w:val="{}"/>
        </w:rPr>
        <w:t xml:space="preserve">{}</w:t>
      </w:r>
"#,
                        self.font_size, escaped
                    ));
                }
                ParagraphRun::LineBreak => {
                    self.body_xml.push_str("      <w:r><w:br/></w:r>\n");
                }
                ParagraphRun::Image {
                    data,
                    width_pt,
                    height_pt,
                } => {
                    self.image_counter += 1;
                    let rid = format!("rId{}", self.image_counter);
                    let filename = format!("media/image{}.png", self.image_counter);

                    let width_emu = self.pt_to_emu(width_pt);
                    let height_emu = self.pt_to_emu(height_pt);
                    let pos = self.position_offset();

                    self.body_xml.push_str(&format!(
                        r#"      <w:r>
        <w:rPr>
          <w:position w:val="{pos}"/>
        </w:rPr>
        <w:drawing>
          <wp:inline distT="0" distB="0" distL="0" distR="0">
            <wp:extent cx="{cx}" cy="{cy}"/>
            <wp:effectExtent l="0" t="0" r="0" b="0"/>
            <wp:docPr id="{id}" name="formula_{id}"/>
            <a:graphic>
              <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
                <pic:pic>
                  <pic:nvPicPr>
                    <pic:cNvPr id="0" name="formula_{id}"/>
                    <pic:cNvPicPr/>
                  </pic:nvPicPr>
                  <pic:blipFill>
                    <a:blip r:embed="{rid}"/>
                    <a:stretch>
                      <a:fillRect/>
                    </a:stretch>
                  </pic:blipFill>
                  <pic:spPr>
                    <a:xfrm>
                      <a:off x="0" y="0"/>
                      <a:ext cx="{cx}" cy="{cy}"/>
                    </a:xfrm>
                    <a:prstGeom prst="rect"/>
                  </pic:spPr>
                </pic:pic>
              </a:graphicData>
            </a:graphic>
          </wp:inline>
        </w:drawing>
      </w:r>
"#,
                        pos = pos,
                        cx = width_emu,
                        cy = height_emu,
                        id = self.image_counter,
                        rid = rid,
                    ));

                    self.images.push(ImageEntry {
                        data,
                        filename,
                        rid,
                    });
                }
            }
        }

        self.body_xml.push_str("    </w:p>\n");
    }

    pub fn add_display_formula_paragraph(&mut self, image_data: Vec<u8>, width_pt: f64, height_pt: f64) {
        self.image_counter += 1;
        let rid = format!("rId{}", self.image_counter);
        let filename = format!("media/image{}.png", self.image_counter);

        let width_emu = self.pt_to_emu(width_pt);
        let height_emu = self.pt_to_emu(height_pt);

        self.body_xml.push_str(&format!(
            r#"    <w:p>
      <w:pPr>
        <w:jc w:val="center"/>
      </w:pPr>
      <w:r>
        <w:drawing>
          <wp:inline distT="0" distB="0" distL="0" distR="0">
            <wp:extent cx="{cx}" cy="{cy}"/>
            <wp:effectExtent l="0" t="0" r="0" b="0"/>
            <wp:docPr id="{id}" name="formula_{id}"/>
            <a:graphic>
              <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
                <pic:pic>
                  <pic:nvPicPr>
                    <pic:cNvPr id="0" name="formula_{id}"/>
                    <pic:cNvPicPr/>
                  </pic:nvPicPr>
                  <pic:blipFill>
                    <a:blip r:embed="{rid}"/>
                    <a:stretch>
                      <a:fillRect/>
                    </a:stretch>
                  </pic:blipFill>
                  <pic:spPr>
                    <a:xfrm>
                      <a:off x="0" y="0"/>
                      <a:ext cx="{cx}" cy="{cy}"/>
                    </a:xfrm>
                    <a:prstGeom prst="rect"/>
                  </pic:spPr>
                </pic:pic>
              </a:graphicData>
            </a:graphic>
          </wp:inline>
        </w:drawing>
      </w:r>
    </w:p>
"#,
            cx = width_emu,
            cy = height_emu,
            id = self.image_counter,
            rid = rid,
        ));

        self.images.push(ImageEntry {
            data: image_data,
            filename,
            rid,
        });
    }

    pub fn add_footer(&mut self, text: &str) {
        self.has_footer = true;
        self.footer_text = text.to_string();
    }

    pub fn build(self) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut buf);
            let options = FileOptions::<()>::default()
                .compression_method(zip::CompressionMethod::Deflated);

            zip.start_file("[Content_Types].xml", options)?;
            zip.write_all(generate_content_types(&self.images).as_bytes())?;

            zip.start_file("_rels/.rels", options)?;
            zip.write_all(generate_rels().as_bytes())?;

            zip.start_file("word/_rels/document.xml.rels", options)?;
            zip.write_all(generate_document_rels(&self.images).as_bytes())?;

            zip.start_file("word/document.xml", options)?;
            zip.write_all(generate_document_xml(&self.body_xml, self.has_footer, &self.footer_text).as_bytes())?;

            for img in &self.images {
                zip.start_file(format!("word/{}", img.filename), options)?;
                zip.write_all(&img.data)?;
            }

            zip.finish()?;
        }
        Ok(buf.into_inner())
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn generate_content_types(images: &[ImageEntry]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
"#,
    );

    for img in images {
        xml.push_str(&format!(
            r#"  <Override PartName="/word/{}" ContentType="image/png"/>
"#,
            img.filename
        ));
    }

    xml.push_str("</Types>");
    xml
}

fn generate_rels() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId0" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#
        .to_string()
}

fn generate_document_rels(images: &[ImageEntry]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
"#,
    );

    for img in images {
        xml.push_str(&format!(
            r#"  <Relationship Id="{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="{}"/>
"#,
            img.rid, img.filename
        ));
    }

    xml.push_str("</Relationships>");
    xml
}

fn generate_document_xml(body: &str, has_footer: bool, footer_text: &str) -> String {
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:wpc="http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas"
            xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
            xmlns:o="urn:schemas-microsoft-com:office:office"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"
            xmlns:v="urn:schemas-microsoft-com:vml"
            xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
            xmlns:w10="urn:schemas-microsoft-com:office:word"
            xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:wne="http://schemas.microsoft.com/office/word/2006/wordml"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
            xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
  <w:body>
{}
  </w:body>
</w:document>"#,
        body
    );

    if has_footer {
        let escaped = escape_xml(footer_text);
        let footer_para = format!(
            r#"    <w:p>
      <w:pPr>
        <w:rPr>
          <w:color w:val="808080"/>
          <w:sz w:val="16"/>
        </w:rPr>
      </w:pPr>
      <w:r>
        <w:rPr>
          <w:color w:val="808080"/>
          <w:sz w:val="16"/>
        </w:rPr>
        <w:t xml:space="preserve">{}</w:t>
      </w:r>
    </w:p>
"#,
            escaped
        );
        xml = xml.replace("  </w:body>", &format!("{}\n  </w:body>", footer_para));
    }

    xml
}

pub fn build_docx_from_tokens(
    tokens: &[Token],
    images: &[(String, Vec<u8>, f64, f64)],
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

    let image_map: std::collections::HashMap<&str, (f64, f64)> = {
        let mut m = std::collections::HashMap::new();
        for (formula, _data, w, h) in images {
            m.insert(formula.as_str(), (*w, *h));
        }
        m
    };

    let image_data_map: std::collections::HashMap<&str, &Vec<u8>> =
        images.iter().map(|(k, v, _, _)| (k.as_str(), v)).collect();

    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Token::DisplayFormula(formula) => {
                if let Some((w, h)) = image_map.get(formula.as_str()) {
                    if let Some(data) = image_data_map.get(formula.as_str()) {
                        builder.add_display_formula_paragraph((*data).clone(), *w, *h);
                    }
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
                        Token::InlineFormula(f) => {
                            if let Some((w, h)) = image_map.get(f.as_str()) {
                                if let Some(data) = image_data_map.get(f.as_str()) {
                                    runs.push(ParagraphRun::Image {
                                        data: (*data).clone(),
                                        width_pt: *w,
                                        height_pt: *h,
                                    });
                                }
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
