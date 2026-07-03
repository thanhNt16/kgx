use crate::model::GraphModel;

const TEMPLATE: &str = include_str!("../templates/graph.html.tera");

pub fn render(model: &GraphModel) -> String {
    if model.nodes.is_empty() {
        return r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><title>KGX Graph</title></head><body style="font-family:system-ui,sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;color:#6b7280"><p>No nodes to display</p></body></html>"#.into();
    }
    let data = serde_json::to_string(model).expect("graph model serializes");
    let mut ctx = tera::Context::new();
    ctx.insert("graph_data", &data);
    tera::Tera::one_off(TEMPLATE, &ctx, false).expect("graph template renders")
}
