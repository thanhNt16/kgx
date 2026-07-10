use kgx_core::{KgError, Result};
use std::path::Path;

pub fn convert(path: &Path) -> Result<String> {
    Err(KgError::Convert("pandoc conversion not yet implemented".into()))
}
