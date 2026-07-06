use assert_cmd::Command;
mod common;

#[test]
fn graph_html_node_count_matches_brain() {
    let d = common::copy_fixture();
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full", "--pagerank"])
        .current_dir(d.path())
        .assert()
        .success();
    let out = d.path().join("graph.html");
    Command::cargo_bin("kg")
        .unwrap()
        .args(["graph", "--format", "html", "--out"])
        .arg(&out)
        .current_dir(d.path())
        .assert()
        .success();
    let html = std::fs::read_to_string(&out).unwrap();
    let conn = rusqlite::Connection::open(d.path().join(".brain/.kg/brain.sqlite")).unwrap();
    let n: i64 = conn
        .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
        .unwrap();
    let embedded = html.matches("\"title\":").count() as i64;
    assert_eq!(embedded, n);
}
