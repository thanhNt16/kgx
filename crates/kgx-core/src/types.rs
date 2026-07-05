/// Note category. Serializes lowercase: "fact", "entity", etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteType {
    Fact,
    Entity,
    Decision,
    Experience,
    Moc,
    Source,
    Question,
    Preference,
    Friction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Active,
    Deprecated,
    Archived,
    Superseded,
}

impl Status {
    pub fn default_active() -> Status {
        Status::Active
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    pub fn default_medium() -> Confidence {
        Confidence::Medium
    }
}

/// POLE classification for entity notes (Person / Object / Location / Event).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    Person,
    Object,
    Location,
    Event,
}

impl EntityType {
    pub fn parse(s: &str) -> Option<EntityType> {
        match s.to_ascii_lowercase().as_str() {
            "person" => Some(Self::Person),
            "object" => Some(Self::Object),
            "location" => Some(Self::Location),
            "event" => Some(Self::Event),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Object => "object",
            Self::Location => "location",
            Self::Event => "event",
        }
    }
}

/// Edge relationship type stored in the brain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelType {
    LinksTo,
    Supersedes,
    DerivedFrom,
    Cites,
    MentionsEntity,
    Contradicts,
    ParticipatesIn,
    LocatedAt,
    Owns,
    Decided,
    Caused,
}

impl RelType {
    pub fn parse(s: &str) -> Option<RelType> {
        serde_json::from_value(serde_json::Value::String(s.to_string())).ok()
    }
}

/// Parsed frontmatter. Unknown keys preserved in `extra` (OKF §9 tolerance).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Frontmatter {
    pub r#type: NoteType,
    pub id: String, // ULID
    pub title: String,
    #[serde(default = "Status::default_active")]
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>, // ISO date
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recorded_at: Option<String>, // ISO datetime
    #[serde(default)]
    pub supersedes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>, // "[[raw/...]]"
    #[serde(default = "Confidence::default_medium")]
    pub confidence: Confidence,
    #[serde(default)]
    pub sources_count: u32,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub created_by: CreatedBy,
    #[serde(default)]
    pub created_via: CreatedVia,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CreatedBy {
    #[default]
    Human,
    Agent,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CreatedVia {
    #[default]
    Cli,
    Mcp,
    Sync,
}

/// A note = frontmatter + Markdown body + on-disk path (relative to vault root).
#[derive(Debug, Clone)]
pub struct Note {
    pub fm: Frontmatter,
    pub body: String,
    pub rel_path: std::path::PathBuf,
}

/// A graph edge (matches the `edges` SQL table).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub src_id: String,
    pub dst_id: String,
    pub rel_type: RelType,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frontmatter_roundtrips_minimal() {
        let yaml = "type: fact\nid: 01J9X2ABC\ntitle: Postgres is primary\n";
        let fm: Frontmatter = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(fm.r#type, NoteType::Fact);
        assert_eq!(fm.status, Status::Active); // defaulted
        assert_eq!(fm.confidence, Confidence::Medium); // defaulted
        let back = serde_yaml::to_string(&fm).unwrap();
        let fm2: Frontmatter = serde_yaml::from_str(&back).unwrap();
        assert_eq!(fm2.title, "Postgres is primary");
    }
    #[test]
    fn unknown_keys_preserved() {
        let yaml = "type: fact\nid: X\ntitle: T\ncustom_key: hello\n";
        let fm: Frontmatter = serde_yaml::from_str(yaml).unwrap();
        assert!(fm.extra.contains_key("custom_key"));
    }

    #[test]
    fn entity_type_parses_pole_only() {
        assert_eq!(EntityType::parse("person"), Some(EntityType::Person));
        assert_eq!(EntityType::parse("OBJECT"), Some(EntityType::Object));
        assert_eq!(EntityType::parse("location"), Some(EntityType::Location));
        assert_eq!(EntityType::parse("event"), Some(EntityType::Event));
        assert_eq!(EntityType::parse("system"), None);
        assert_eq!(EntityType::Person.as_str(), "person");
    }

    #[test]
    fn rel_type_parses_snake_case_typed_relations() {
        assert_eq!(
            RelType::parse("participates_in"),
            Some(RelType::ParticipatesIn)
        );
        assert_eq!(RelType::parse("located_at"), Some(RelType::LocatedAt));
        assert_eq!(RelType::parse("owns"), Some(RelType::Owns));
        assert_eq!(RelType::parse("decided"), Some(RelType::Decided));
        assert_eq!(RelType::parse("caused"), Some(RelType::Caused));
        assert_eq!(RelType::parse("links_to"), Some(RelType::LinksTo));
        assert_eq!(RelType::parse("nonsense"), None);
    }
}
