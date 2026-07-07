use kgx_graph::{build::build_full, embed::MockEmbedder, Brain};
use kgx_retrieval::{search, Mode, Retrievers, SearchOpts};
use kgx_vault::scan::scan_vault;
fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min/.brain")
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

#[test]
fn search_deduplicates_repeated_titles_before_limit() {
    let tmp = tempfile::tempdir().unwrap();
    let notes_dir = tmp.path().join("notes");
    std::fs::create_dir_all(&notes_dir).unwrap();
    for (slug, id, title, heading) in [
        (
            "dup-a",
            "01DUPLICATE00000000000001",
            "standardized s3 object storage data governance",
            "DL-10101 (Sprint 101): standardized s3 object storage data governance",
        ),
        (
            "dup-b",
            "01DUPLICATE00000000000002",
            "standardized s3 object storage data governance",
            "DL-20202 (Sprint 202): standardized s3 object storage data governance",
        ),
        (
            "target",
            "01TARGET0000000000000003",
            "S3 lifecycle: tier to Glacier after 90 days",
            "ADR-06 (Sprint 12): S3 lifecycle: tier to Glacier after 90 days",
        ),
    ] {
        std::fs::write(
            notes_dir.join(format!("{slug}.md")),
            format!(
                "---\ntype: fact\nid: {id}\ntitle: \"{title}\"\nstatus: active\ntags: [storage]\nlinks: []\n---\n# {heading}\n\nstandardized s3 object storage data governance\n"
            ),
        )
        .unwrap();
    }
    let notes = scan_vault(tmp.path()).unwrap();
    let mut b = Brain::open_in_memory().unwrap();
    build_full(&mut b, &notes, &MockEmbedder::new()).unwrap();
    let e = MockEmbedder::new();
    let r = Retrievers::new(&e);

    let hits = search(
        &b,
        &r,
        "standardized s3 object storage governance",
        SearchOpts {
            mode: Mode::Keyword,
            limit: 2,
            expand_ppr: false,
            filter_entities: false,
            rerank_graph: false,
            rerank_llm: false,
            rerank_topk: 0,
        },
    )
    .unwrap();

    assert_eq!(hits.len(), 2);
    assert!(
        hits.iter().any(|h| h.id == "01TARGET0000000000000003"),
        "limited search should not spend both slots on duplicate titles: {hits:?}"
    );
}
