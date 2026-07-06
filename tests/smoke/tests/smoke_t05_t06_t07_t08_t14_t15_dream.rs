/// T05/T06/T07/T08/T14/T15: dream/review smoke coverage through the real kg binary.
use assert_cmd::Command;

mod common;

#[test]
fn t05_t07_t08_t15_dream_stages_then_review_applies_soft_and_blocks_hard() {
    let d = common::copy_fixture();
    common::run_index(d.path());

    let pg_path = d.path().join(".brain/notes/facts/f-postgres-primary.md");
    let pg_before = std::fs::read_to_string(&pg_path).unwrap();
    let dream = Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["dream", "--max-iterations", "3", "--json"])
        .current_dir(d.path())
        .assert()
        .success();
    let dream_json: serde_json::Value = serde_json::from_slice(&dream.get_output().stdout).unwrap();

    assert!(d.path().join(".brain/.kg/staged_diffs.json").exists());
    assert_eq!(pg_before, std::fs::read_to_string(&pg_path).unwrap());
    assert!(dream_json["data"]["iterations"].as_u64().unwrap() <= 3);
    assert!(dream_json["data"]["hard_blocks"].as_u64().unwrap() >= 1);

    let review = Command::cargo_bin("kg")
        .unwrap()
        .args(["review", "--approve", "all", "--json"])
        .current_dir(d.path())
        .assert()
        .success();
    let review_json: serde_json::Value =
        serde_json::from_slice(&review.get_output().stdout).unwrap();

    assert!(review_json["data"]["blocked_hard"].as_u64().unwrap() >= 1);
    let pg_after = std::fs::read_to_string(&pg_path).unwrap();
    assert!(pg_after.contains("status: superseded"));
    assert!(pg_after.contains("valid_to:"));
    assert!(pg_path.exists());
}

#[test]
fn t06_dedup_merge_archives_duplicate_and_keeps_files() {
    let d = common::copy_fixture();
    let dup_path = d.path().join(".brain/notes/facts/f-postgres-primary-copy.md");
    std::fs::copy(
        d.path().join(".brain/notes/facts/f-postgres-primary.md"),
        &dup_path,
    )
    .unwrap();
    let mut dup = std::fs::read_to_string(&dup_path).unwrap();
    dup = dup.replace("01FACT01POSTGRESPRIMARY00", "01FACT99POSTGRESDUP000000");
    dup = dup.replace(
        "Postgres is the primary datastore",
        "Postgres is the primary datastore duplicate",
    );
    std::fs::write(&dup_path, dup).unwrap();

    common::run_index(d.path());
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["dream", "--only", "dedup", "--max-iterations", "1"])
        .current_dir(d.path())
        .assert()
        .success();
    Command::cargo_bin("kg")
        .unwrap()
        .args(["review", "--approve", "all"])
        .current_dir(d.path())
        .assert()
        .success();

    assert!(d.path().join(".brain/notes/facts/f-postgres-primary.md").exists());
    assert!(dup_path.exists());
    assert!(std::fs::read_to_string(dup_path)
        .unwrap()
        .contains("status: archived"));
}

#[test]
fn t14_stale_archival_retains_note_file() {
    let d = common::copy_fixture();
    let stale_path = d.path().join(".brain/notes/facts/f-stale.md");
    std::fs::write(
        &stale_path,
        "---\ntype: fact\nid: 01FACT99STALE0000000000\ntitle: Stale fact\nstatus: active\nvalid_from: 2020-01-01\nsource: \"[[raw/missing-source]]\"\ncreated_by: agent\ncreated_via: cli\n---\n\nThis fact has a missing old source.\n",
    )
    .unwrap();

    common::run_index(d.path());
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["dream", "--only", "staleness", "--max-iterations", "1"])
        .current_dir(d.path())
        .assert()
        .success();
    Command::cargo_bin("kg")
        .unwrap()
        .args(["review", "--approve", "all"])
        .current_dir(d.path())
        .assert()
        .success();

    assert!(stale_path.exists());
    assert!(std::fs::read_to_string(stale_path)
        .unwrap()
        .contains("status: archived"));
}
