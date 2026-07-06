use assert_cmd::Command;
mod common;

#[test]
fn index_communities_writes_summaries_and_moc() {
    let d = common::copy_fixture();
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full", "--communities", "--json"])
        .current_dir(d.path())
        .assert()
        .success();
    let conn = rusqlite::Connection::open(d.path().join(".brain/.kg/brain.sqlite")).unwrap();
    let n: i64 = conn
        .query_row("SELECT count(*) FROM community_summaries", [], |r| r.get(0))
        .unwrap();
    assert!(n >= 1);
    let moc = std::fs::read_dir(d.path().join(".brain/notes/moc"))
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| e.file_name().to_string_lossy().starts_with("community-"));
    assert!(moc, "no community MOC materialized");
}
