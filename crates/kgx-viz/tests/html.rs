use kgx_graph::{build::build_full, embed::MockEmbedder, Brain};
use kgx_vault::scan::scan_vault;
use kgx_viz::{html, mermaid::render as mermaid, model::from_brain};
use std::collections::BTreeSet;

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
fn communities_take_minimum_on_duplicates() {
    let notes = scan_vault(&fixture()).unwrap();
    let mut brain = Brain::open_in_memory().unwrap();
    build_full(&mut brain, &notes, &MockEmbedder::new()).unwrap();
    kgx_graph::community::detect(&mut brain, 0).unwrap();

    let first_id = from_brain(&brain, None).unwrap().nodes[0].id.clone();
    brain
        .conn_mut()
        .execute(
            "INSERT INTO communities (id, community_id) VALUES (?1, ?2)",
            rusqlite::params![&first_id, 9999],
        )
        .unwrap();

    let m = from_brain(&brain, None).unwrap();

    assert_eq!(m.nodes.len(), 17, "total node count must not change");

    let ids: BTreeSet<&str> = m.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(
        ids.len(),
        m.nodes.len(),
        "every node id must appear exactly once"
    );

    let first = m.nodes.iter().find(|n| n.id == first_id).unwrap();
    assert_ne!(
        first.community, 9999,
        "duplicate community should not override the minimum community"
    );
    assert!(
        first.community < 9999,
        "first node should keep the smaller detected community, got {}",
        first.community
    );
}

#[test]
fn mermaid_renders_edges() {
    let m = model();
    let s = mermaid(&m);
    assert!(s.starts_with("graph TD") || s.starts_with("flowchart"));
    assert!(s.contains("-->"));
}
