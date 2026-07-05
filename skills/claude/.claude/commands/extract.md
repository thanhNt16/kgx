---
name: kgx:extract
description: Extract atomic facts, entities, and decisions from a captured source
disable-model-invocation: true
---

Extract atomic facts from a source. You (Claude) are the extractor — do not
shell out to `kg extract`, it calls an external LLM provider and needs its own
API key. You already have everything required to do this reasoning yourself.

1. Ask the user for the source note ID (or find candidates via `query_memory`
   with `note_type: source`, or `deep_search_memory`).
2. Fetch the source with `get_note` and read its body. Ask the user for an
   intensity if it matters: lite, full (default), or ultra.
3. Extract atomic facts from the body, following these rules:
   - One claim per note — never bundle multiple facts into one.
   - Lite: only explicit facts stated verbatim. No inference.
   - Full/Ultra: atomic facts with provenance; add entities only when
     explicitly named; avoid speculative facts.
   - Assign each fact a confidence: `high` (stated directly), `medium`
     (reasonably implied), or `low` (uncertain/inferred).
   - Note any named entities mentioned per fact.
4. For each fact, call `upsert_note` with:
   - `type: "fact"`, `title` (short claim), `body` (the fact, one sentence)
   - `source`: the source note's own wikilink, e.g. `[[raw/<source-stem>]]`
     (derive `<source-stem>` from the source note's file name, stripped of
     extension)
   - `confidence`: from step 3
   - `links`: `["[[Entity Name]]", ...]` for each named entity in the fact
5. If a fact describes a choice/tradeoff rather than a claim, use
   `type: "decision"` instead of `"fact"`. If it names a new entity worth
   tracking on its own, also `upsert_note` a `type: "entity"` note for it.
6. Report what was created: counts by type, titles, and any facts skipped
   (e.g. duplicates, non-atomic statements you split up).
