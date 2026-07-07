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
}

#[test]
fn mermaid_renders_edges() {
    let m = model();
    let s = mermaid(&m);
    assert!(s.starts_with("graph TD") || s.starts_with("flowchart"));
    assert!(s.contains("-->"));
}
