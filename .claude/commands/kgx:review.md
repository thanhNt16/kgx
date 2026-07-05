---
name: kgx:review
description: Review staged dream diffs without running consolidation
disable-model-invocation: true
---

Review staged dream diffs.

1. Run `kg review` via Bash to show staged diffs
2. Ask the user whether to approve or reject
3. If approve: run `kg review --approve all --ponytail-audit` via Bash
4. If reject: inform the user and suggest `kg dream` to regenerate
