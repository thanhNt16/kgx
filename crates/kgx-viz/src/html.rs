use crate::model::GraphModel;

const TEMPLATE: &str = include_str!("../templates/graph.html.tera");

pub fn render(model: &GraphModel) -> String {
    let data = serde_json::to_string(model).expect("graph model serializes");
    let mut ctx = tera::Context::new();
    ctx.insert("graph_data", &data);
    tera::Tera::one_off(TEMPLATE, &ctx, false).expect("graph template renders")
}
