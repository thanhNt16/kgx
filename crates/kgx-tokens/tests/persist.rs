use kgx_tokens::aggregate::{summarize, GroupBy};
use kgx_tokens::record::{append, TokenRecord};

fn rec(op: &str, i: u32, o: u32) -> TokenRecord {
    TokenRecord {
        model: "mock".into(),
        operation: op.into(),
        command: "index".into(),
        input_tokens: i,
        output_tokens: o,
        elapsed_ms: 5,
        correlation_id: "c1".into(),
        ts: "2026-06-27T10:00:00Z".into(),
    }
}

#[test]
fn append_then_aggregate_by_operation() {
    let d = tempfile::tempdir().unwrap();
    append(d.path(), &rec("embed", 100, 0)).unwrap();
    append(d.path(), &rec("embed", 50, 0)).unwrap();
    append(d.path(), &rec("extract", 200, 80)).unwrap();
    let mut aggs = summarize(d.path(), 30, GroupBy::Operation).unwrap();
    aggs.sort_by(|a, b| a.key.cmp(&b.key));
    assert_eq!(aggs.len(), 2);
    let embed = aggs.iter().find(|a| a.key == "embed").unwrap();
    assert_eq!(embed.input_tokens, 150);
    assert_eq!(embed.count, 2);
}
