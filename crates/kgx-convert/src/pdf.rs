use kgx_core::{KgError, Result};
use std::path::Path;

pub fn convert(path: &Path) -> Result<super::ConvertOutput> {
    Err(KgError::Convert("pdf conversion not yet implemented".into()))
}
