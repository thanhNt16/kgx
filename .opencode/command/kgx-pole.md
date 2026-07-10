---
description: Extract a POLE (Person/Object/Location/Event) graph from a captured source
---
Extract a POLE graph from a captured source. **You (the agent) do the
extraction** — this is a reasoning task, not an external LLM call.

1. Identify the source: `get_note` on the captured raw source, or
   `query_memory({note_type: "source"})` to list available sources.
2. Read the content and extract POLE entities:
   - **Persons** → `upsert_note({type:"entity", entity_type:"person", title, links:[]})`
   - **Objects** → `upsert_note({type:"entity", entity_type:"object", ...})`
   - **Locations** → `upsert_note({type:"entity", entity_type:"location", ...})`
   - **Events** → `upsert_note({type:"entity", entity_type:"event", ...})`
3. Create fact notes with typed relationships via `links`:
   - `participates_in`, `located_at`, `owns`, `decided`, `caused`, `mentions_entity`
   - Example: `upsert_note({type:"fact", title, body, source:"[[raw/...]]", links:["[[Alice]]","[[Meeting]]"]})`
4. Index: `kg index --full` via Bash.
5. Verify: `kg query --entity-type person` or `recall_entity({entity:"X", relations:true})`.

Only create entities for explicitly named things. Cite the source. One entity
per `upsert_note` call.
