pub mod pandoc;
pub mod pdf;
pub mod xlsx;

use kgx_core::{KgError, Result};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceFormat {
    Pdf,
    Docx,
    Xlsx,
    Pptx,
    Odt,
    Epub,
    Html,
    Markdown,
    Text,
}

#[derive(Debug, Clone)]
pub struct ConvertOutput {
    pub markdown: String,
    pub title: String,
    pub source_format: SourceFormat,
}

pub const SUPPORTED_EXTS: &[&str] = &[
    "md", "txt", "markdown", "mdx", "pdf", "docx", "pptx", "odt", "epub", "html", "htm", "xlsx",
    "xls",
];

pub fn is_document_ext(ext: &str) -> bool {
    SUPPORTED_EXTS.iter().any(|e| e.eq_ignore_ascii_case(ext))
}

fn classify(ext: &str) -> Option<SourceFormat> {
    match ext.to_ascii_lowercase().as_str() {
        "md" | "markdown" | "mdx" => Some(SourceFormat::Markdown),
        "txt" => Some(SourceFormat::Text),
        "pdf" => Some(SourceFormat::Pdf),
        "docx" => Some(SourceFormat::Docx),
        "xlsx" | "xls" => Some(SourceFormat::Xlsx),
        "pptx" => Some(SourceFormat::Pptx),
        "odt" => Some(SourceFormat::Odt),
        "epub" => Some(SourceFormat::Epub),
        "html" | "htm" => Some(SourceFormat::Html),
        _ => None,
    }
}

fn title_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("capture")
        .to_string()
}

fn title_from_markdown(markdown: &str, fallback: &str) -> String {
    for line in markdown.lines() {
        let trimmed = line.trim_start_matches('#').trim();
        if !trimmed.is_empty() {
            return trimmed.chars().take(80).collect();
        }
    }
    fallback.to_string()
}

pub fn convert(path: &Path) -> Result<ConvertOutput> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| KgError::Convert("file has no extension".into()))?;

    let fmt = classify(ext).ok_or_else(|| {
        KgError::Convert(format!(
            "unsupported format: .{ext}. Supported: {}",
            SUPPORTED_EXTS.join(", ")
        ))
    })?;

    match fmt {
        SourceFormat::Markdown | SourceFormat::Text => {
            let content = std::fs::read_to_string(path).map_err(|e| KgError::Io {
                path: path.display().to_string(),
                source: e,
            })?;
            let title = title_from_markdown(&content, &title_from_path(path));
            Ok(ConvertOutput {
                markdown: content,
                title,
                source_format: fmt,
            })
        }
        SourceFormat::Pdf => pdf::convert(path),
        SourceFormat::Xlsx => xlsx::convert(path),
        SourceFormat::Docx
        | SourceFormat::Pptx
        | SourceFormat::Odt
        | SourceFormat::Epub
        | SourceFormat::Html => {
            let md = pandoc::convert(path)?;
            let title = title_from_markdown(&md, &title_from_path(path));
            Ok(ConvertOutput {
                markdown: md,
                title,
                source_format: fmt,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_is_document_ext() {
        assert!(is_document_ext("pdf"));
        assert!(is_document_ext("PDF"));
        assert!(is_document_ext("docx"));
        assert!(is_document_ext("xlsx"));
        assert!(is_document_ext("md"));
        assert!(!is_document_ext("xyz"));
        assert!(!is_document_ext(""));
    }

    #[test]
    fn test_classify() {
        assert_eq!(classify("pdf"), Some(SourceFormat::Pdf));
        assert_eq!(classify("DOCX"), Some(SourceFormat::Docx));
        assert_eq!(classify("md"), Some(SourceFormat::Markdown));
        assert_eq!(classify("xyz"), None);
    }

    #[test]
    fn test_convert_markdown_passthrough() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.md");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "# Hello World\n\nSome content.").unwrap();
        let out = convert(&path).unwrap();
        assert_eq!(out.source_format, SourceFormat::Markdown);
        assert_eq!(out.title, "Hello World");
        assert!(out.markdown.contains("Some content."));
    }

    #[test]
    fn test_convert_text_passthrough() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "Plain text content").unwrap();
        let out = convert(&path).unwrap();
        assert_eq!(out.source_format, SourceFormat::Text);
        assert!(out.markdown.contains("Plain text content"));
    }

    #[test]
    fn test_convert_unsupported_ext() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.xyz");
        std::fs::write(&path, "content").unwrap();
        let err = convert(&path).unwrap_err();
        assert!(matches!(err, KgError::Convert(_)));
        assert!(err.to_string().contains("unsupported format"));
    }
}
