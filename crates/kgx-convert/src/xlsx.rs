use kgx_core::{KgError, Result};
use std::path::Path;

pub fn convert(path: &Path) -> Result<super::ConvertOutput> {
    Err(KgError::Convert("xlsx conversion not yet implemented".into()))
}
