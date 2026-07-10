# Document Ingestion, POLE Graph, and URL Crawl — Design Spec

**Date:** 2026-07-10
**Status:** Approved (pending user review)

## Goal

Add four capabilities to the KGX toolset (CLI, MCP, skills):
1. Document ingestion (PDF, Excel, Word, PPTX, etc.) via bundled pandoc + native Rust parsing, preserving text and tables.
2. A reusable `kgx:pole` skill that guides any agent harness through POLE (Person/Object/Location/Event) graph extraction from ingested content.
3. Index the POLE graph into the existing brain for later query, with new query filters.
4. URL/link ingestion with configurable crawl depth (same-domain, max-pages cap).

## Architecture

A new `kgx-convert` crate handles document-to-markdown conversion. Existing commands auto-detect by file extension — no new commands. A new `kgx:pole` skill guides POLE extraction. Query enhancements surface POLE-typed data from the brain (no schema migration needed).

```
Document/URL → kgx-convert (pandoc/Rust) → markdown
    → kg capture / ingest_file → raw/<date>-<slug>.md (immutable)
    → kgx:pole skill → agent extracts POLE entities + facts → upsert_note
    → kg index --full → brain.sqlite (entity_type + typed edges)
    → kg query --entity-type / kg recall --relations → queryable POLE graph
```

**Crate dependency graph change:**
```
kgx-convert (new) ← depends on: kgx-core (for KgError only)
kgx-cli → adds kgx-convert
kgx-mcp → adds kgx-convert
```

## Component 1: `kgx-convert` crate

**Single responsibility:** take a file path (any supported format), return structured markdown. No vault/brain knowledge.

### Format routing by extension

| Extension | Engine | Output |
|---|---|---|
| `.md`, `.txt`, `.markdown`, `.mdx` | Passthrough | Raw text as-is |
| `.docx`, `.pptx`, `.odt`, `.epub`, `.html`, `.htm` | Bundled pandoc → `--to gfm` | GitHub-flavored markdown with tables, headings, lists |
| `.pdf` | `pdf-extract` crate (pure Rust) | Extracted text, page breaks as `\n---\n` |
| `.xlsx`, `.xls` | `calamine` crate (pure Rust) | Each sheet → `## SheetName` heading + markdown table |

### Pandoc bundling

- Release archive (`install.sh`) downloads the platform-appropriate pandoc binary and places it at `~/.local/bin/pandoc-kgx` (versioned, namespaced to avoid conflicts with system pandoc).
- `kgx-convert` resolves pandoc path: `$KGX_PANDOC` env var → `~/.local/bin/pandoc-kgx` → system `pandoc` → error with install instructions.
- Build-from-source users need pandoc installed separately (documented in README). `dev-install.sh` can optionally install it.
- Pandoc invoked as subprocess: `pandoc-kgx <input> --to gfm --wrap=none`. Output captured from stdout. Non-zero exit → `KgError::Convert`.

### ConvertOutput struct

```rust
pub enum SourceFormat {
    Pdf,
    Docx,
    Xlsx,
    Pptx,
    Odt,
    Epub,
    Html,
    Markdown,
    Text,
}

pub struct ConvertOutput {
    pub markdown: String,
    pub title: String,           // first heading or filename stem
    pub source_format: SourceFormat,
}

pub fn convert(path: &Path) -> Result<ConvertOutput>;
pub fn convert_bytes(content: &str, ext: &str) -> Result<ConvertOutput>;
pub fn is_document_ext(ext: &str) -> bool;
pub const SUPPORTED_EXTS: &[&str] = &[
    "md", "txt", "markdown", "mdx",
    "pdf", "docx", "pptx", "odt", "epub", "html", "htm",
    "xlsx", "xls",
];
```

### Structure preservation (text + tables only)

- Pandoc `gfm` output preserves: headings, tables, lists, blockquotes, code blocks — all indexable by FTS5 and vector embeddings.
- PDF: `pdf-extract` gives plain text. Multi-column layouts may interleave — acceptable trade-off.
- Excel: `calamine` reads cell values. Each sheet → `## <SheetName>\n\n| col1 | col2 |\n|---|---|\n| val1 | val2 |`. Empty cells → empty string. Sheets with no data are skipped.

### Error handling

- Unknown extension → `KgError::Convert("unsupported format: .xyz. Supported: pdf, docx, pptx, ...")`.
- Pandoc not found → `KgError::Convert("pandoc not found. Set KGX_PANDOC or install pandoc.")`.
- Pandoc failure → `KgError::Convert("pandoc failed: <stderr>")`.
- PDF parse failure → `KgError::Convert("pdf extraction failed: <detail>")`.
- Empty result (scanned PDF) → return markdown with: `[No extractable text — this may be a scanned/image-only document]`.

## Component 2: CLI integration (`kg capture`)

### Single-file capture with auto-detection

`capture.rs` modified: before reading file content, check if the extension is a document format. If so, run `kgx-convert::convert(path)` and capture the resulting markdown instead of the raw file bytes.

```rust
// In capture_one_returning, before reading content:
if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
    if kgx_convert::is_document_ext(ext) && ext != "md" && ext != "txt" {
        let converted = kgx_convert::convert(path)?;
        // capture the markdown content, not the raw file
    }
}
```

### Directory walks

Default extension list expanded to include document formats:
```rust
const DEFAULT_TEXT_EXTS: &[&str] = &[
    "md", "txt", "markdown", "mdx",
    "pdf", "docx", "pptx", "odt", "epub", "html", "htm",
    "xlsx", "xls",
];
```

Files are now auto-converted instead of skipped. The walk reads the file, passes it through `kgx-convert` if needed, then captures the markdown.

### URL capture (currently disabled — enable it)

The `capture.rs` URL branch (`url if url.starts_with("http")`) currently bails. Replace with:
- Fetch the URL
- Convert HTML → markdown via `kgx-convert` (pandoc or fallback)
- Capture the markdown as a raw source
- If `--depth > 0`, crawl same-domain links (see Component 4)

New CLI flags on `Capture`:
```rust
Capture {
    #[arg(long)]
    from: String,
    #[arg(long = "type", default_value = "doc")]
    kind: String,
    #[arg(long)]
    ext: Option<String>,
    /// URL crawl depth: 0 = single page, 1 = page + direct links, etc.
    #[arg(long, default_value = "0")]
    depth: u32,
    /// Maximum pages to fetch during URL crawl (default: 50)
    #[arg(long, default_value = "50")]
    max_pages: u32,
}
```

## Component 3: MCP integration

### `ingest_file` — auto-detect document formats

`ingest_file.rs` modified: when `path` is a file with a document extension, run `kgx-convert::convert(path)` first, then ingest the resulting markdown. The raw source note records the original format in frontmatter:

```yaml
---
type: source
id: <ULID>
title: "Report Q3 2026"
source_format: pdf
created_via: mcp
hash: <sha256>
---
<converted markdown>
```

Directory ingestion: same auto-detection per file. The `ext` param accepts document extensions.

### `ingest_url` — add depth + max_pages params

Updated input schema:
```json
{
  "url": "string (required)",
  "depth": "integer (default 0)",
  "max_pages": "integer (default 50)",
  "same_domain": "boolean (default true)"
}
```

- `depth: 0` — fetch single page, convert HTML → markdown, capture (current behavior + HTML→md conversion).
- `depth > 0` — BFS crawl of same-domain links (see Component 4).

### New MCP tool: `recall_entity`

Exposes `kg recall` via MCP. Currently recall is CLI-only. Agents in any harness can query the POLE graph neighborhood.

```json
{
  "name": "recall_entity",
  "description": "Retrieve an entity's graph neighborhood with optional typed relations",
  "inputSchema": {
    "type": "object",
    "properties": {
      "entity": {"type": "string"},
      "relations": {"type": "boolean", "description": "Include typed relationship edges"}
    },
    "required": ["entity"]
  }
}
```

### `query_memory` — add `entity_type` field

```json
{
  "note_type": "string",
  "entity_type": "string (person|object|location|event)",
  "tag": "string",
  "status": "string",
  "limit": "integer"
}
```

## Component 4: URL crawl with depth

### Crawl algorithm (BFS)

```
1. Fetch seed URL → HTML
2. Convert HTML → markdown via kgx-convert (pandoc gfm, or html2text fallback)
3. Capture as raw/<date>-<slug>.md
4. If depth > 0:
   a. Parse HTML for <a href> links (scraper crate)
   b. Resolve relative URLs against seed URL
   c. Filter: same-domain, http(s) only, skip media files (.pdf/.png/.jpg/.gif/.css/.js)
   d. Deduplicate against visited set
   e. For each unvisited link (up to max_pages):
      - Fetch → convert → capture
      - If current depth > 1, enqueue this page's same-domain links for next level
   f. BFS queue with depth tracking; stop when max_pages hit or queue exhausted
5. Return: { seed_url, depth, pages_captured, pages_skipped, raw: [...] }
```

### Politeness

- 500ms delay between fetches (configurable: `KGX_CRAWL_DELAY_MS` env var, default 500).
- Same-domain filter prevents crawling off into unrelated sites.
- max_pages hard cap prevents runaway crawls.

### Dependencies

- `scraper` crate — lightweight HTML parser for link extraction (already in the Rust ecosystem, ~2MB compiled).
- `reqwest` — already a dependency of kgx-mcp.
- `url` crate — for domain comparison and relative URL resolution.

### Output

Each page becomes its own `raw/<date>-<slug>.md` source note. MCP response:
```json
{
  "status": "ok",
  "seed_url": "https://example.com/article",
  "depth": 1,
  "pages_captured": 12,
  "pages_skipped": 3,
  "raw": ["raw/2026-07-10-example-article.md", "raw/2026-07-10-example-page2.md"]
}
```

## Component 5: `kgx:pole` skill

### New skill files

- `.opencode/skills/kgx-pole/SKILL.md` (OpenCode)
- `.opencode/command/kgx-pole.md` (OpenCode slash command)
- `skills/claude/` equivalent
- `skills/codex/` equivalent
- `skills/cursor/` equivalent
- `skills/zcode/` equivalent

### Skill workflow

The skill instructs the agent through POLE graph extraction from any captured source:

1. **Identify the source** — `get_note` on the captured raw source to read the converted markdown.
2. **Extract POLE entities:**
   - **Persons** — named individuals, roles, organizations-as-actors → `upsert_note({type:"entity", entity_type:"person", title, links})`
   - **Objects** — systems, products, documents, tools → `upsert_note({type:"entity", entity_type:"object", ...})`
   - **Locations** — places, addresses, regions, facilities → `upsert_note({type:"entity", entity_type:"location", ...})`
   - **Events** — meetings, incidents, deployments, dated occurrences → `upsert_note({type:"entity", entity_type:"event", ...})`
3. **Extract typed relationships:**
   - `participates_in` — person participated in event
   - `located_at` — object/event at a location
   - `owns` — person/org owns an object
   - `decided` — person decided something
   - `caused` — event/object caused another event
   - `mentions_entity` — fact mentions an entity (general)
4. **Create fact notes** — atomic claims with `source` provenance, `links` to entities, and `relations` extra field:
   ```yaml
   relations:
     - target: "[[Alice Chen]]"
       rel: "participates_in"
     - target: "[[Q3 Planning Meeting]]"
       rel: "participates_in"
   ```
5. **Index** — `kg index --full` so POLE graph is queryable.

### Design decisions

- **Harness-driven** — the agent does the extraction reasoning. No external LLM provider needed in-session.
- **Complements `kgx:extract`** — `kgx:extract` is generic fact extraction; `kgx:pole` is structured POLE graph building. Agent can run either or both.
- **Source-format agnostic** — operates on markdown content, not original format.
- **No schema changes** — `entity_type` field already in `Frontmatter` and brain `notes` table. `relations` extra field already parsed by `kg index` into `edges` table with `rel_type`.

## Component 6: Query & indexing enhancements

### `kg query` — `--entity-type` filter

```bash
kg query --entity-type person          # all person entities
kg query --entity-type event --tag q3  # events tagged "q3"
```

MCP: `query_memory({note_type: "entity", entity_type: "person"})`.

Implementation: add `AND entity_type = ?` WHERE clause when filter present. The `entity_type` column already exists in `notes` table (schema.rs:7).

### `kg recall` — `--relations` flag

```bash
kg recall --entity "Alice Chen" --relations
```

```json
{
  "ok": true,
  "command": "recall",
  "data": {
    "entity": "Alice Chen",
    "neighbors": ["Q3 Planning Meeting", "Project Atlas", "HQ Building"],
    "relations": [
      {"target": "Q3 Planning Meeting", "rel": "participates_in"},
      {"target": "Project Atlas", "rel": "owns"},
      {"target": "HQ Building", "rel": "located_at"}
    ]
  }
}
```

Implementation: join `edges` table with `rel_type` to return typed relationship data alongside neighbor list. The `relations` data is stored in:
- **Brain `edges` table** — `edges(src_id, dst_id, rel_type)` — built during `kg index` from frontmatter `relations` extra field.
- **Frontmatter `relations` extra** — raw source of typed relations.

### `kg query` CLI command

Currently there is no `kg query` CLI command — `query_memory` is MCP-only. Add a `kg query` command to the CLI for parity:

```rust
/// Query notes with filters
Query {
    #[arg(long)]
    note_type: Option<String>,
    #[arg(long)]
    entity_type: Option<String>,
    #[arg(long)]
    tag: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long, default_value = "20")]
    limit: usize,
}
```

### No schema migration

- `entity_type` column: already in `notes` table.
- `rel_type` column: already in `edges` table.
- `relations` extra field: already parsed and written to edges during `kg index`.

## Testing

### Unit tests (per crate)

**kgx-convert:**
- `test_convert_markdown_passthrough` — .md file passes through unchanged.
- `test_convert_pdf_extracts_text` — .pdf file returns text with page separators.
- `test_convert_xlsx_produces_tables` — .xlsx file returns markdown tables per sheet.
- `test_convert_docx_via_pandoc` — .docx returns markdown (mock pandoc or integration test).
- `test_convert_unsupported_ext` — .xyz returns `KgError::Convert`.
- `test_convert_empty_pdf` — scanned PDF returns placeholder message.
- `test_is_document_ext` — extension classification.

**kgx-cli:**
- `test_capture_pdf_auto_converts` — `kg capture --from test.pdf` produces a raw markdown source.
- `test_capture_url_single_page` — `kg capture --from http://localhost:port` captures one page.
- `test_capture_url_depth_crawl` — depth=1 captures multiple same-domain pages.
- `test_capture_dir_with_documents` — directory walk converts .pdf/.docx files.
- `test_query_entity_type_filter` — `kg query --entity-type person` returns only person entities.
- `test_recall_relations` — `kg recall --entity X --relations` returns typed edges.

**kgx-mcp:**
- `test_ingest_file_pdf` — `ingest_file({path: "test.pdf"})` converts and ingests.
- `test_ingest_url_depth` — `ingest_url({url, depth: 1, max_pages: 5})` crawls subpages.
- `test_query_memory_entity_type` — `query_memory({entity_type: "person"})` filters correctly.
- `test_recall_entity_mcp` — `recall_entity({entity: "X", relations: true})` returns typed edges.

### Integration test

A new smoke test: `t19_doc_pole_pipeline`:
1. Create a test .docx file (or use a fixture) with persons, events, locations.
2. `kg capture --from fixture.docx` → raw source created with converted markdown.
3. Simulate POLE extraction: `upsert_note` for entities with `entity_type` + `relations`.
4. `kg index --full`.
5. `kg query --entity-type person` → returns the person entities.
6. `kg recall --entity "<person>" --relations` → returns typed edges.

### Test fixtures

- `tests/fixtures/sample.pdf` — small PDF with text.
- `tests/fixtures/sample.xlsx` — small Excel with 2 sheets.
- `tests/fixtures/sample.docx` — small Word doc with a table.
- A local HTTP server for URL crawl tests (use `axum` test utilities, already a dependency).

## Dependencies

### New crate dependencies

**kgx-convert/Cargo.toml:**
```toml
[dependencies]
kgx-core = { path = "../kgx-core" }
pdf-extract = "0.7"       # PDF text extraction (pure Rust)
calamine = "0.26"         # Excel .xlsx/.xls reader (pure Rust)
serde.workspace = true
serde_json.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

**kgx-mcp/Cargo.toml additions:**
```toml
kgx-convert = { path = "../kgx-convert" }
scraper = "0.20"          # HTML link parsing for URL crawl
url = "2"                 # URL resolution and domain comparison
```

**kgx-cli/Cargo.toml additions:**
```toml
kgx-convert = { path = "../kgx-convert" }
```

**Workspace Cargo.toml additions:**
```toml
pdf-extract = "0.7"
calamine = "0.26"
scraper = "0.20"
url = "2"
```

### External runtime dependency

- **pandoc** — bundled in release archive via `install.sh`. Resolved at runtime via `$KGX_PANDOC` → `~/.local/bin/pandoc-kgx` → system `pandoc`. Build-from-source users install separately.

## Error handling summary

| Error condition | Behavior |
|---|---|
| Unknown file extension | `KgError::Convert` with list of supported formats |
| Pandoc not found | `KgError::Convert` with install instructions |
| Pandoc subprocess failure | `KgError::Convert` with stderr |
| PDF extraction failure | `KgError::Convert` with detail |
| Excel with no readable sheets | `KgError::Convert("no sheets found")` |
| URL fetch failure (network) | `KgError::Other("fetch failed: <detail>")` — skip page, continue crawl |
| URL fetch failure (HTTP status) | Skip page, continue crawl, count in `pages_skipped` |
| Scanned PDF (no text) | Return placeholder markdown, don't error |
| max_pages reached | Stop crawl, return summary with pages captured so far |

## Files to create

| File | Responsibility |
|---|---|
| `crates/kgx-convert/Cargo.toml` | Crate manifest |
| `crates/kgx-convert/src/lib.rs` | Public API: `convert`, `convert_bytes`, `is_document_ext`, `ConvertOutput`, `SourceFormat` |
| `crates/kgx-convert/src/pandoc.rs` | Pandoc subprocess wrapper |
| `crates/kgx-convert/src/pdf.rs` | PDF extraction via `pdf-extract` crate |
| `crates/kgx-convert/src/xlsx.rs` | Excel extraction via `calamine` crate |
| `crates/kgx-convert/tests/convert_tests.rs` | Unit tests |
| `.opencode/skills/kgx-pole/SKILL.md` | OpenCode POLE skill |
| `.opencode/command/kgx-pole.md` | OpenCode POLE slash command |
| `skills/claude/kgx-pole.md` | Claude Code POLE command |
| `skills/codex/kgx-pole.md` | Codex POLE command |
| `skills/cursor/kgx-pole.md` | Cursor POLE command |
| `skills/zcode/kgx-pole.md` | ZCode POLE command |
| `tests/fixtures/sample.pdf` | PDF test fixture |
| `tests/fixtures/sample.xlsx` | Excel test fixture |
| `tests/fixtures/sample.docx` | Word test fixture |

## Files to modify

| File | Change |
|---|---|
| `Cargo.toml` (workspace) | Add `kgx-convert` to members, add new deps to workspace deps |
| `crates/kgx-cli/Cargo.toml` | Add `kgx-convert` dependency |
| `crates/kgx-cli/src/cli.rs` | Add `depth`, `max_pages` flags to `Capture`; add `Query` command with `entity_type` flag |
| `crates/kgx-cli/src/commands/capture.rs` | Auto-detect document extensions, convert before capture; enable URL branch with crawl |
| `crates/kgx-cli/src/commands/recall.rs` | Add `--relations` flag |
| `crates/kgx-cli/src/commands/query.rs` | New command: query notes with filters (new file) |
| `crates/kgx-cli/src/main.rs` | Dispatch new `Query` command |
| `crates/kgx-mcp/Cargo.toml` | Add `kgx-convert`, `scraper`, `url` deps |
| `crates/kgx-mcp/src/tools/mod.rs` | Register `recall_entity` tool; add `entity_type` to `query_memory` schema |
| `crates/kgx-mcp/src/tools/ingest_file.rs` | Auto-detect document formats, convert before ingest |
| `crates/kgx-mcp/src/tools/ingest_url.rs` | Add depth/max_pages crawl; HTML→md conversion |
| `crates/kgx-mcp/src/tools/query.rs` | Add `entity_type` filter |
| `crates/kgx-mcp/src/tools/recall.rs` | New tool: recall_entity (new file) |
| `crates/kgx-graph/src/query.rs` | Add `entity_type` filter to query function |
| `install.sh` | Download and install pandoc binary to `~/.local/bin/pandoc-kgx` |
| `README.md` | Document document ingestion, URL crawl, POLE skill, new query commands |
| `AGENTS.md` | Add `kgx:pole` to composite verbs table |
| `.opencode/skills/kgx/SKILL.md` | Add `kgx-pole` to command table |

## Non-goals

- Image/diagram extraction or OCR (text + tables only, per user decision).
- Cross-domain URL crawling (same-domain only, per user decision).
- Separate POLE graph store (use existing brain schema, per user decision).
- New CLI commands for documents (auto-detect in existing commands, per user decision).
- robots.txt parsing (same-domain + max-pages cap is sufficient for v1).
