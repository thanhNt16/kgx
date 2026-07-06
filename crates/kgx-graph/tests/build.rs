use kgx_graph::{
    build::{build_full, derive_edges},
    embed::MockEmbedder,
    Brain,
};
use kgx_vault::scan::scan_vault;

fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min")
}

#[test]
fn build_full_populates_nodes_and_edges_deterministically() {
    let notes = scan_vault(&fixture()).unwrap();
    let mut b1 = Brain::open_in_memory().unwrap();
    let mut b2 = Brain::open_in_memory().unwrap();
    let s1 = build_full(&mut b1, &notes, &MockEmbedder::new()).unwrap();
    let s2 = build_full(&mut b2, &notes, &MockEmbedder::new()).unwrap();
    assert_eq!(s1.nodes, s2.nodes);
    assert_eq!(s1.edges, s2.edges);
    assert_eq!(s1.nodes, notes.len());
    assert!(s1.edges > 0);
    let cnt: i64 = b1
        .conn()
        .query_row("SELECT count(*) FROM notes_fts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cnt as usize, notes.len());
    let titled: i64 = b1
        .conn()
        .query_row(
            "SELECT count(*) FROM notes WHERE title IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(titled as usize, notes.len());
}

#[test]
fn derive_edges_includes_supersedes_and_source() {
    let notes = scan_vault(&fixture()).unwrap();
    let edges = derive_edges(&notes);
    assert!(edges
        .iter()
        .any(|e| matches!(e.rel_type, kgx_core::RelType::DerivedFrom)));
}
