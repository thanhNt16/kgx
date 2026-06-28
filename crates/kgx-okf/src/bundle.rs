use kgx_core::{KgError, Result};
use std::path::{Component, Path};

pub fn ship(root: &Path, out: &Path) -> Result<()> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| KgError::Io {
            path: parent.display().to_string(),
            source: e,
        })?;
    }
    let file = std::fs::File::create(out).map_err(|e| KgError::Io {
        path: out.display().to_string(),
        source: e,
    })?;
    let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut tar = tar::Builder::new(enc);
    for item in ["index.md", "log.md", "CLAUDE.md", "notes", "raw"] {
        let path = root.join(item);
        if !path.exists() {
            continue;
        }
        if path.is_dir() {
            tar.append_dir_all(item, &path)
                .map_err(|e| KgError::Other(e.to_string()))?;
        } else {
            tar.append_path_with_name(&path, item)
                .map_err(|e| KgError::Other(e.to_string()))?;
        }
    }
    tar.finish().map_err(|e| KgError::Other(e.to_string()))?;
    Ok(())
}

pub fn pull(bundle: &Path, root: &Path, namespace: Option<&str>) -> Result<usize> {
    let file = std::fs::File::open(bundle).map_err(|e| KgError::Io {
        path: bundle.display().to_string(),
        source: e,
    })?;
    let dec = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(dec);
    let base = namespace
        .map(|ns| root.join("notes").join(ns))
        .unwrap_or_else(|| root.to_path_buf());
    std::fs::create_dir_all(&base).map_err(|e| KgError::Io {
        path: base.display().to_string(),
        source: e,
    })?;

    let mut count = 0;
    for entry in archive
        .entries()
        .map_err(|e| KgError::Other(e.to_string()))?
    {
        let mut entry = entry.map_err(|e| KgError::Other(e.to_string()))?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry
            .path()
            .map_err(|e| KgError::Other(e.to_string()))?
            .into_owned();
        if !is_safe_relative(&path) {
            return Err(KgError::Validation(format!(
                "unsafe bundle path: {}",
                path.display()
            )));
        }
        let target = if namespace.is_some() {
            let rel = match path.strip_prefix("notes") {
                Ok(rel) if !rel.as_os_str().is_empty() => rel,
                _ => continue,
            };
            base.join(rel)
        } else {
            base.join(path)
        };
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| KgError::Io {
                path: parent.display().to_string(),
                source: e,
            })?;
        }
        entry
            .unpack(&target)
            .map_err(|e| KgError::Other(e.to_string()))?;
        count += 1;
    }
    Ok(count)
}

fn is_safe_relative(path: &Path) -> bool {
    path.components().all(|c| matches!(c, Component::Normal(_)))
}
