use kgx_core::llm::Embedder;
use kgx_graph::{
    build::build_full,
    embed::MockEmbedder,
    knn::vector_search,
    query::{bm25_search, neighbors},
    Brain,
};
use kgx_vault::scan::scan_vault;

fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min")
}

fn built() -> Brain {
    let notes = scan_vault(&fixture()).unwrap();
    let mut b = Brain::open_in_memory().unwrap();
    build_full(&mut b, &notes, &MockEmbedder::new()).unwrap();
    b
}

#[test]
fn bm25_finds_postgres() {
    let b = built();
    let hits = bm25_search(&b, "primary datastore Postgres", 5).unwrap();
    assert!(!hits.is_empty());
}

#[test]
fn vector_search_returns_ranked() {
    let b = built();
    let q = MockEmbedder::new()
        .embed(&["primary datastore".into()])
        .unwrap()
        .remove(0);
    let hits = vector_search(&b, &q, 3).unwrap();
    assert!(hits.windows(2).all(|w| w[0].1 >= w[1].1));
}

#[test]
fn neighbors_one_hop() {
    let b = built();
    let pg = "01FACT01POSTGRESPRIMARY00";
    let n = neighbors(&b, pg, 1).unwrap();
    assert!(!n.is_empty());
}
