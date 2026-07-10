use calamine::{open_workbook, Data, Reader, Xls, Xlsx};
use kgx_core::{KgError, Result};
use std::io::{Read, Seek};
use std::path::Path;

pub fn convert(path: &Path) -> Result<super::ConvertOutput> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let sheets: Vec<(String, Vec<Vec<String>>)> = if ext.eq_ignore_ascii_case("xlsx") {
        let mut workbook: Xlsx<_> =
            open_workbook(path).map_err(|e| KgError::Convert(format!("xlsx open failed: {e}")))?;
        extract_sheets(&mut workbook)
    } else if ext.eq_ignore_ascii_case("xls") {
        let mut workbook: Xls<_> =
            open_workbook(path).map_err(|e| KgError::Convert(format!("xls open failed: {e}")))?;
        extract_sheets(&mut workbook)
    } else {
        return Err(KgError::Convert(format!("not an excel file: .{ext}")));
    };

    if sheets.is_empty() {
        return Err(KgError::Convert("no sheets found in excel file".into()));
    }

    let mut markdown = String::new();
    for (i, (name, rows)) in sheets.iter().enumerate() {
        if i > 0 {
            markdown.push_str("\n\n");
        }
        markdown.push_str(&format!("## {name}\n\n"));
        if rows.is_empty() {
            markdown.push_str("(empty sheet)\n");
            continue;
        }
        let header = &rows[0];
        let col_count = header.len();
        markdown.push('|');
        for h in header {
            markdown.push_str(&format!(" {} |", h));
        }
        markdown.push_str("\n|");
        for _ in 0..col_count {
            markdown.push_str("---|");
        }
        markdown.push('\n');
        for row in &rows[1..] {
            markdown.push('|');
            for j in 0..col_count {
                let cell = row.get(j).map(|s| s.as_str()).unwrap_or("");
                markdown.push_str(&format!(" {} |", cell));
            }
            markdown.push('\n');
        }
    }

    let title = sheets[0].0.clone();

    Ok(super::ConvertOutput {
        markdown,
        title,
        source_format: super::SourceFormat::Xlsx,
    })
}

fn cell_to_string(data: &Data) -> String {
    match data {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => {
            if *f == (*f as i64) as f64 {
                format!("{}", *f as i64)
            } else {
                format!("{f}")
            }
        }
        Data::Int(i) => format!("{i}"),
        Data::Bool(b) => format!("{b}"),
        Data::DateTime(d) => format!("{d}"),
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("{e}"),
    }
}

fn extract_sheets<R, RS>(workbook: &mut R) -> Vec<(String, Vec<Vec<String>>)>
where
    R: Reader<RS>,
    RS: Read + Seek,
{
    let mut result = Vec::new();
    for sheet_name in workbook.sheet_names() {
        if let Ok(range) = workbook.worksheet_range(&sheet_name) {
            let rows: Vec<Vec<String>> = range
                .rows()
                .map(|row| row.iter().map(cell_to_string).collect())
                .collect();
            if !rows.is_empty() {
                result.push((sheet_name, rows));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xlsx_not_found() {
        let err = convert(std::path::Path::new("/nonexistent/file.xlsx")).unwrap_err();
        assert!(matches!(err, KgError::Convert(_)));
    }
}
