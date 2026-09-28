# Performance log

Budgets from `PLAN.md` §5. Measured on a MacBook (Apple Silicon, macOS 26.4), debug builds unless noted.

| Date | Build | Cold start → window (ms) | Idle CPU | RSS idle | Notes |
|---|---|---|---|---|---|
| 2026-09-28 | M0 debug, runtime shaders | 4.8–6.6 s cold cache; 1.26 s warm (store 0.15, platform+fonts 0.15, gpui-kit init 0.7, window 0.2) | 0.0–1.0 % (top, 3 s samples) | 72 MB (top MEM) / 90 MB (ps rss) | Empty repository chrome only; no git engine yet |

| 2026-09-28 | M1 release (fat LTO, runtime shaders) | 1.9–2.8 s (store 0.02, platform 0.22, **gpui-kit init 1.4–1.6**, window 0.15) | 0–3 % | 80–87 MB (top MEM) | Binary 13.7 MB. `gpui_kit::init` dominates; investigate theme registry / highlighter init |

| 2026-09-28 | M2 debug, kit theme pre-seeded | 0.7–1.7 s under heavy load (gpui-kit init **24–47 ms**, was 1.5 s) | — | — | Root cause: gpui-component enumerated every installed font (CoreText) unless its theme fonts were set explicitly; fixed by `theme::preseed_kit_theme` |

Cold-start numbers are the `main window opened elapsed_ms` log line from three launches.

## How to measure

```bash
packaging/bundle.sh && target/bundle/Corvane.app/Contents/MacOS/corvane 2>&1 | grep elapsed_ms
top -l 3 -s 3 -pid $(pgrep -x corvane) -stats pid,cpu,mem
```
