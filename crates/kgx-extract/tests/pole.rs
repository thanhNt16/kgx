use kgx_core::llm::{LlmProvider, LlmRequest, LlmResponse};
use kgx_core::{EntityType, NoteType};
use kgx_extract::pipeline::{extract, Intensity};

struct StubProvider(String);

#[async_trait::async_trait]
impl LlmProvider for StubProvider {
    async fn complete(&self, _req: LlmRequest) -> kgx_core::Result<LlmResponse> {
        Ok(LlmResponse {
            text: self.0.clone(),
            input_tokens: 1,
            output_tokens: 1,
            model: "stub".into(),
        })
    }

    fn model_id(&self) -> &str {
        "stub"
    }
}

fn source_note() -> kgx_core::Note {
    kgx_core::Note {
        body: "Alice decided to migrate billing to Iceberg in Dublin.".into(),
        rel_path: std::path::PathBuf::from("raw/2026-07-05-adr.md"),
        fm: kgx_core::Frontmatter {
            r#type: NoteType::Source,
            id: "01STUB".into(),
            title: "adr".into(),
            status: kgx_core::Status::Active,
            valid_from: None,
            valid_to: None,
            recorded_at: None,
            supersedes: vec![],
            superseded_by: None,
            source: None,
            confidence: kgx_core::Confidence::Medium,
            sources_count: 1,
            tags: vec![],
            links: vec![],
            entity_type: None,
            aliases: vec![],
            created_by: Default::default(),
            created_via: Default::default(),
            extra: Default::default(),
        },
    }
}

const POLE_RESPONSE: &str = r#"{"facts":[{
  "title":"Billing migrates to Iceberg",
  "body":"Alice decided the billing ledger migrates to Iceberg.",
  "confidence":"high",
  "entities":[
    {"name":"Alice","entity_type":"person","rel":"decided"},
    {"name":"Apache Iceberg","entity_type":"object"},
    {"name":"Dublin","entity_type":"location","rel":"located_at"},
    "legacy-bare-entity"
  ]}]}"#;

#[tokio::test]
async fn pole_entities_become_typed_entity_notes_and_relations() {
    let provider = StubProvider(POLE_RESPONSE.into());
    let out = extract(&provider, &source_note(), Intensity::Full)
        .await
        .unwrap();

    let facts: Vec<_> = out
        .notes
        .iter()
        .filter(|n| n.fm.r#type == NoteType::Fact)
        .collect();
    let entities: Vec<_> = out
        .notes
        .iter()
        .filter(|n| n.fm.r#type == NoteType::Entity)
        .collect();
    assert_eq!(facts.len(), 1);
    assert_eq!(entities.len(), 4);

    let alice = entities.iter().find(|e| e.fm.title == "Alice").unwrap();
    assert_eq!(
        alice.fm.entity_type.as_deref(),
        Some(EntityType::Person.as_str())
    );
    assert!(alice.rel_path.starts_with("notes/entities"));
    assert!(alice.fm.source.as_deref().unwrap().contains("raw/"));

    let bare = entities
        .iter()
        .find(|e| e.fm.title == "legacy-bare-entity")
        .unwrap();
    assert_eq!(bare.fm.entity_type, None);

    let fact = facts[0];
    for name in ["Alice", "Apache Iceberg", "Dublin", "legacy-bare-entity"] {
        assert!(
            fact.fm.links.iter().any(|l| l.contains(name)),
            "fact should link {name}"
        );
    }

    let rels = fact.fm.extra.get("relations").expect("relations key");
    let rels: Vec<(String, String)> = rels
        .as_sequence()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m.get("target").unwrap().as_str().unwrap().to_string(),
                m.get("rel").unwrap().as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert!(rels.contains(&("Alice".into(), "decided".into())));
    assert!(rels.contains(&("Dublin".into(), "located_at".into())));
    assert_eq!(rels.len(), 2, "mentions/untyped rels are not recorded");
}

#[tokio::test]
async fn legacy_string_entities_still_work() {
    let provider = StubProvider(
        r#"{"facts":[{"title":"T","body":"B","confidence":"medium","entities":["alpha","beta"]}]}"#
            .into(),
    );
    let out = extract(&provider, &source_note(), Intensity::Full)
        .await
        .unwrap();
    assert_eq!(
        out.notes
            .iter()
            .filter(|n| n.fm.r#type == NoteType::Fact)
            .count(),
        1
    );
    assert_eq!(
        out.notes
            .iter()
            .filter(|n| n.fm.r#type == NoteType::Entity)
            .count(),
        2
    );
}
