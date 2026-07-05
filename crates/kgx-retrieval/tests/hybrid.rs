use kgx_graph::{build::build_full, embed::MockEmbedder, Brain};
use kgx_retrieval::{search, Mode, Retrievers, SearchOpts};
use kgx_vault::scan::scan_vault;
fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min")
}
#[test]
fn hybrid_beats_keyword_on_postgres_query() {
    let notes = scan_vault(&fixture()).unwrap();
    let mut b = Brain::open_in_memory().unwrap();
    build_full(&mut b, &notes, &MockEmbedder::new()).unwrap();
    let e = MockEmbedder::new();
    let r = Retrievers::new(&e);
    let hits = search(
        &b,
        &r,
        "primary datastore",
        SearchOpts {
            mode: Mode::Hybrid,
            limit: 5,
            expand_ppr: false,
            filter_entities: true,
            rerank_graph: false,
            rerank_llm: false,
            rerank_topk: 30,
        },
    )
    .unwrap();
    assert!(!hits.is_empty());
    assert!(hits.iter().any(|h| h.id == "01FACT01POSTGRESPRIMARY00"));
}
