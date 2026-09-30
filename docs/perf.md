# Performance log

Budgets from `PLAN.md` §5. Measured on a MacBook (Apple Silicon, macOS 26.4), debug builds unless noted.

| Date | Build | Cold start → window (ms) | Idle CPU | RSS idle | Notes |
|---|---|---|---|---|---|
| 2026-09-28 | M0 debug, runtime shaders | 4.8–6.6 s cold cache; 1.26 s warm (store 0.15, platform+fonts 0.15, gpui-kit init 0.7, window 0.2) | 0.0–1.0 % (top, 3 s samples) | 72 MB (top MEM) / 90 MB (ps rss) | Empty repository chrome only; no git engine yet |
| 2026-09-28 | M1 release (fat LTO, runtime shaders) | 1.9–2.8 s (store 0.02, platform 0.22, **gpui-kit init 1.4–1.6**, window 0.15) | 0–3 % | 80–87 MB (top MEM) | Binary 13.7 MB. `gpui_kit::init` dominates; investigate theme registry / highlighter init |
| 2026-09-28 | M2 debug, kit theme pre-seeded | 0.7–1.7 s under heavy load (gpui-kit init **24–47 ms**, was 1.5 s) | — | — | Root cause: gpui-component enumerated every installed font (CoreText) unless its theme fonts were set explicitly; fixed by `theme::preseed_kit_theme` |
| 2026-09-29 | M7 release (fat LTO, runtime shaders, `cargo build --release`) | **1012 ms first launch after the build, 257 / 227 ms warm** | **0.4–1.1 % (top, 5 s samples, 8 samples; same with a one-file repository)** | **75–84 MB (ps rss), 88–91 MB (top MEM)** with one repository (this checkout, 10 changed files) | Binary **22.7 MB** (`ls`; 24.0 MB before `strip` accounting). Release build 10 min with cached dependencies |

Cold-start numbers are the `main window opened elapsed_ms` log line from three launches.

## M7 pass against the budgets (2026-09-29)

| Metric | Target | Measured | Status |
|---|---|---|---|
| Cold start to first frame | < 300 ms | 227–257 ms warm (1.0 s on the first launch of a fresh binary) | met |
| Idle RSS, 1 repo | ≤ 80 MB | 75–79 MB (`ps rss`), 84 MB with the tiny test repository | met, no headroom |
| Idle RSS, 10 repos | ≤ 120 MB | not measured (no second machine state with ten repositories added) | open |
| Idle CPU | 0.0 % | 0.4–1.1 % per 5 s sample | **missed**: something still ticks while nothing changes; not the file watcher (a one-file repository idles the same). Candidates: GPUI's display link kept alive by a hover/animation timer, the `relative_time` refresh, the kit's cursor blink. Profile with `sample` while idle |
| `git status` equivalent, 50k files | < 300 ms warm | not measured (no criterion bench yet, `corvane-git` tests use small repositories) | open |
| Diff open, 5k-line file | < 50 ms | not measured | open |
| Binary size (default build) | ≤ 25 MB | 22.7 MB | met |

## Tree-sitter highlighting (2026-09-30)

Opt-in engine (flag `105-tree-sitter-highlighting`, `crates/corvane-highlight/src/treesitter`). Apple Silicon, grammar packs built by `packaging/packs.sh` (release, macOS 15 target).

| Metric | Measured | Notes |
|---|---|---|
| Default build size | +~0.2 MB | the tree-sitter C runtime (`libtree-sitter.a`: 169 KB `__text`) plus the loader and painter; the grammars are not linked. The M7 release was 22.7 MB against the 25 MB budget; not re-measured with a full release build (disk) |
| `tree-sitter-all` pack (102 grammars) | 13.5 MB zipped, 170 MB installed | per architecture; the parse tables dominate (systemverilog 23.5 MB, verilog 18.2 MB, fsharp 14.2 MB, ocaml 12.9 MB) |
| `tree-sitter-rest` pack (53 grammars) | 7.4 MB zipped, 101 MB installed | |
| `full` build | +~170 MB per architecture | every grammar linked in |
| Loading a pack | 2.3 s the first time a new pack file is opened, 2 ms afterwards | `dlopen` of the freshly written dylib (macOS evaluates the new file once); on a background thread, diffs keep GHD's highlighting until it finishes |
| Highlighting a 246 KB Rust file | 148 ms | debug build of the Rust side, optimised C runtime and grammars; budget is `MAX_HIGHLIGHT_BYTES` (256 KB) like the other engines |

## How to measure

```bash
cargo build --release -p corvane && packaging/bundle.sh release
# three launches; the `main window opened elapsed_ms` line is in ~/Library/Logs/Corvane/corvane.log.<date>
open -n target/bundle/Corvane.app --env CORVANE_ADD_REPO=<repo>
# after 60 s idle
top -l 8 -s 5 -pid $(pgrep -x corvane) -stats pid,cpu,mem
ps -o rss= -p $(pgrep -x corvane)
```
