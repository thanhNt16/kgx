---
description: Scaffold a new KGX vault with templates and skills
---
Scaffold a new KGX vault.

1. Ask the user for:
   - Vault path (default: current directory or choose)
   - Template: research, code, pkm, or team
   - Whether to include skills (`--with-skills`)
   - Whether to include RTK (`--with-rtk`)
2. Run `kg init [--template <type>] [--with-skills] [--with-rtk] [--vault <path>]` via Bash.
   This creates a `.brain/` knowledge vault inside the target; agent/tooling config is
   written to the project root. Pass `--migrate` to relocate a legacy root-level vault
   into `.brain/`.
3. Show the created directory structure (knowledge content under `.brain/`, agent config
   at the project root)
