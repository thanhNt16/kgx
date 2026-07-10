use kgx_core::{KgError, Result};
use std::path::Path;
use std::process::Command;

pub fn resolve_pandoc() -> Result<String> {
    if let Ok(p) = std::env::var("KGX_PANDOC") {
        if !p.is_empty() {
            return Ok(p);
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let bundled = format!("{home}/.local/bin/pandoc-kgx");
    if Path::new(&bundled).exists() {
        return Ok(bundled);
    }
    if let Ok(output) = Command::new("which").arg("pandoc").output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                return Ok(s);
            }
        }
    }
    Err(KgError::Convert(
        "pandoc not found. Set KGX_PANDOC env var, or install pandoc to ~/.local/bin/pandoc-kgx or system PATH.".into()
    ))
}

pub fn convert(path: &Path) -> Result<String> {
    let pandoc = resolve_pandoc()?;
    let output = Command::new(&pandoc)
        .arg(path)
        .arg("--to")
        .arg("gfm")
        .arg("--wrap=none")
        .output()
        .map_err(|e| KgError::Convert(format!("failed to run pandoc: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(KgError::Convert(format!("pandoc failed: {stderr}")));
    }

    let markdown = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(markdown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_pandoc_returns_error_or_path() {
        let result = resolve_pandoc();
        match result {
            Ok(path) => assert!(!path.is_empty()),
            Err(KgError::Convert(msg)) => assert!(msg.contains("pandoc not found")),
            Err(_) => panic!("expected Convert error"),
        }
    }
}
