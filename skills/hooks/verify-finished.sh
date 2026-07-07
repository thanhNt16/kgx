#!/usr/bin/env bash
# verify-finished: shared completion gate for Claude Code, Codex, and OpenCode.
set -u

mode="${1:---json}"

root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$root" || exit 0

log_dir=".kgx/hooks"
log_file="$log_dir/last-finish-check.log"
mkdir -p "$log_dir"

if ! git status --short --untracked-files=all | grep -q .; then
  exit 0
fi

checks=()
checks+=("git diff --check")
if [ -f Cargo.toml ]; then
  checks+=("cargo fmt --all --check")
  checks+=("cargo test --workspace")
fi
if [ -f package.json ]; then
  if command -v npm >/dev/null 2>&1; then
    # Only run `npm test` when package.json actually defines scripts.test.
    # `npm pkg get` prints {} when absent and a quoted string when present.
    test_script="$(npm pkg get scripts.test 2>/dev/null)"
    if [ -n "$test_script" ] && [ "$test_script" != "{}" ] && [ "$test_script" != "null" ] && [ "$test_script" != '""' ]; then
      checks+=("npm test")
    fi
  fi
fi

: > "$log_file"
failed=0
for check in "${checks[@]}"; do
  {
    printf '\n$ %s\n' "$check"
    sh -c "$check"
  } >> "$log_file" 2>&1
  status=$?
  if [ "$status" -ne 0 ]; then
    failed=$status
    break
  fi
done

if [ "$failed" -eq 0 ]; then
  exit 0
fi

reason="Finish verification failed. Inspect $log_file, fix the failure, then run the finish checks again before responding."
if [ "$mode" = "--plain" ]; then
  printf '%s\n' "$reason" >&2
  exit 1
fi

printf '{"decision":"block","reason":"%s"}\n' "$reason"
exit 0
