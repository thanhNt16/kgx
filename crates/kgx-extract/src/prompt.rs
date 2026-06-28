pub fn extract_prompt(source_body: &str, ladder: Option<&str>) -> String {
    match ladder {
        Some(prefix) if !prefix.trim().is_empty() => {
            format!("{}\n\nEXTRACT_FACTS\n{source_body}", prefix.trim())
        }
        _ => format!("EXTRACT_FACTS\n{source_body}"),
    }
}

pub const EXTRACT_SYSTEM: &str =
    "You extract atomic, one-claim-per-note facts with provenance. Reply JSON {facts:[{title,body,confidence,entities}]}.";
