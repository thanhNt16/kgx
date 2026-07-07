use kgx_graph::{build::build_full, embed::MockEmbedder, Brain};
use kgx_vault::scan::scan_vault;
use kgx_viz::{html, mermaid::render as mermaid, model::from_brain};

fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min/.brain")
}

fn model() -> kgx_viz::model::GraphModel {
    let notes = scan_vault(&fixture()).unwrap();
    let mut brain = Brain::open_in_memory().unwrap();
    build_full(&mut brain, &notes, &MockEmbedder::new()).unwrap();
    from_brain(&brain, None).unwrap()
}

#[test]
fn html_renders_with_3d_viewer_and_counts_match() {
    let m = model();
    let h = html::render(&m);
    assert!(h.contains("<html") && h.contains("</html>"));
    assert!(h.contains("esm.sh/three"), "expected Three.js CDN import");
    assert!(h.contains("\"nodes\":"));
    assert_eq!(m.nodes.len(), 17);
    assert_eq!(h.matches("\"title\":").count(), m.nodes.len());
    assert_eq!(h.matches("\"community\":").count(), m.nodes.len(),
        "every node should include a community field");
}

#[test]
fn communities_do_not_duplicate_nodes() {
    let notes = scan_vault(&fixture()).unwrap();
    let mut brain = Brain::open_in_memory().unwrap();
    build_full(&mut brain, &notes, &MockEmbedder::new()).unwrap();
    kgx_graph::community::detect(&mut brain, 0).unwrap();

    let m = from_brain(&brain, None).unwrap();
    let html = html::render(&m);

    // Every node must have a resolved community (not the -1 fallback).
    for node in &m.nodes {
        assert!(
            node.community >= 0,
            "node {} should have a community >= 0, got {}",
            node.id,
            node.community
        );
    }

    // The HTML serialization must include a community field for every node.
    assert_eq!(
        html.matches("\"community\":").count(),
        m.nodes.len(),
        "every node should include a community field in the HTML output"
    );
}

#[test]
fn mermaid_renders_edges() {
    let m = model();
    let s = mermaid(&m);
    assert!(s.starts_with("graph TD") || s.starts_with("flowchart"));
    assert!(s.contains("-->"));
}
