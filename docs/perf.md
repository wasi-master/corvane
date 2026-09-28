# Performance log

Budgets from `PLAN.md` §5. Measured on a MacBook (Apple Silicon, macOS 26.4), debug builds unless noted.

| Date | Build | Cold start → window (ms) | Idle CPU | RSS idle | Notes |
|---|---|---|---|---|---|
| 2026-09-28 | M0 debug, runtime shaders | see below | 0.0–1.0 % (top, 3 s samples) | 72 MB (top MEM) / 90 MB (ps rss) | Empty repository chrome only; no git engine yet |

Cold-start numbers are the `main window opened elapsed_ms` log line from three launches.

## How to measure

```bash
packaging/bundle.sh && target/bundle/Corvane.app/Contents/MacOS/corvane 2>&1 | grep elapsed_ms
top -l 3 -s 3 -pid $(pgrep -x corvane) -stats pid,cpu,mem
```
