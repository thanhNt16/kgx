#!/usr/bin/env bash
set -euo pipefail

echo "=== gate: format ==="
(cd "$(dirname "$0")" && cargo fmt --all --check)

echo "=== gate: clippy ==="
(cd "$(dirname "$0")" && cargo clippy --all-targets --all-features -- -D warnings)

echo "=== gate: unit tests ==="
(cd "$(dirname "$0")" && cargo test --workspace --lib)

echo "=== gate: PASSED ==="
