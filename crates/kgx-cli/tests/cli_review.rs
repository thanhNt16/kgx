use assert_cmd::Command;
mod common;

#[test]
fn review_approve_all_applies_soft_but_blocks_hard() {
    let d = common::copy_fixture();
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full"])
        .current_dir(d.path())
        .assert()
        .success();

    // `kg dream` was removed (consolidation is harness-driven now). Stage a
    // diff manually the same way the kgx:dream skill does: write a soft
    // supersede against f-postgres-primary.md + a hard diff that --approve all
    // must block.
    let pg_path = d.path().join(".brain/notes/facts/f-postgres-primary.md");
    let before = std::fs::read_to_string(&pg_path).unwrap();
    let after = before.replace("status: active", "status: superseded") + "valid_to: 2026-07-07\n";
    let staged = serde_json::json!([
        {
            "id": "01SOFT00000000000000000001",
            "pass": "supersession",
            "kind": "supersede",
            "rationale": "soft supersede of postgres-primary",
            "severity": "soft",
            "files": [{
                "rel_path": "notes/facts/f-postgres-primary.md",
                "before": before,
                "after": after,
            }]
        },
        {
            "id": "01HARD00000000000000000001",
            "pass": "contradiction",
            "kind": "flag_contradiction",
            "rationale": "hard block: destructive contradiction",
            "severity": "hard",
            "files": [{
                "rel_path": "notes/facts/f-cockroach-primary.md",
                "before": null,
                "after": "---\ntype: fact\nid: 01HARDX00000000000000000A\ntitle: \"hard\"\nstatus: active\n---\nblocked\n",
            }]
        }
    ]);
    std::fs::create_dir_all(d.path().join(".brain/.kg")).unwrap();
    std::fs::write(
        d.path().join(".brain/.kg/staged_diffs.json"),
        serde_json::to_string_pretty(&staged).unwrap(),
    )
    .unwrap();

    let out = Command::cargo_bin("kg")
        .unwrap()
        .args(["review", "--approve", "all", "--json"])
        .current_dir(d.path())
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert!(
        v["data"]["applied"].as_u64().unwrap() >= 1,
        "soft diff should apply: {v}"
    );
    assert!(
        v["data"]["blocked_hard"].as_u64().unwrap() >= 1,
        "hard diff should block: {v}"
    );
    let pg =
        std::fs::read_to_string(d.path().join(".brain/notes/facts/f-postgres-primary.md")).unwrap();
    assert!(
        pg.contains("status: superseded"),
        "soft supersede not applied: {pg}"
    );
}
