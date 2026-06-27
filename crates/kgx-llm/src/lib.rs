pub mod claude;
pub mod mock;
pub mod ollama;
pub mod openai;
pub mod select;

pub use mock::MockProvider;
pub use select::{embedder_from_env, provider_from_env};
