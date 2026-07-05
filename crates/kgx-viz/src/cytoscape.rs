use crate::model::GraphModel;

const CYTOSCAPE_JS: &str = include_str!("../assets/cytoscape.min.js");

const POLE_COLORS: &[(&str, &str)] = &[
    ("person", "#e15759"),
    ("object", "#4e79a7"),
    ("location", "#59a14f"),
    ("event", "#f28e2b"),
];

pub fn render(model: &GraphModel) -> String {
    let elements: Vec<serde_json::Value> = model
        .nodes
        .iter()
        .map(|n| {
            serde_json::json!({"data": {
                "id": n.id,
                "label": n.title,
                "type": n.r#type,
                "status": n.status,
                "pagerank": n.pagerank,
                "entity_type": n.entity_type.as_deref(),
            }})
        })
        .chain(model.edges.iter().enumerate().map(|(i, e)| {
            serde_json::json!({"data": {
                "id": format!("e{i}"),
                "source": e.src,
                "target": e.dst,
                "label": e.rel,
            }})
        }))
        .collect();
    let elements_json = serde_json::to_string(&elements).unwrap_or_else(|_| "[]".into());
    let color_rules: String = POLE_COLORS
        .iter()
        .map(|(t, c)| {
            format!(
                "{{ selector: 'node[entity_type = \"{t}\"]', style: {{ 'background-color': '{c}' }} }},"
            )
        })
        .collect();
    format!(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>KGX graph</title>
<style>
  body {{ margin:0; font:13px system-ui; }}
  #cy {{ position:absolute; top:40px; bottom:0; left:0; right:0; }}
  #bar {{ height:40px; display:flex; gap:8px; align-items:center; padding:0 12px; border-bottom:1px solid #ccc; }}
  #info {{ position:absolute; right:12px; top:52px; width:280px; background:#fff; border:1px solid #ccc; padding:8px; display:none; }}
</style>
<script>{lib}</script>
</head><body>
<div id="bar">
  <strong>KGX graph</strong>
  <label>type <select id="ftype"><option value="">all</option></select></label>
  <span id="counts"></span>
</div>
<div id="cy"></div>
<div id="info"></div>
<script>
const elements = {elements};
const cy = cytoscape({{
  container: document.getElementById('cy'),
  elements: elements,
  layout: {{ name: 'cose', animate: false }},
  style: [
    {{ selector: 'node', style: {{ 'label': 'data(label)', 'font-size': 9, 'width': 'mapData(pagerank, 0, 1, 12, 40)', 'height': 'mapData(pagerank, 0, 1, 12, 40)', 'background-color': '#9aa0a6' }} }},
    {color_rules}
    {{ selector: 'edge', style: {{ 'label': 'data(label)', 'font-size': 7, 'curve-style': 'bezier', 'target-arrow-shape': 'triangle', 'width': 1, 'line-color': '#bbb' }} }},
    {{ selector: 'node[status = "superseded"], node[status = "archived"]', style: {{ 'opacity': 0.35 }} }}
  ]
}});
const types = [...new Set(elements.filter(e => !e.data.source).map(e => e.data.type))];
const sel = document.getElementById('ftype');
types.forEach(t => {{ const o = document.createElement('option'); o.value = t; o.textContent = t; sel.appendChild(o); }});
sel.onchange = () => {{
  cy.nodes().forEach(n => n.style('display', (!sel.value || n.data('type') === sel.value) ? 'element' : 'none'));
}};
cy.on('tap', 'node', evt => {{
  const d = evt.target.data();
  const info = document.getElementById('info');
  info.style.display = 'block';
  info.innerHTML = '<b>' + d.label + '</b><br>type: ' + d.type + (d.entity_type ? ' / ' + d.entity_type : '') + '<br>status: ' + d.status + '<br>pagerank: ' + d.pagerank.toFixed(4) + '<br>id: ' + d.id;
}});
document.getElementById('counts').textContent = cy.nodes().length + ' nodes, ' + cy.edges().length + ' edges';
</script>
</body></html>"#,
        lib = CYTOSCAPE_JS,
        elements = elements_json,
        color_rules = color_rules,
    )
}

#[cfg(test)]
mod tests {
    use crate::model::{GraphModel, VizEdge, VizNode};

    #[test]
    fn render_is_self_contained_and_pole_colored() {
        let model = GraphModel {
            nodes: vec![
                VizNode {
                    id: "E1".into(),
                    title: "Alice".into(),
                    r#type: "entity".into(),
                    status: "active".into(),
                    pagerank: 0.5,
                    entity_type: Some("person".into()),
                },
                VizNode {
                    id: "F1".into(),
                    title: "fact".into(),
                    r#type: "fact".into(),
                    status: "active".into(),
                    pagerank: 0.1,
                    entity_type: None,
                },
            ],
            edges: vec![VizEdge {
                src: "F1".into(),
                dst: "E1".into(),
                rel: "decided".into(),
            }],
        };
        let html = super::render(&model);
        assert!(html.contains("<html"));
        assert!(html.contains("cytoscape"), "embeds the library");
        assert!(
            !html.contains("https://unpkg.com"),
            "no CDN — self-contained"
        );
        assert!(html.contains("\"E1\""));
        assert!(html.contains("person"));
        assert!(html.contains("#e15759"), "POLE person color present");
        assert!(html.contains("decided"), "edge rel label present");
    }
}
