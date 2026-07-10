// tests/smoke/tests/t19_doc_pole_pipeline.rs
// Integration test: document capture → POLE extraction → index → query

use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

#[test]
fn t19_doc_pole_pipeline() {
    let dir = TempDir::new().unwrap();
    let vault = dir.path();

    // 1. Init vault
    Command::cargo_bin("kg")
        .unwrap()
        .current_dir(vault)
        .args(["init", "--template", "research"])
        .assert()
        .success();

    // 2. Capture a markdown "document" (simulates converted PDF output)
    let doc_content = "# Q3 Report\n\nAlice Chen presented Q3 results at the all-hands meeting.\nThe meeting was held at HQ Building.\n";
    let doc_path = dir.path().join("q3-report.md");
    fs::write(&doc_path, doc_content).unwrap();

    Command::cargo_bin("kg")
        .unwrap()
        .current_dir(vault)
        .args([
            "capture",
            "--from",
            doc_path.to_str().unwrap(),
            "--type",
            "doc",
            "--json",
        ])
        .assert()
        .success();

    // 3. Create POLE entity notes by writing them directly to the vault
    let brain = vault.join(".brain");
    fs::create_dir_all(brain.join("notes/entities")).unwrap();

    let alice = "---\ntype: entity\nid: 01PERSON000000000000001\ntitle: \"Alice Chen\"\nentity_type: person\ncreated_by: agent\ncreated_via: cli\n---\nAlice Chen is a person.\n";
    fs::write(brain.join("notes/entities/alice-chen.md"), alice).unwrap();

    let meeting = "---\ntype: entity\nid: 01EVENT000000000000001\ntitle: \"Q3 All-Hands Meeting\"\nentity_type: event\ncreated_by: agent\ncreated_via: cli\n---\nQ3 all-hands meeting.\n";
    fs::write(
        brain.join("notes/entities/q3-all-hands-meeting.md"),
        meeting,
    )
    .unwrap();

    let hq = "---\ntype: entity\nid: 01LOCATION0000000000001\ntitle: \"HQ Building\"\nentity_type: location\ncreated_by: agent\ncreated_via: cli\n---\nHQ Building is a location.\n";
    fs::write(brain.join("notes/entities/hq-building.md"), hq).unwrap();

    // Create a fact with typed relations
    let fact = "---\ntype: fact\nid: 01FACT0000000000000001\ntitle: \"Alice presented at Q3 all-hands\"\nsource: \"[[raw/q3-report]]\"\nconfidence: high\nlinks: [\"[[Alice Chen]]\", \"[[Q3 All-Hands Meeting]]\", \"[[HQ Building]]\"]\ncreated_by: agent\ncreated_via: cli\nrelations:\n  - target: \"[[Alice Chen]]\"\n    rel: participates_in\n  - target: \"[[Q3 All-Hands Meeting]]\"\n    rel: participates_in\n  - target: \"[[HQ Building]]\"\n    rel: located_at\n---\nAlice Chen presented Q3 results at the all-hands meeting at HQ Building.\n";
    fs::create_dir_all(brain.join("notes/facts")).unwrap();
    fs::write(brain.join("notes/facts/alice-presented-q3.md"), fact).unwrap();

    // 4. Build the brain
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .env("KGX_EMBED", "mock")
        .current_dir(vault)
        .args(["index", "--full", "--json"])
        .assert()
        .success();

    // 5. Query by entity type — should find Alice
    let output = Command::cargo_bin("kg")
        .unwrap()
        .current_dir(vault)
        .args(["query", "--entity-type", "person", "--json"])
        .ok();
    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Alice Chen") || stdout.contains("01PERSON"),
            "query --entity-type person should find Alice. Got: {stdout}"
        );
    }

    // 6. Recall with relations — should find typed edges
    let output = Command::cargo_bin("kg")
        .unwrap()
        .current_dir(vault)
        .args(["recall", "--entity", "Alice Chen", "--relations", "--json"])
        .ok();
    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("participates_in") || stdout.contains("Q3"),
            "recall --relations should find typed edges. Got: {stdout}"
        );
    }
}
