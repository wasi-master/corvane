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
- A deviation that can be switched off gets a flag (`docs/reference/flags.md`): add it to `crates/corvane-core/src/flags/registry.rs` with the next id in its category block (ids are never reused), a value for every preset and "on" meaning the deviation is active; gate at the highest layer that has a `cx` (lower crates take parameters); run `UPDATE_FLAGS_DOC=1 cargo test -p corvane-core flags_doc`; end its `deviations.md` bullet with "Flag: `NNN-slug`". The parity harness runs Corvane with `CORVANE_FLAGS=preset=github-desktop`.
- Pin `gpui-kit` and `gpui-pre` to exact versions; upgrade only on a dedicated branch.
- Keep GHD's dialog/foldout/menu names as Rust type names (`PopupType::ConfirmDiscardChanges` → `dialogs::confirm_discard_changes`).
- No telemetry, no usage stats.

## Verification

`cargo test --workspace`, then the manual QA checklist in `PLAN.md` §6 for touched views. Compare against GHD at 1367×814 dark + light with the parity harness: `cargo build -p corvane --features snapshots && python3 tools/parity/parity.py [scenario…]` (drives a private GHD over CDP and Corvane over `CORVANE_CONTROL` with identical input, diffs every snap, report in `target/parity/latest/index.html`; see `tools/parity/README.md`). A touched view's scenarios must not get worse; add a scenario for any new surface or state.
