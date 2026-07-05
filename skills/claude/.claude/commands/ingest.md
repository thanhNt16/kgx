---
name: kgx:ingest
description: Capture a source (file/URL/conversation) and extract atomic facts
disable-model-invocation: true
---

Capture a source and extract atomic facts.

1. Ask the user what to ingest: a file path, URL, or conversation text
2. Use the appropriate MCP tool:
   - `ingest_file` for a file (pass content as text)
   - `ingest_url` for a URL
   - `ingest_conversation` for a conversation (pass each turn as {role, content})
3. After ingest, run `kg extract --source <returned_id> --intensity full` via Bash
4. Summarize what was captured and extracted
