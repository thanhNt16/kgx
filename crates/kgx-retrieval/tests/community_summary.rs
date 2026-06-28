use kgx_graph::{build::build_full, community::detect, embed::MockEmbedder, Brain};
use kgx_llm::mock::MockProvider;
use kgx_retrieval::community_summary::summarize_all;
use kgx_vault::scan::scan_vault;

fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min")
}

#[tokio::test]
async fn summaries_one_per_community() {
    let notes = scan_vault(&fixture()).unwrap();
    let mut b = Brain::open_in_memory().unwrap();
    build_full(&mut b, &notes, &MockEmbedder::new()).unwrap();
    let stats = detect(&mut b, 42).unwrap();
    let sums = summarize_all(&b, &MockProvider::new(), &notes)
        .await
        .unwrap();
    assert_eq!(sums.len(), stats.count);
    let cnt: i64 = b
        .conn()
        .query_row("SELECT count(*) FROM community_summaries", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cnt as usize, stats.count);
}
