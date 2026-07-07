---
description: Answer a question over the knowledge graph with citations
---
Answer a question using the vault's retrieval tools and your own reasoning.
**You (the agent) synthesize the answer** — do not shell out to `kg ask`; it
calls an external LLM provider that is not configured in-session.

1. Ask the user for the question (and scope: local = direct hits, global =
   widen through community/MOC summaries first).
2. Retrieve context:
   - Local: `nl_query_memory({query, scope:"local", mode:"hybrid"})` or
     `deep_search_memory({query})` for progressive disclosure.
   - Global: `nl_query_memory({query, scope:"global"})` — returns retrieved
     community context + hits, no synthesized answer.
   - Drop to `query_memory({note_type, tag, status})` for structured filters.
3. If the top hits are thin, fetch neighbors with `recall --entity "<name>"`
   (Bash) or pull related notes via `get_note` on cited ids.
4. **Synthesize the answer yourself** over the retrieved context. Cite note
   ids inline, e.g. "Postgres is the primary datastore [01FACT01...]."
5. Report the answer with citations. If confidence is low or context is
   missing, say so and suggest capturing the missing source.
