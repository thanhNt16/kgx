use crate::model::GraphModel;

pub fn render(model: &GraphModel) -> String {
    let mut out = String::from("digraph kgx {\n");
    for node in &model.nodes {
        out.push_str(&format!(
            "  \"{}\" [label=\"{}\"];\n",
            esc(&node.id),
            esc(&node.title)
        ));
    }
    for edge in &model.edges {
        out.push_str(&format!(
            "  \"{}\" -> \"{}\" [label=\"{}\"];\n",
            esc(&edge.src),
            esc(&edge.dst),
            esc(&edge.rel)
        ));
    }
    out.push_str("}\n");
    out
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
