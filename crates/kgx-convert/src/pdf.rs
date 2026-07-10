use kgx_core::{KgError, Result};
use std::path::Path;

pub fn convert(path: &Path) -> Result<super::ConvertOutput> {
    let text = pdf_extract::extract_text(path)
        .map_err(|e| KgError::Convert(format!("pdf extraction failed: {e}")))?;

    let markdown = if text.trim().is_empty() {
        "[No extractable text — this may be a scanned/image-only document]".to_string()
    } else {
        text.trim().to_string()
    };

    let title = markdown
        .lines()
        .next()
        .unwrap_or("pdf-document")
        .trim()
        .chars()
        .take(80)
        .collect::<String>();

    Ok(super::ConvertOutput {
        markdown,
        title,
        source_format: super::SourceFormat::Pdf,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_minimal_pdf(path: &Path) {
        let pdf = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n3 0 obj\n<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R /MediaBox [0 0 612 792] >>\nendobj\n4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n5 0 obj\n<< /Length 47 >>\nstream\nBT /F1 12 Tf 100 700 Td (Hello PDF World) Tj ET\nendstream\nendobj\nxref\n0 6\n0000000000 65535 f \n0000000009 00000 n \n0000000058 00000 n \n0000000115 00000 n \n0000000241 00000 n \n0000000311 00000 n \ntrailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n408\n%%EOF";
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(pdf).unwrap();
    }

    #[test]
    fn test_pdf_extracts_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.pdf");
        create_minimal_pdf(&path);
        let out = convert(&path).unwrap();
        assert_eq!(out.source_format, crate::SourceFormat::Pdf);
    }

    #[test]
    fn test_pdf_not_found() {
        let err = convert(std::path::Path::new("/nonexistent/file.pdf")).unwrap_err();
        assert!(matches!(err, KgError::Convert(_)));
    }
}
