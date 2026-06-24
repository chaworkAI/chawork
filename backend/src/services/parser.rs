//! Parser Service — pure-Rust text extraction for the document types supported
//! by V1 (DESIGN §3 / §9). No subprocesses, no external CLI dependencies.
//!
//! Supported source types and crates:
//!
//! | type | crate(s)                          |
//! |------|-----------------------------------|
//! | TXT  | `std::fs`                         |
//! | MD   | `pulldown-cmark`                  |
//! | DOCX | `zip` + `quick-xml`               |
//! | XLSX | `calamine`                        |
//! | CSV  | `csv` + `serde`                   |
//! | PDF  | `pdf-extract`                     |
//!
//! Anything outside this set is rejected (`SourceType::Unsupported`); upstream
//! UI is responsible for not creating import tasks for those.

use std::fs;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseResult {
    pub text: String,
    pub source_type: SourceType,
    /// Identifier of the crate (and version, when stable) that produced the
    /// markdown. Surfaced into wiki frontmatter as the `parser` field.
    pub parser: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Text,
    Markdown,
    Docx,
    Xlsx,
    Csv,
    Pdf,
    /// Reject path: caller must not create an import task for this type.
    Unsupported,
}

impl SourceType {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "txt" => Self::Text,
            "md" | "markdown" => Self::Markdown,
            "docx" => Self::Docx,
            "xlsx" => Self::Xlsx,
            "csv" => Self::Csv,
            "pdf" => Self::Pdf,
            _ => Self::Unsupported,
        }
    }

    pub fn raw_subdirectory(self) -> &'static str {
        match self {
            Self::Text | Self::Markdown => "notes",
            Self::Docx | Self::Xlsx | Self::Csv | Self::Pdf => "uploads",
            Self::Unsupported => "uploads",
        }
    }

    pub fn is_supported(self) -> bool {
        !matches!(self, Self::Unsupported)
    }
}

/// Parse a file and extract markdown-ready text. Caller passes the already-
/// landed raw file path (under `<workspace>/raw/...`).
pub fn parse_file(path: &Path) -> Result<ParseResult, String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let source_type = SourceType::from_extension(ext);

    match source_type {
        SourceType::Text => parse_text(path),
        SourceType::Markdown => parse_markdown(path),
        SourceType::Docx => parse_docx(path),
        SourceType::Xlsx => parse_xlsx(path),
        SourceType::Csv => parse_csv(path),
        SourceType::Pdf => parse_pdf(path),
        SourceType::Unsupported => Err(format!(
            "unsupported file type '.{ext}'; accepts only pdf/docx/txt/md/xlsx/csv"
        )),
    }
}

fn parse_text(path: &Path) -> Result<ParseResult, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("读取 TXT 失败: {e}"))?;
    Ok(ParseResult {
        text,
        source_type: SourceType::Text,
        parser: "std::fs".to_string(),
    })
}

fn parse_markdown(path: &Path) -> Result<ParseResult, String> {
    use pulldown_cmark::Parser;

    let raw = fs::read_to_string(path).map_err(|e| format!("读取 Markdown 失败: {e}"))?;
    // Validate well-formed Markdown by parsing once (pulldown-cmark won't fail
    // on user content; just exhaust events). Source returned verbatim so wiki
    // pages preserve headings/lists/tables/etc.
    for _evt in Parser::new(&raw) {}
    Ok(ParseResult {
        text: raw,
        source_type: SourceType::Markdown,
        parser: "pulldown-cmark".to_string(),
    })
}

/// DOCX extraction: open the package as ZIP, pull `word/document.xml`,
/// walk `<w:t>` runs to assemble paragraphs. No formatting preservation
/// beyond paragraph breaks — V1 scope is text.
fn parse_docx(path: &Path) -> Result<ParseResult, String> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let file = fs::File::open(path).map_err(|e| format!("打开 DOCX 失败: {e}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("DOCX 不是有效 ZIP: {e}"))?;
    let mut document_xml = String::new();
    {
        let mut entry = zip
            .by_name("word/document.xml")
            .map_err(|e| format!("DOCX 缺少 word/document.xml: {e}"))?;
        entry
            .read_to_string(&mut document_xml)
            .map_err(|e| format!("读取 DOCX 内部 XML 失败: {e}"))?;
    }

    let mut reader = Reader::from_str(&document_xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut text = String::new();
    let mut in_text_run = false;
    let mut paragraph_pending = false;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                let local = name.as_ref();
                if local.ends_with(b":t") || local == b"t" {
                    in_text_run = true;
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                let local = name.as_ref();
                if local.ends_with(b":t") || local == b"t" {
                    in_text_run = false;
                }
                if local.ends_with(b":p") || local == b"p" {
                    paragraph_pending = true;
                }
            }
            Ok(Event::Empty(e)) => {
                let name = e.name();
                let local = name.as_ref();
                if local.ends_with(b":br") || local == b"br" {
                    text.push('\n');
                }
            }
            Ok(Event::Text(t)) => {
                if in_text_run {
                    if paragraph_pending {
                        text.push_str("\n\n");
                        paragraph_pending = false;
                    }
                    let raw_str = std::str::from_utf8(t.as_ref())
                        .map_err(|e| format!("DOCX 文本不是合法 UTF-8: {e}"))?;
                    let decoded = quick_xml::escape::unescape(raw_str)
                        .map_err(|e| format!("DOCX XML 实体解码失败: {e}"))?;
                    text.push_str(&decoded);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("DOCX XML 解析失败: {e}")),
            _ => {}
        }
        buf.clear();
    }
    Ok(ParseResult {
        text: text.trim().to_string(),
        source_type: SourceType::Docx,
        parser: "zip+quick-xml".to_string(),
    })
}

/// XLSX → markdown tables, one per sheet. Cell values use the workbook's
/// stored representation (`calamine` doesn't recompute formulas).
fn parse_xlsx(path: &Path) -> Result<ParseResult, String> {
    use calamine::{open_workbook_auto, Reader as _};

    let mut wb = open_workbook_auto(path).map_err(|e| format!("打开 XLSX 失败: {e}"))?;
    let sheet_names: Vec<String> = wb.sheet_names().to_vec();
    let mut out = String::new();
    for (idx, name) in sheet_names.iter().enumerate() {
        let range = wb
            .worksheet_range(name)
            .map_err(|e| format!("读取 sheet '{name}' 失败: {e}"))?;

        if idx > 0 {
            out.push_str("\n\n");
        }
        out.push_str(&format!("## {name}\n\n"));

        let mut rows = range.rows().peekable();
        if rows.peek().is_none() {
            out.push_str("_(empty sheet)_\n");
            continue;
        }

        let header_row = rows.next().unwrap();
        out.push_str("| ");
        out.push_str(
            &header_row
                .iter()
                .map(format_xlsx_cell)
                .collect::<Vec<_>>()
                .join(" | "),
        );
        out.push_str(" |\n|");
        for _ in header_row {
            out.push_str("---|");
        }
        out.push('\n');

        for row in rows {
            out.push_str("| ");
            out.push_str(
                &row.iter()
                    .map(format_xlsx_cell)
                    .collect::<Vec<_>>()
                    .join(" | "),
            );
            out.push_str(" |\n");
        }
    }

    Ok(ParseResult {
        text: out,
        source_type: SourceType::Xlsx,
        parser: "calamine".to_string(),
    })
}

fn format_xlsx_cell(cell: &calamine::Data) -> String {
    use calamine::Data;
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.replace('|', "\\|").replace('\n', " "),
        Data::Float(f) => {
            if f.fract() == 0.0 && f.abs() < 1e16 {
                format!("{}", *f as i64)
            } else {
                format!("{f}")
            }
        }
        Data::Int(i) => format!("{i}"),
        Data::Bool(b) => format!("{b}"),
        Data::DateTime(dt) => format!("{}", dt.as_f64()),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("#ERROR({e:?})"),
    }
}

/// CSV → single markdown table. First row treated as header. Empty file
/// returns an empty result.
fn parse_csv(path: &Path) -> Result<ParseResult, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_path(path)
        .map_err(|e| format!("打开 CSV 失败: {e}"))?;

    let mut rows = rdr.records();
    let Some(first) = rows.next() else {
        return Ok(ParseResult {
            text: String::new(),
            source_type: SourceType::Csv,
            parser: "csv".to_string(),
        });
    };
    let header = first.map_err(|e| format!("读取 CSV 表头失败: {e}"))?;

    let mut out = String::from("| ");
    out.push_str(
        &header
            .iter()
            .map(escape_csv_cell)
            .collect::<Vec<_>>()
            .join(" | "),
    );
    out.push_str(" |\n|");
    for _ in &header {
        out.push_str("---|");
    }
    out.push('\n');

    let col_count = header.len();
    for row in rows {
        let row = row.map_err(|e| format!("读取 CSV 行失败: {e}"))?;
        out.push_str("| ");
        let cells: Vec<String> = (0..col_count)
            .map(|i| escape_csv_cell(row.get(i).unwrap_or("")))
            .collect();
        out.push_str(&cells.join(" | "));
        out.push_str(" |\n");
    }

    Ok(ParseResult {
        text: out,
        source_type: SourceType::Csv,
        parser: "csv".to_string(),
    })
}

fn escape_csv_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn parse_pdf(path: &Path) -> Result<ParseResult, String> {
    let text = pdf_extract::extract_text(path).map_err(|e| stable_pdf_error(&e.to_string()))?;
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(
            "未提取到可用文本。当前仅支持文本型 PDF，不支持扫描件或图片型 PDF。".to_string(),
        );
    }

    Ok(ParseResult {
        text,
        source_type: SourceType::Pdf,
        parser: "pdf-extract".to_string(),
    })
}

fn stable_pdf_error(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("encrypt") || lower.contains("password") || lower.contains("permission") {
        "当前不支持受密码保护的 PDF。".to_string()
    } else {
        "PDF 文件无法解析，请确认文件未损坏。".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn minimal_text_pdf_bytes() -> &'static [u8] {
        b"%PDF-1.4
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj
2 0 obj
<< /Type /Pages /Kids [3 0 R] /Count 1 >>
endobj
3 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>
endobj
4 0 obj
<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>
endobj
5 0 obj
<< /Length 47 >>
stream
BT /F1 24 Tf 100 700 Td (Hello PDF text) Tj ET
endstream
endobj
xref
0 6
0000000000 65535 f 
0000000009 00000 n 
0000000058 00000 n 
0000000115 00000 n 
0000000241 00000 n 
0000000311 00000 n 
trailer
<< /Root 1 0 R /Size 6 >>
startxref
407
%%EOF
"
    }

    #[test]
    fn from_extension_maps_supported_types() {
        assert_eq!(SourceType::from_extension("txt"), SourceType::Text);
        assert_eq!(SourceType::from_extension("md"), SourceType::Markdown);
        assert_eq!(SourceType::from_extension("MARKDOWN"), SourceType::Markdown);
        assert_eq!(SourceType::from_extension("docx"), SourceType::Docx);
        assert_eq!(SourceType::from_extension("xlsx"), SourceType::Xlsx);
        assert_eq!(SourceType::from_extension("csv"), SourceType::Csv);
        assert_eq!(SourceType::from_extension("pdf"), SourceType::Pdf);
    }

    #[test]
    fn from_extension_rejects_unsupported() {
        for ext in ["doc", "mp3", "wav", "png", "jpg", "", "rtf"] {
            assert_eq!(
                SourceType::from_extension(ext),
                SourceType::Unsupported,
                "extension '{ext}' should be unsupported"
            );
        }
    }

    #[test]
    fn parse_text_reads_utf8() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().with_extension("txt");
        std::fs::write(&path, "hello 中文 world").unwrap();
        let r = parse_file(&path).unwrap();
        assert_eq!(r.source_type, SourceType::Text);
        assert_eq!(r.parser, "std::fs");
        assert!(r.text.contains("中文"));
    }

    #[test]
    fn parse_markdown_passes_through_source() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().with_extension("md");
        std::fs::write(&path, "# Title\n\n- bullet\n- another\n").unwrap();
        let r = parse_file(&path).unwrap();
        assert_eq!(r.source_type, SourceType::Markdown);
        assert_eq!(r.parser, "pulldown-cmark");
        assert!(r.text.contains("# Title"));
        assert!(r.text.contains("- bullet"));
    }

    #[test]
    fn parse_csv_emits_markdown_table() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().with_extension("csv");
        std::fs::write(&path, "name,age\nAlice,30\nBob,25\n").unwrap();
        let r = parse_file(&path).unwrap();
        assert_eq!(r.source_type, SourceType::Csv);
        assert_eq!(r.parser, "csv");
        assert!(r.text.contains("| name | age |"));
        assert!(r.text.contains("|---|---|"));
        assert!(r.text.contains("| Alice | 30 |"));
    }

    #[test]
    fn parse_csv_escapes_pipes() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().with_extension("csv");
        std::fs::write(&path, "col\n\"a|b\"\n").unwrap();
        let r = parse_file(&path).unwrap();
        assert!(r.text.contains("a\\|b"));
    }

    #[test]
    fn parse_unsupported_returns_err() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().with_extension("doc");
        std::fs::write(&path, b"legacy doc").unwrap();
        let err = parse_file(&path).unwrap_err();
        assert!(err.contains("unsupported"));
    }

    #[test]
    fn parse_pdf_extracts_text_from_text_pdf() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().with_extension("pdf");
        std::fs::write(&path, minimal_text_pdf_bytes()).unwrap();

        let r = parse_file(&path).unwrap();

        assert_eq!(r.source_type, SourceType::Pdf);
        assert_eq!(r.parser, "pdf-extract");
        assert!(r.text.contains("Hello PDF text"));
    }

    #[test]
    fn parse_pdf_empty_text_returns_stable_error() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().with_extension("pdf");
        std::fs::write(&path, b"%PDF-1.4\n%%EOF\n").unwrap();

        let err = parse_file(&path).unwrap_err();

        assert!(
            err == "PDF 文件无法解析，请确认文件未损坏。"
                || err == "未提取到可用文本。当前仅支持文本型 PDF，不支持扫描件或图片型 PDF。"
        );
    }

    #[test]
    fn parse_docx_extracts_paragraph_text() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().with_extension("docx");

        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Hello</w:t></w:r><w:r><w:t> 中文世界</w:t></w:r></w:p>
    <w:p><w:r><w:t>Second paragraph</w:t></w:r></w:p>
  </w:body>
</w:document>"#;
        zip.start_file("word/document.xml", options).unwrap();
        zip.write_all(document_xml.as_bytes()).unwrap();
        zip.finish().unwrap();

        let r = parse_file(&path).unwrap();
        assert_eq!(r.source_type, SourceType::Docx);
        assert_eq!(r.parser, "zip+quick-xml");
        assert!(r.text.contains("Hello"));
        assert!(r.text.contains("中文世界"));
        assert!(r.text.contains("Second paragraph"));
    }
}
