pub const SCHEMA_VERSION: i32 = 4;

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS notes (
  id TEXT PRIMARY KEY, path TEXT NOT NULL, title TEXT, type TEXT NOT NULL, status TEXT NOT NULL,
  valid_from TEXT, valid_to TEXT, recorded_at TEXT, tags TEXT, raw_text TEXT, embedding BLOB,
  entity_type TEXT);
CREATE TABLE IF NOT EXISTS edges (
  src_id TEXT NOT NULL, dst_id TEXT NOT NULL, rel_type TEXT NOT NULL,
  valid_from TEXT, valid_to TEXT, PRIMARY KEY (src_id, dst_id, rel_type));
CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(id, raw_text, tags, tokenize='porter');
CREATE TABLE IF NOT EXISTS pagerank (id TEXT PRIMARY KEY, score REAL);
CREATE TABLE IF NOT EXISTS communities (id TEXT, community_id INTEGER, PRIMARY KEY (id, community_id));
CREATE TABLE IF NOT EXISTS community_summaries (
  community_id INTEGER PRIMARY KEY, title TEXT, summary TEXT, member_count INTEGER);
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT);
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS idx_edges_src ON edges(src_id);
CREATE INDEX IF NOT EXISTS idx_edges_dst ON edges(dst_id);
CREATE INDEX IF NOT EXISTS idx_notes_type ON notes(type);
CREATE TABLE IF NOT EXISTS sparse_postings (
  term_id INTEGER NOT NULL, note_id TEXT NOT NULL, weight REAL NOT NULL,
  PRIMARY KEY (term_id, note_id));
CREATE INDEX IF NOT EXISTS idx_sparse_note ON sparse_postings(note_id);
"#;
