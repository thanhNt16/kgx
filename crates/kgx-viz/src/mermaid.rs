use crate::model::GraphModel;

pub fn render(model: &GraphModel) -> String {
    let mut out = String::from("graph TD\n");
    for edge in &model.edges {
        out.push_str(&format!(
            "  {} -->|{}| {}\n",
            mermaid_id(&edge.src),
            label(&edge.rel),
            mermaid_id(&edge.dst)
        ));
    }
    out
}

fn mermaid_id(id: &str) -> String {
    let s: String = id.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if s.is_empty() {
        "node".into()
    } else {
        s
    }
}

fn label(s: &str) -> String {
    s.replace('|', "/")
}
