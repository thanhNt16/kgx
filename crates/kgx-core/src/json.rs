/// Universal --json output envelope. EVERY command serializes exactly this.
#[derive(Debug, serde::Serialize)]
pub struct JsonEnvelope<T: serde::Serialize> {
    pub ok: bool,
    pub command: String,                 // e.g. "index"
    pub data: T,                         // command-specific payload
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub elapsed_ms: u64,
}
impl<T: serde::Serialize> JsonEnvelope<T> {
    pub fn success(command: &str, data: T, elapsed_ms: u64) -> Self {
        Self { ok: true, command: command.into(), data, warnings: vec![], error: None, elapsed_ms }
    }
}
