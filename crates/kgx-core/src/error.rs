#[derive(Debug, thiserror::Error)]
pub enum KgError {
    #[error("io error at {path}: {source}")]
    Io { path: String, #[source] source: std::io::Error },
    #[error("frontmatter parse error in {path}: {msg}")]
    Frontmatter { path: String, msg: String },
    #[error("brain/sqlite error: {0}")]
    Brain(String),
    #[error("llm provider error: {0}")]
    Llm(String),
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("{0}")]
    Other(String),
}
pub type Result<T> = std::result::Result<T, KgError>;
