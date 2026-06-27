use assert_cmd::Command;
mod common;

#[test]
fn extract_creates_facts_with_provenance() {
    let d = common::copy_fixture();
    // notes/facts dir may not exist yet — create it for the before count
    std::fs::create_dir_all(d.path().join("notes/facts")).unwrap();
    let before = std::fs::read_dir(d.path().join("notes/facts"))
        .unwrap()
        .count();
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args([
            "extract",
            "--source",
            "01RAW01ARCHREVIEW00000000",
            "--intensity",
            "full",
        ])
        .current_dir(d.path())
        .assert()
        .success();
    let after = std::fs::read_dir(d.path().join("notes/facts"))
        .unwrap()
        .count();
    assert!(after > before, "no new facts written");
    for e in std::fs::read_dir(d.path().join("notes/facts")).unwrap() {
        let c = std::fs::read_to_string(e.unwrap().path()).unwrap();
        assert!(c.contains("source:"), "fact missing provenance");
    }
}
