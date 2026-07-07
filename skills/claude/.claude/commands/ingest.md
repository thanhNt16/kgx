---
name: kgx:ingest
description: Capture a source (file/folder/URL/conversation) and extract atomic facts
disable-model-invocation: true
---

Capture a source and extract atomic facts from it. **You (the agent) do the
extraction** — never shell out to `kg extract`, it requires an external LLM
provider that is not available in-session. The retrieval/extraction reasoning
is yours.

1. Ask the user what to ingest:
   - A **file** path → `ingest_file({content})` (read it first) or `ingest_file({path})`.
   - A **folder** path → `ingest_file({path: "<dir>"})` (walked recursively) or
     `kg capture --from <dir>` via Bash. Both capture every text file
     (`.md,.txt,.markdown,.mdx` by default; `--ext`/`ext` overrides) as its own
     immutable raw source.
   - A **URL** → `ingest_url({url})`.
   - **Conversation** text → `ingest_conversation({turns:[{role,content}], action:"finalize"})`.
2. From the returned raw source(s), read each one with `get_note` (or read the
   file directly) and **extract atomic facts yourself**, following the rules in
   `kgx:extract`:
   - One claim per note — never bundle multiple facts.
   - Lite: only explicit facts stated verbatim. Full/Ultra: atomic facts with
     provenance; add entities only when explicitly named.
   - Assign each a confidence: `high` (stated directly), `medium` (reasonably
     implied), `low` (uncertain/inferred).
3. For each fact, call `upsert_note`:
   - `type: "fact"`, `title` (short claim), `body` (one sentence)
   - `source`: the source note's wikilink, e.g. `[[raw/<source-stem>]]`
     (derive `<source-stem>` from the source note's file name, no extension)
   - `confidence`: from step 2
   - `links`: `["[[Entity Name]]", ...]` for each named entity
   - Use `type: "decision"` for choices/tradeoffs; `type: "entity"` for new
     named entities worth tracking on their own.
4. Rebuild the index so new notes are searchable: `kg index --full` via Bash.
5. Summarize: counts by type, titles, anything skipped (duplicates, non-atomic
   statements split up).

For many sources, capture them all first (one batch), then extract in a second
pass — you don't need an LLM round-trip per file, the reasoning is yours.
