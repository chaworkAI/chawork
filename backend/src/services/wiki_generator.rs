//! Wiki Generator — writes one Markdown file under `wiki/documents/` per
//! imported source (DESIGN §5 post-revision).
//!
//! V1 keeps it deliberately simple: a fixed YAML frontmatter (type / source /
//! created_at / parser) followed by the parsed text content. Domain Pack
//! templates no longer drive document-import rendering; they stay relevant
//! for hand-authored objects/concepts/reports, not imports.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::services::parser::SourceType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiPage {
    pub path: String,
    pub title: String,
    /// Identifier of the parser crate that produced this page (e.g. `calamine`).
    pub parser: String,
}

/// Generate a wiki page from parsed text content.
///
/// `parser` is surfaced into frontmatter so consumers can tell how a file was
/// converted (e.g. `csv`, `calamine`, `zip+quick-xml`).
pub fn generate_wiki_page(
    workspace_path: &Path,
    title: &str,
    text_content: &str,
    source_type: SourceType,
    source_filename: &str,
    parser_name: &str,
    raw_relpath: &str,
) -> Result<WikiPage, String> {
    let now = chrono::Utc::now();
    let created_at = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let date_slug = now.format("%Y%m%d-%H%M%S").to_string();

    let wiki_dir = workspace_path.join("wiki").join("documents");
    std::fs::create_dir_all(&wiki_dir).map_err(|e| format!("创建 wiki/documents 失败: {e}"))?;

    let slug = slugify(title);
    let filename = format!("{slug}-{date_slug}.md");
    let full_path = wiki_dir.join(&filename);

    let body = render_document(
        title,
        &source_type_yaml_value(source_type),
        raw_relpath,
        &created_at,
        parser_name,
        text_content,
        source_filename,
    );

    std::fs::write(&full_path, &body).map_err(|e| format!("写入 wiki 页面失败: {e}"))?;

    let rel = full_path
        .strip_prefix(workspace_path)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| full_path.display().to_string());

    Ok(WikiPage {
        path: rel,
        title: title.to_string(),
        parser: parser_name.to_string(),
    })
}

fn source_type_yaml_value(source_type: SourceType) -> String {
    match source_type {
        SourceType::Text => "txt",
        SourceType::Markdown => "markdown",
        SourceType::Docx => "docx",
        SourceType::Xlsx => "xlsx",
        SourceType::Csv => "csv",
        SourceType::Pdf => "pdf",
        SourceType::Unsupported => "unknown",
    }
    .to_string()
}

fn yaml_quote(s: &str) -> String {
    // Simple YAML double-quoted string escaping: \ and "
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn render_document(
    title: &str,
    type_value: &str,
    raw_relpath: &str,
    created_at: &str,
    parser_name: &str,
    text_content: &str,
    source_filename: &str,
) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("title: {}\n", yaml_quote(title)));
    out.push_str(&format!("type: {}\n", yaml_quote(type_value)));
    out.push_str(&format!("source: {}\n", yaml_quote(raw_relpath)));
    out.push_str(&format!(
        "source_filename: {}\n",
        yaml_quote(source_filename)
    ));
    out.push_str(&format!("created_at: {}\n", yaml_quote(created_at)));
    out.push_str(&format!("parser: {}\n", yaml_quote(parser_name)));
    out.push_str("---\n\n");

    if !title.is_empty() {
        out.push_str(&format!("# {title}\n\n"));
    }
    out.push_str(text_content.trim_end_matches('\n'));
    out.push('\n');
    out
}

/// Simple slug generation: lowercase, CJK preserved, special chars replaced with hyphens.
fn slugify(input: &str) -> String {
    let mut slug = String::with_capacity(input.len());
    let mut last_was_sep = false;

    for ch in input.chars() {
        if ch.is_alphanumeric() || ch > '\u{4E00}' {
            slug.push(ch);
            last_was_sep = false;
        } else if !last_was_sep && !slug.is_empty() {
            slug.push('-');
            last_was_sep = true;
        }
    }

    let trimmed = slug.trim_end_matches('-').to_lowercase();
    if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_document_includes_required_frontmatter_fields() {
        let body = render_document(
            "Title with 中文",
            "csv",
            "raw/uploads/data.csv",
            "2026-05-20T10:00:00Z",
            "csv",
            "| col |\n|---|\n| v |\n",
            "data.csv",
        );
        assert!(body.starts_with("---\n"));
        assert!(body.contains("title: \"Title with 中文\"\n"));
        assert!(body.contains("type: \"csv\"\n"));
        assert!(body.contains("source: \"raw/uploads/data.csv\"\n"));
        assert!(body.contains("created_at: \"2026-05-20T10:00:00Z\"\n"));
        assert!(body.contains("parser: \"csv\"\n"));
        assert!(body.contains("source_filename: \"data.csv\""));
        assert!(body.contains("# Title with 中文\n"));
        assert!(body.contains("| col |"));
    }

    #[test]
    fn slugify_preserves_cjk_collapses_separators() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("黄一鸣 笔记"), "黄一鸣-笔记");
        assert_eq!(slugify("###"), "untitled");
    }

    #[test]
    fn yaml_quote_escapes_special_chars() {
        assert_eq!(yaml_quote("plain"), "\"plain\"");
        assert_eq!(yaml_quote("has \"quotes\""), "\"has \\\"quotes\\\"\"");
        assert_eq!(yaml_quote("back\\slash"), "\"back\\\\slash\"");
    }

    #[test]
    fn generate_wiki_page_writes_under_wiki_documents() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path();
        std::fs::create_dir_all(ws.join("wiki")).unwrap();

        let wp = generate_wiki_page(
            ws,
            "test note",
            "hello body",
            SourceType::Text,
            "test.txt",
            "std::fs",
            "raw/notes/test.txt",
        )
        .unwrap();

        assert!(wp.path.starts_with("wiki/documents/"));
        assert!(wp.path.ends_with(".md"));
        let body = std::fs::read_to_string(ws.join(&wp.path)).unwrap();
        assert!(body.contains("parser: \"std::fs\""));
        assert!(body.contains("source: \"raw/notes/test.txt\""));
        assert!(body.contains("hello body"));
    }
}
