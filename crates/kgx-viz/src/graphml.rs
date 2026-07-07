use crate::model::GraphModel;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn render(model: &GraphModel) -> String {
    let mut out = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<graphml xmlns="http://graphml.graphdrawing.org/xmlns">
  <key id="d0" for="node" attr.name="title" attr.type="string"/>
  <key id="d1" for="node" attr.name="type" attr.type="string"/>
  <key id="d2" for="node" attr.name="status" attr.type="string"/>
  <key id="d3" for="node" attr.name="entity_type" attr.type="string"/>
  <key id="d4" for="node" attr.name="pagerank" attr.type="double"/>
  <key id="d5" for="edge" attr.name="rel" attr.type="string"/>
  <graph id="kgx" edgedefault="directed">
"#,
    );
    for n in &model.nodes {
        out.push_str(&format!(
            "    <node id=\"{}\"><data key=\"d0\">{}</data><data key=\"d1\">{}</data><data key=\"d2\">{}</data><data key=\"d3\">{}</data><data key=\"d4\">{}</data></node>\n",
            esc(&n.id),
            esc(&n.title),
            esc(&n.r#type),
            esc(&n.status),
            esc(n.entity_type.as_deref().unwrap_or("")),
            n.pagerank,
        ));
    }
    for (i, e) in model.edges.iter().enumerate() {
        out.push_str(&format!(
            "    <edge id=\"e{i}\" source=\"{}\" target=\"{}\"><data key=\"d5\">{}</data></edge>\n",
            esc(&e.src),
            esc(&e.dst),
            esc(&e.rel),
        ));
    }
    out.push_str("  </graph>\n</graphml>\n");
    out
}

#[cfg(test)]
mod tests {
    use crate::model::{GraphModel, VizEdge, VizNode};

    #[test]
    fn graphml_has_typed_nodes_and_edges() {
        let model = GraphModel {
            nodes: vec![VizNode {
                id: "E1".into(),
                title: "Alice & Bob".into(),
                r#type: "entity".into(),
                status: "active".into(),
                pagerank: 0.5,
                entity_type: Some("person".into()),
                community: 0,
            }],
            edges: vec![VizEdge {
                src: "E1".into(),
                dst: "E1".into(),
                rel: "owns".into(),
            }],
        };
        let xml = super::render(&model);
        assert!(xml.starts_with("<?xml"));
        assert!(xml.contains("graphml.graphdrawing.org"));
        assert!(xml.contains("Alice &amp; Bob"), "XML-escapes titles");
        assert!(xml.contains(">person<"));
        assert!(xml.contains(">owns<"));
    }
}
