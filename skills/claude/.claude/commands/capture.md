---
name: kgx:capture
description: Capture raw source material verbatim (immutable). No extraction.
disable-model-invocation: true
---

Capture raw source material verbatim into the vault. This is **capture only**
— it does not extract facts. Run `kgx:ingest` (capture + extract) when you
want atomic facts out of the source.

1. Ask the user what to capture:
   - A **file** path → `ingest_file({path})` or `ingest_file({content})`.
   - A **folder** path → `ingest_file({path: "<dir>"})` (walked recursively,
     every text file captured as its own source) or `kg capture --from <dir>`
     via Bash (`--ext` overrides the default `.md,.txt,.markdown,.mdx`).
   - A **URL** → `ingest_url({url})`.
   - **Conversation** → `ingest_conversation({turns:[{role,content}]})`.
   - Pasted content → `ingest_file({content})`.
2. Show the captured source id(s)/path(s) to the user.
3. Point the user to `kgx:ingest` / `kgx:extract` to extract atomic facts.
