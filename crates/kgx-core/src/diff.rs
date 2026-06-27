// crates/kgx-core/src/diff.rs — dream passes emit these; review consumes them.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProposedDiff {
    pub id: String,                      // ULID of the proposal
    pub pass: String,                    // "dedup" | "contradiction" | ...
    pub kind: DiffKind,
    pub rationale: String,
    pub severity: Severity,              // affects auto-commit gating
    pub files: Vec<FileChange>,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind { Merge, Supersede, Archive, AddLink, AddNote, Resummarize, FlagContradiction }
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity { Info, Soft, Scope, Hard }  // Hard blocks auto-commit (T07)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileChange {
    pub rel_path: String,
    pub before: Option<String>,
    pub after: Option<String>,
}
