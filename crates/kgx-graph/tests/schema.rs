use kgx_graph::brain::Brain;

#[test]
fn open_creates_all_tables() {
    let b = Brain::open_in_memory().unwrap();
    let tables: Vec<String> = b
        .conn()
        .prepare("SELECT name FROM sqlite_master WHERE type IN ('table','view') ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    for expected in ["communities", "edges", "meta", "notes", "notes_fts", "pagerank"] {
        assert!(
            tables.iter().any(|t| t == expected),
            "missing table {expected}; have {tables:?}"
        );
    }
}
