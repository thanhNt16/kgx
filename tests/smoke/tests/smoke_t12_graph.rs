use assert_cmd::Command;
mod common;

#[test]
fn t12_graph_html_counts_match_brain() {
    let d = common::copy_fixture();
    common::run_index(d.path());
    let out = d.path().join("graph.html");
    Command::cargo_bin("kg")
        .unwrap()
        .args(["graph", "--format", "html", "--out"])
        .arg(&out)
        .current_dir(d.path())
        .assert()
        .success();
    let html = std::fs::read_to_string(out).unwrap();
    let conn = rusqlite::Connection::open(d.path().join(".kg/brain.sqlite")).unwrap();
    let n: i64 = conn
        .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
        .unwrap();
    assert_eq!(html.matches("\"title\":").count() as i64, n);
}
