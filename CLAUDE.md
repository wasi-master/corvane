# Corvane

Native Rust clone of GitHub Desktop (macOS first). UI is a 1:1 port of GitHub Desktop 3.6.6; engine is GPUI (via `gpui-kit`) + gitoxide reads + git CLI writes.

## Read first

- `PLAN.md` — architecture, decisions, milestones, budgets.
- `TODO.md` — GHD features not built yet, with source pointers.
- `docs/reference/deviations.md` — built features that differ from GHD and why.
- `docs/reference/ghd-ui-inventory.md` — GHD layout constants, menus, dialogs, foldouts, refresh model.
- `docs/reference/ghd-theme-tokens.md` — Primer palette + GHD light/dark tokens.

## Conventions

- Rust 2024 edition, stable toolchain per `rust-toolchain.toml`. `cargo fmt`, `cargo clippy --all-targets -- -D warnings` must pass.
- Library crates use `thiserror`; the binary uses `anyhow`. No `unwrap()`/`expect()` outside tests and `main.rs` startup.
- Every git subprocess goes through `corvane_git::process::run` (env, tracing span, cancellation). Never spawn `git` elsewhere.
- UI crates never touch git, network or disk; they dispatch actions on `corvane_core::Dispatcher`.
- Match GHD behaviour first, then improve. When deviating, note it in the module doc comment with the GHD file path and in `docs/reference/deviations.md`. When a `TODO.md` item is built, delete it there; leftovers that are mere differences go to `deviations.md`.
- Pin `gpui-kit` and `gpui-pre` to exact versions; upgrade only on a dedicated branch.
- Keep GHD's dialog/foldout/menu names as Rust type names (`PopupType::ConfirmDiscardChanges` → `dialogs::confirm_discard_changes`).
- No telemetry, no usage stats.

## Verification

`cargo test --workspace`, then the manual QA checklist in `PLAN.md` §6 for touched views. Compare screenshots against GHD at 1367×814 dark + light.
