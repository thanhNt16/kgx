use std::sync::OnceLock;

pub fn new_ulid() -> String {
    ulid::Ulid::new().to_string()
}

pub fn now_iso() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs() as i64;
    let dt =
        time::OffsetDateTime::from_unix_timestamp(secs).unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
    dt.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

static WIKILINK_RE: OnceLock<regex::Regex> = OnceLock::new();
pub fn extract_wikilinks(s: &str) -> Vec<String> {
    let re = WIKILINK_RE.get_or_init(|| {
        regex::Regex::new(r"\[\[([^\]\|]+?)(?:\|[^\]]+)?\]\]").expect("valid wikilink regex")
    });
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for cap in re.captures_iter(s) {
        let target = cap[1].trim().to_string();
        if seen.insert(target.clone()) {
            out.push(target);
        }
    }
    out
}

pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wikilinks_extracted_and_deduped() {
        let links = extract_wikilinks("See [[Postgres]] and [[Billing Service]] and [[Postgres]].");
        assert_eq!(
            links,
            vec!["Postgres".to_string(), "Billing Service".to_string()]
        );
    }
    #[test]
    fn ulid_is_26_chars_and_monotonic() {
        let a = new_ulid();
        // Wait 2ms so timestamps differ, ensuring the second ULID sorts after
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = new_ulid();
        assert_eq!(a.len(), 26);
        assert_eq!(b.len(), 26);
        assert!(b >= a, "ULIDs should sort by time: {a} <= {b}");
    }
    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("Postgres is Primary!"), "postgres-is-primary");
    }
}
