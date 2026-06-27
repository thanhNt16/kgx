use kgx_core::{Confidence, CreatedBy, CreatedVia, Frontmatter, Note, NoteType, Status};
use kgx_extract::pipeline::{extract, Intensity};
use kgx_llm::MockProvider;
use std::path::PathBuf;

fn source() -> Note {
    Note {
        rel_path: PathBuf::from("raw/2026-01-15-arch-review.md"),
        body: "Postgres is the primary datastore. Billing Service depends on it.".into(),
        fm: Frontmatter {
            r#type: NoteType::Source,
            id: "01RAW01ARCHREVIEW00000000".into(),
            title: "Arch Review".into(),
            status: Status::Active,
            valid_from: None,
            valid_to: None,
            recorded_at: None,
            supersedes: vec![],
            superseded_by: None,
            source: None,
            confidence: Confidence::High,
            sources_count: 0,
            tags: vec![],
            links: vec![],
            entity_type: None,
            aliases: vec![],
            created_by: CreatedBy::Human,
            created_via: CreatedVia::Cli,
            extra: Default::default(),
        },
    }
}

#[tokio::test]
async fn extract_produces_facts_with_provenance() {
    let p = MockProvider::new();
    let res = extract(&p, &source(), Intensity::Full).await.unwrap();
    assert!(
        res.notes.len() >= 2,
        "expected >=2 facts, got {}",
        res.notes.len()
    );
    for n in &res.notes {
        assert_eq!(n.fm.created_by, CreatedBy::Agent);
        let src = n.fm.source.as_ref().expect("missing source");
        assert!(
            src.contains("raw/2026-01-15-arch-review"),
            "bad provenance: {src}"
        );
        assert!(n.fm.recorded_at.is_some());
    }
}
