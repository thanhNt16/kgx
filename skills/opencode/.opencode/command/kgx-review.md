---
description: Review staged dream diffs without running consolidation
---
Review and apply staged consolidation diffs (written to
`.brain/.kg/staged_diffs.json` by the `kgx:dream` skill).

1. Run `kg review` via Bash to show the staged diffs.
2. Ask the user whether to approve or reject.
3. If approve: run `kg review --approve all --ponytail-audit` via Bash.
4. If reject: clear or rewrite `.brain/.kg/staged_diffs.json` and re-run the
   `kgx:dream` skill to regenerate diffs.
