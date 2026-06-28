# KGX MVP Status

Phase 6 polish status as of 2026-06-27.

| PRD checklist item | Test or file |
| --- | --- |
| Workspace and module layout | `Cargo.toml`, crate manifests |
| `kg init` and `CLAUDE.md` | `crates/kgx-cli/tests/cli_init.rs`, `tests/smoke/tests/smoke_t11_okf.rs` |
| OKF parser and validator | `crates/kgx-okf/tests/validate_integration.rs` |
| Brain schema and deterministic index | `crates/kgx-graph/tests/build.rs`, `tests/smoke/tests/smoke_t10_rebuild.rs` |
| Hybrid retrieval and RRF | `crates/kgx-retrieval/tests/hybrid.rs` |
| `ask`, `recall`, `search --json` | `crates/kgx-cli/tests/cli_ask.rs`, `cli_search.rs` |
| Dream and review flows | `crates/kgx-dream/tests/run.rs`, `crates/kgx-cli/tests/cli_review.rs` |
| RTK wrapper and installer | `crates/kgx-rtk/tests/wrap.rs`, `install.sh`, `tests/smoke/tests/smoke_t_install.rs` |
| Token accounting, `tokens`, dashboard | `crates/kgx-tokens/tests/persist.rs`, `crates/kgx-cli/tests/cli_status.rs`, `cli_dashboard.rs` |
| Cron helpers | `crates/kgx-cli/tests/cli_cron.rs` |
| Graph export HTML/Mermaid/DOT/Canvas | `crates/kgx-viz/tests/html.rs`, `crates/kgx-cli/tests/cli_graph.rs`, `tests/smoke/tests/smoke_t12_graph.rs` |
| Docs use cases | `crates/kgx-docs/tests/usecase.rs`, `crates/kgx-cli/tests/cli_docs.rs` |
| OKF ship/pull round-trip | `crates/kgx-okf/tests/bundle.rs`, `tests/smoke/tests/smoke_t11_okf.rs` |
| Cross-tool CI and bench-ish coverage | `.github/workflows/ci.yml` |
| MiniLM embedder | `crates/kgx-graph/src/embed.rs` placeholder behind `feature = "candle"` |

MiniLM limitation: the `candle` feature is scaffolded with a placeholder type so `--all-features` remains network-free and avoids heavyweight model dependency churn. A production MiniLM loader still needs vendored or explicitly downloaded model assets.
