---
name: kgx:extract
description: Extract atomic facts, entities, and decisions from a captured source
disable-model-invocation: true
---

Extract atomic facts from a source.

1. Ask the user for the source note ID (or find it via `query_memory`)
2. Run `kg extract --source <source_id> --intensity full` via Bash
3. Show the extracted facts, entities, and decisions
