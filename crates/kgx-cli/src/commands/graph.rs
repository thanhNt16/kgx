use crate::output::emit;
use kgx_graph::Brain;
use kgx_viz::{cytoscape, dot, graphml, html, mermaid, model::from_brain};
use std::{path::PathBuf, time::Instant};

pub fn run(
    json: bool,
    format: &str,
    out: Option<PathBuf>,
    filter: Option<String>,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let brain = Brain::open(&root.join(".kg/brain.sqlite"))?;
    let model = from_brain(&brain, filter.as_deref())?;
    let content = match format {
        "html" => html::render(&model),
        "cytoscape" => cytoscape::render(&model),
        "graphml" => graphml::render(&model),
        "mermaid" => mermaid::render(&model),
        "dot" => dot::render(&model),
        "obsidian" => obsidian_canvas(&model)?,
        other => anyhow::bail!(
            "unknown graph format: {other} (supported: html, cytoscape, graphml, mermaid, dot, obsidian)"
        ),
    };
    let ext = match format {
        "obsidian" => "canvas",
        "cytoscape" => "html",
        f => f,
    };
    let out = out.unwrap_or_else(|| root.join(format!("graph.{ext}")));
    std::fs::write(&out, content)?;
    let data = serde_json::json!({
        "out": out.display().to_string(),
        "nodes": model.nodes.len(),
        "edges": model.edges.len()
    });
    emit("graph", data, json, start, |d| {
        println!(
            "wrote {} ({} nodes, {} edges)",
            d["out"].as_str().unwrap_or("graph"),
            d["nodes"],
            d["edges"]
        )
    });
    Ok(())
}

fn obsidian_canvas(model: &kgx_viz::model::GraphModel) -> anyhow::Result<String> {
    let nodes = model
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            serde_json::json!({
                "id": n.id,
                "type": "text",
                "text": n.title,
                "x": (i as i64 % 8) * 260,
                "y": (i as i64 / 8) * 120,
                "width": 220,
                "height": 80
            })
        })
        .collect::<Vec<_>>();
    let edges = model
        .edges
        .iter()
        .enumerate()
        .map(|(i, e)| {
            serde_json::json!({
                "id": format!("e{i}"),
                "fromNode": e.src,
                "toNode": e.dst,
                "label": e.rel
            })
        })
        .collect::<Vec<_>>();
    Ok(serde_json::to_string_pretty(
        &serde_json::json!({ "nodes": nodes, "edges": edges }),
    )?)
}
