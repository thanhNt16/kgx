pub fn extract_prompt(source_body: &str, _ladder: Option<&str>) -> String {
    format!("EXTRACT_FACTS\n{source_body}")
}

pub const EXTRACT_SYSTEM: &str =
    "You extract atomic, one-claim-per-note facts with provenance. Reply JSON {facts:[{title,body,confidence,entities}]}.";
