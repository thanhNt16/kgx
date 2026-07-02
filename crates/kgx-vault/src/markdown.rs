use pulldown_cmark::{Event, Tag, TagEnd};

pub struct ParsedMarkdown {
    pub headings: Vec<String>,
    pub wikilinks: Vec<String>,
}

pub fn parse_markdown(body: &str) -> ParsedMarkdown {
    let mut headings = Vec::new();
    let mut wikilinks = Vec::new();
    let parser = pulldown_cmark::Parser::new(body);

    let mut in_heading = false;
    let mut heading_text = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level: _, id: _, classes: _, attrs: _ }) => {
                in_heading = true;
                heading_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                if !heading_text.is_empty() {
                    headings.push(std::mem::take(&mut heading_text));
                }
            }
            Event::Text(t) => {
                let s = t.to_string();
                if in_heading {
                    heading_text.push_str(&s);
                }
            }
            _ => {}
        }
    }

    let link_re = regex::Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]+)?\]\]").unwrap();
    for cap in link_re.captures_iter(body) {
        let target = cap[1].trim().to_string();
        if !target.is_empty() {
            wikilinks.push(target);
        }
    }

    ParsedMarkdown {
        headings,
        wikilinks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_headings() {
        let md = "# Hello\n\nSome text\n\n## Sub heading\n\nMore text\n";
        let result = parse_markdown(md);
        assert_eq!(result.headings, vec!["Hello", "Sub heading"]);
    }

    #[test]
    fn extracts_wikilinks() {
        let md = "See [[Postgres]] and [[Redis|Redis cache]]. Also [[../raw/note]].";
        let result = parse_markdown(md);
        assert_eq!(
            result.wikilinks,
            vec!["Postgres", "Redis", "../raw/note"]
        );
    }

    #[test]
    fn empty_body() {
        let result = parse_markdown("");
        assert!(result.headings.is_empty());
        assert!(result.wikilinks.is_empty());
    }
}
