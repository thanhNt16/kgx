pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS notes (
  id TEXT PRIMARY KEY, path TEXT NOT NULL, type TEXT NOT NULL, status TEXT NOT NULL,
  valid_from TEXT, valid_to TEXT, recorded_at TEXT, tags TEXT, raw_text TEXT, embedding BLOB);
CREATE TABLE IF NOT EXISTS edges (
  src_id TEXT NOT NULL, dst_id TEXT NOT NULL, rel_type TEXT NOT NULL,
  valid_from TEXT, valid_to TEXT, PRIMARY KEY (src_id, dst_id, rel_type));
CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(id, raw_text, tags, content='', tokenize='porter');
CREATE TABLE IF NOT EXISTS pagerank (id TEXT PRIMARY KEY, score REAL);
CREATE TABLE IF NOT EXISTS communities (id TEXT, community_id INTEGER, PRIMARY KEY (id, community_id));
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT);
CREATE INDEX IF NOT EXISTS idx_edges_src ON edges(src_id);
CREATE INDEX IF NOT EXISTS idx_edges_dst ON edges(dst_id);
CREATE INDEX IF NOT EXISTS idx_notes_type ON notes(type);
"#;
