pub fn extract_prompt(source_body: &str, ladder: Option<&str>) -> String {
    match ladder {
        Some(prefix) if !prefix.trim().is_empty() => {
            format!("{}\n\nEXTRACT_FACTS\n{source_body}", prefix.trim())
        }
        _ => format!("EXTRACT_FACTS\n{source_body}"),
    }
}

pub const EXTRACT_SYSTEM: &str = "You extract atomic, one-claim-per-note facts with provenance, and classify referenced entities with the POLE taxonomy. Reply JSON {facts:[{title,body,confidence,entities:[{name,entity_type,rel}]}]}. entity_type is one of person|object|location|event. rel describes how the fact relates to the entity: one of mentions|participates_in|located_at|owns|decided|caused (default mentions). If unsure of entity_type, omit it.";
