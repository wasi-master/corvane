//! Remote control for the GitHub Desktop parity harness (`tools/parity`).
//!
//! `CORVANE_CONTROL=<port>` (build with `--features snapshots`) listens on
//! `127.0.0.1:<port>` for newline-delimited JSON commands and answers each
//! with one JSON line. Input is injected straight into the window's event
//! dispatch (hover, press, drag, scroll, keys, text) and frames are rendered
//! offscreen, so neither focus nor screen-recording rights are needed and the
//! user's own windows are left alone. GHD is driven the same way over the
//! Chrome DevTools Protocol; the harness compares the two captures.
//!
//! Commands (`{"cmd": …}`; coordinates are window points, top-left origin):
//! - `ping` → `{w, h, scale}`
//! - `resize {w, h}`
//! - `move {x, y, pressed}`, `down {x, y}`, `up {x, y}`, `click {x, y}`
//!   (`button`: left|right|middle, `clicks`, `mods`: "cmd-shift" …)
//! - `drag {x, y, x2, y2, steps}`
//! - `scroll {x, y, dx, dy}` (pixel deltas, positive dy scrolls content down)
//! - `key {keys}` (GPUI keystroke syntax, space separated: "cmd-a escape")
//! - `type {text}`
//! - `action {name}` (a registered action, e.g. `corvane::OpenSettings`)
//! - `hook {name, arg}`: `complete-welcome`, `add-repo <path>`,
//!   `theme light|dark|high-contrast|system`, `popup <name>` (a
//!   `CORVANE_POPUP` name, opened now), `fake-accounts <json>` (signed-in
//!   accounts + their repository lists, `tools/parity/accounts.py`)
//! - `snap {path, cached}` → draws a fresh frame and saves it as PNG
//!   (`cached`: re-render only invalidated views, like a real frame)
//! - `menu` → the items of the last native menu (while the control socket is
//!   on, menus still pop up but are recorded and close themselves after
//!   `CORVANE_MENU_HOLD_MS`, default 1500); `menu-pick {label}` runs one
//! - `bench {steps, until, timeout_ms}` → `{input_ms, settle_ms, draw_ms,
//!   total_ms}`: runs `steps` (commands as above) back to back, waits until the
//!   `until` predicate holds on the app state ([`predicate`]), then draws one
//!   frame (only the views that were invalidated, as the display link would). `input_ms` is the synchronous handling of the steps, `settle_ms`
//!   the time from the first step to the predicate, `draw_ms` the frame
//!   (layout + paint on the main thread). Latency benchmarks (`tools/perf`).
//! - `frames {n}` → `{avg_ms, max_ms, cpu_ms, minstr}`: draws `n` frames of the
//!   current state, every view re-rendered; `scroll-frames {x, y, dy, n}`
//!   scrolls by `dy` before each frame and draws what the scroll invalidated
//! - `state` → the selected repository's selected file / commit, the first
//!   200 history SHAs and the changed-file count
//! - `quit`

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

use corvane_core::Dispatcher;
use gpui_kit::*;
use serde_json::{Value, json};
use tracing::{error, info};

type Reply = mpsc::Sender<Value>;
/// `open_dev_popup` from `main.rs` (the `CORVANE_POPUP` names).
pub type PopupHook = fn(&str, &mut App);

/// Start the listener thread and the foreground command loop.
pub fn start(port: u16, popup: PopupHook, cx: &mut App) {
    let (tx, rx) = mpsc::channel::<(Value, Reply)>();
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(listener) => listener,
        Err(err) => {
            error!(?err, port, "parity control: could not listen");
            return;
        }
    };
    info!(port, "parity control listening");
    // menus pop for real (screen captures compare them with GHD's) but are
    // recorded and close themselves, so this loop is only held for a moment
    corvane_ui::native_menu::set_auto_dismiss(Some(Duration::from_millis(
        std::env::var("CORVANE_MENU_HOLD_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1500),
    )));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let Ok(mut writer) = stream.try_clone() else {
                    return;
                };
                for line in BufReader::new(stream).lines() {
                    let Ok(line) = line else { break };
                    let reply = match serde_json::from_str::<Value>(&line) {
                        Ok(request) => {
                            let (reply_tx, reply_rx) = mpsc::channel();
                            if tx.send((request, reply_tx)).is_err() {
                                break;
                            }
                            reply_rx
                                .recv_timeout(Duration::from_secs(30))
                                .unwrap_or_else(|_| json!({"ok": false, "error": "timeout"}))
                        }
                        Err(err) => json!({"ok": false, "error": err.to_string()}),
                    };
                    if writeln!(writer, "{reply}").is_err() {
                        break;
                    }
                }
            });
        }
    });
    // A GPUI app only runs foreground work from its own executor: poll the
    // channel (a dev-only path, 4 ms is plenty).
    cx.spawn(async move |cx: &mut AsyncApp| {
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(4))
                .await;
            while let Ok((request, reply)) = rx.try_recv() {
                if request["cmd"] == "bench" {
                    bench(request, reply, popup, cx);
                    continue;
                }
                let quit = request["cmd"] == "quit";
                let result = cx.update(|cx| handle(&request, popup, cx));
                let _ = reply.send(match result {
                    Ok(mut value) => {
                        value["ok"] = json!(true);
                        value
                    }
                    Err(err) => json!({"ok": false, "error": err}),
                });
                if quit {
                    cx.update(|cx| cx.quit());
                    return;
                }
            }
        }
    })
    .detach();
}

fn handle(request: &Value, popup: PopupHook, cx: &mut App) -> Result<Value, String> {
    let cmd = request["cmd"].as_str().unwrap_or_default();
    match cmd {
        "quit" => return Ok(json!({})),
        "hook" => return hook(request, popup, cx),
        "state" => return Ok(state_summary(cx)),
        _ => {}
    }
    let handle = cx
        .windows()
        .first()
        .copied()
        .ok_or_else(|| "no window".to_string())?;
    handle
        .update(cx, |_, window, cx| window_command(cmd, request, window, cx))
        .map_err(|err| err.to_string())?
}

fn window_command(
    cmd: &str,
    request: &Value,
    window: &mut Window,
    cx: &mut App,
) -> Result<Value, String> {
    let position = point(px(num(request, "x")), px(num(request, "y")));
    let button = match request["button"].as_str() {
        Some("right") => MouseButton::Right,
        Some("middle") => MouseButton::Middle,
        _ => MouseButton::Left,
    };
    let clicks = request["clicks"].as_u64().unwrap_or(1) as usize;
    let modifiers = parse_modifiers(request["mods"].as_str().unwrap_or_default());
    let down = |position| {
        PlatformInput::MouseDown(MouseDownEvent {
            button,
            position,
            modifiers,
            click_count: clicks,
            first_mouse: false,
        })
    };
    let up = |position| {
        PlatformInput::MouseUp(MouseUpEvent {
            button,
            position,
            modifiers,
            click_count: clicks,
        })
    };
    let moved = |position, pressed_button| {
        PlatformInput::MouseMove(MouseMoveEvent {
            position,
            pressed_button,
            modifiers,
        })
    };
    match cmd {
        "ping" => {}
        "resize" => window.resize(size(px(num(request, "w")), px(num(request, "h")))),
        "move" => {
            // `pressed`: the button stays down (a drag, or moving off a
            // pressed button before releasing to cancel its click)
            let pressed = request["pressed"].as_bool().unwrap_or_default();
            window.dispatch_event(moved(position, pressed.then_some(button)), cx);
        }
        "down" => {
            window.dispatch_event(moved(position, None), cx);
            window.dispatch_event(down(position), cx);
        }
        "up" => {
            window.dispatch_event(up(position), cx);
        }
        "click" => {
            // a menu this click opens must not be confused with an older one
            if button == MouseButton::Right {
                corvane_ui::native_menu::clear_recorded();
            }
            window.dispatch_event(moved(position, None), cx);
            window.dispatch_event(down(position), cx);
            window.dispatch_event(up(position), cx);
        }
        "drag" => {
            let to = point(px(num(request, "x2")), px(num(request, "y2")));
            let steps = request["steps"].as_u64().unwrap_or(10).max(1);
            window.dispatch_event(moved(position, None), cx);
            window.dispatch_event(down(position), cx);
            for step in 1..=steps {
                let t = step as f32 / steps as f32;
                let at = point(
                    position.x + (to.x - position.x) * t,
                    position.y + (to.y - position.y) * t,
                );
                window.dispatch_event(moved(at, Some(button)), cx);
            }
            window.dispatch_event(up(to), cx);
        }
        "scroll" => {
            window.dispatch_event(moved(position, None), cx);
            window.dispatch_event(
                PlatformInput::ScrollWheel(ScrollWheelEvent {
                    position,
                    // GPUI: positive delta moves content towards the bottom
                    delta: ScrollDelta::Pixels(point(
                        px(-num(request, "dx")),
                        px(-num(request, "dy")),
                    )),
                    modifiers,
                    touch_phase: TouchPhase::Moved,
                }),
                cx,
            );
        }
        "key" => {
            for key in request["keys"]
                .as_str()
                .unwrap_or_default()
                .split_whitespace()
            {
                let keystroke = Keystroke::parse(key).map_err(|err| err.to_string())?;
                window.dispatch_keystroke(keystroke, cx);
            }
        }
        "type" => {
            for ch in request["text"].as_str().unwrap_or_default().chars() {
                let key = match ch {
                    ' ' => "space".to_string(),
                    '-' => "-".to_string(),
                    c => c.to_string(),
                };
                let mut keystroke = Keystroke::parse(&key).map_err(|err| err.to_string())?;
                keystroke.key_char = Some(ch.to_string());
                if ch.is_uppercase() {
                    keystroke.modifiers.shift = true;
                }
                window.dispatch_keystroke(keystroke, cx);
            }
        }
        "action" => {
            let name = request["name"].as_str().unwrap_or_default();
            let action = cx
                .build_action(name, None)
                .map_err(|err| format!("{name}: {err}"))?;
            window.dispatch_action(action, cx);
        }
        "snap" => {
            let path = request["path"]
                .as_str()
                .ok_or_else(|| "snap needs a path".to_string())?;
            // draw now: an unfocused window gets no display-link frames, and
            // render_to_image uses the last drawn scene. `cached`: only what
            // invalidation re-renders, as a display-link frame would (shows
            // a cached view that failed to re-render)
            if !request["cached"].as_bool().unwrap_or_default() {
                window.refresh();
            }
            window.draw(cx).clear(cx);
            let image = window.render_to_image().map_err(|err| err.to_string())?;
            image.save(path).map_err(|err| err.to_string())?;
            return Ok(json!({"w": image.width(), "h": image.height()}));
        }
        "frames" | "scroll-frames" => {
            let n = request["n"].as_u64().unwrap_or(20).max(1);
            let dy = num(request, "dy");
            let (mut total, mut max, mut cpu_total, mut instr_total) = (0.0f64, 0.0f64, 0.0, 0.0);
            for _ in 0..n {
                if cmd == "scroll-frames" {
                    window.dispatch_event(
                        PlatformInput::ScrollWheel(ScrollWheelEvent {
                            position,
                            delta: ScrollDelta::Pixels(point(px(0.), px(-dy))),
                            modifiers,
                            touch_phase: TouchPhase::Moved,
                        }),
                        cx,
                    );
                }
                let (started, cpu, instr) =
                    (std::time::Instant::now(), thread_cpu_ms(), thread_minstr());
                // `frames`: everything re-rendered (the worst case, e.g. after
                // a focus change); `scroll-frames`: what the scroll invalidated
                if cmd == "frames" {
                    window.refresh();
                }
                window.draw(cx).clear(cx);
                let took = ms(started.elapsed());
                cpu_total += thread_cpu_ms() - cpu;
                instr_total += thread_minstr() - instr;
                total += took;
                max = max.max(took);
            }
            return Ok(json!({
                "avg_ms": total / n as f64,
                "max_ms": max,
                "cpu_ms": cpu_total / n as f64,
                "minstr": instr_total / n as f64,
            }));
        }
        // the last native menu the app tried to show (recorded headless)
        "menu" => return Ok(json!({"items": corvane_ui::native_menu::recorded_menu()})),
        "menu-pick" => {
            let label = request["label"].as_str().unwrap_or_default();
            if !corvane_ui::native_menu::pick_recorded(label, window, cx) {
                return Err(format!("no enabled menu item {label:?}"));
            }
        }
        other => return Err(format!("unknown command {other:?}")),
    }
    let viewport = window.viewport_size();
    Ok(json!({
        "w": f32::from(viewport.width),
        "h": f32::from(viewport.height),
        "scale": window.scale_factor(),
    }))
}

/// `bench`: see the module docs. The predicate is polled every 250 µs on the
/// foreground executor, so `settle_ms` is exact to about a quarter millisecond.
fn bench(request: Value, reply: Reply, popup: PopupHook, cx: &mut AsyncApp) {
    let until = request["until"].as_str().unwrap_or("idle").to_string();
    let timeout = Duration::from_millis(request["timeout_ms"].as_u64().unwrap_or(20_000));
    let (started, cpu, instr) = (std::time::Instant::now(), thread_cpu_ms(), thread_minstr());
    let steps = request["steps"].as_array().cloned().unwrap_or_default();
    for step in &steps {
        if let Err(err) = cx.update(|cx| handle(step, popup, cx)) {
            let _ = reply.send(json!({"ok": false, "error": err}));
            return;
        }
    }
    let input_ms = ms(started.elapsed());
    let input_cpu_ms = thread_cpu_ms() - cpu;
    let input_minstr = thread_minstr() - instr;
    cx.spawn(async move |cx: &mut AsyncApp| {
        loop {
            let done = cx.update(|cx| predicate(&until, cx));
            match done {
                Err(err) => {
                    let _ = reply.send(json!({"ok": false, "error": err}));
                    return;
                }
                Ok(true) => break,
                Ok(false) if started.elapsed() > timeout => {
                    let _ = reply.send(
                        json!({"ok": false, "error": format!("timeout waiting for {until}")}),
                    );
                    return;
                }
                Ok(false) => {
                    cx.background_executor()
                        .timer(Duration::from_micros(250))
                        .await
                }
            }
        }
        let settle_ms = ms(started.elapsed());
        let (draw_ms, draw_cpu_ms, draw_minstr) =
            cx.update(draw_frame).unwrap_or((-1.0, -1.0, f64::NAN));
        let _ = reply.send(json!({
            "ok": true,
            "input_ms": input_ms,
            "input_cpu_ms": input_cpu_ms,
            "settle_ms": settle_ms,
            "draw_ms": draw_ms,
            "draw_cpu_ms": draw_cpu_ms,
            "draw_minstr": draw_minstr,
            "input_minstr": input_minstr,
            "total_ms": ms(started.elapsed()),
        }));
    })
    .detach();
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

/// Instructions this thread has retired, in millions (Apple's
/// `thread_selfcounts`, no privileges needed): the one frame cost that stays
/// the same on a loaded machine, whichever core the thread runs on.
#[cfg(target_os = "macos")]
fn thread_minstr() -> f64 {
    unsafe extern "C" {
        fn thread_selfcounts(kind: libc::c_int, buf: *mut u64, nbytes: libc::size_t)
        -> libc::c_int;
    }
    let mut counts = [0u64; 2];
    // SAFETY: kind 1 (instructions, cycles) fills two u64s
    let ok = unsafe { thread_selfcounts(1, counts.as_mut_ptr(), std::mem::size_of_val(&counts)) };
    if ok == 0 {
        counts[0] as f64 / 1e6
    } else {
        f64::NAN
    }
}

#[cfg(not(target_os = "macos"))]
fn thread_minstr() -> f64 {
    f64::NAN
}

/// CPU time this thread has used, in ms: frame costs measured this way do
/// not grow when other processes preempt the app (a loaded machine).
fn thread_cpu_ms() -> f64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: clock_gettime writes the timespec it is given
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    ts.tv_sec as f64 * 1000.0 + ts.tv_nsec as f64 / 1_000_000.0
}

/// Draw one fresh frame of the first window; its duration in ms (wall
/// clock, main-thread CPU) and its main-thread instructions (millions).
fn draw_frame(cx: &mut App) -> Result<(f64, f64, f64), String> {
    let handle = cx.windows().first().copied().ok_or("no window")?;
    handle
        .update(cx, |_, window, cx| {
            let (started, cpu, instr) =
                (std::time::Instant::now(), thread_cpu_ms(), thread_minstr());
            // what the next display-link frame would draw: views the input
            // or the state change invalidated, cached views reused
            window.draw(cx).clear(cx);
            (
                ms(started.elapsed()),
                thread_cpu_ms() - cpu,
                thread_minstr() - instr,
            )
        })
        .map_err(|err| err.to_string())
}

/// `until` predicates on the selected repository's state:
/// - `idle`: nothing of it is loading (status, diff, history, commit)
/// - `repo:<path suffix>`: that repository is selected, its status loaded
/// - `diff:<path>`: `path` is the selected changed file and its diff is loaded
/// - `commits:<n>`: at least `n` history commits are loaded
/// - `commit:<sha prefix>`: that commit is selected, its files and (when a
///   file is selected) its diff loaded
/// - `branch:<name>`: `name` is checked out and the refresh after it is done
/// - `files:<n>`: the status lists `n` changed files (shown; a later refresh
///   may still be running)
/// - `frame`: true at once (the bench then only times the steps + a frame)
fn predicate(until: &str, cx: &mut App) -> Result<bool, String> {
    let (kind, arg) = until.split_once(':').unwrap_or((until, ""));
    if kind == "frame" {
        return Ok(true);
    }
    let state = corvane_core::AppState::global(cx).read(cx);
    let Some(id) = state.selected else {
        return Ok(false);
    };
    if kind == "repo" {
        let Some(repo) = state.repository(id) else {
            return Ok(false);
        };
        if !repo.path.to_string_lossy().ends_with(arg) {
            return Ok(false);
        }
    }
    let Some(rs) = state.repo_states.get(&id) else {
        return Ok(false);
    };
    let settled = !rs.loading && rs.status.is_some() && rs.info.is_some();
    Ok(match kind {
        "idle" => {
            settled
                && !rs.diff_loading
                && !rs.commits_loading
                && !rs.committing
                && (rs.selected_file.is_none() || rs.diff.is_some())
        }
        "repo" => settled,
        "diff" => rs.selected_file.as_deref() == Some(arg) && !rs.diff_loading && rs.diff.is_some(),
        "commits" => rs.commits.len() >= arg.parse().unwrap_or(1) && !rs.commits_loading,
        "commit" => {
            rs.selected_commit
                .as_deref()
                .is_some_and(|sha| sha.starts_with(arg))
                && rs.changeset.is_some()
                && (rs.commit_selected_file.is_none() || rs.commit_diff.is_some())
        }
        "branch" => {
            settled
                && rs
                    .info
                    .as_ref()
                    .and_then(|info| info.current_branch())
                    .is_some_and(|b| b.name == arg)
        }
        // what the list shows, even while another refresh runs
        "files" => {
            rs.info.is_some()
                && rs
                    .status
                    .as_ref()
                    .is_some_and(|st| st.files.len() == arg.parse::<usize>().unwrap_or(0))
        }
        other => return Err(format!("unknown predicate {other:?}")),
    })
}

/// `state`: the selected repository's selections (for benchmarks).
fn state_summary(cx: &mut App) -> Value {
    let state = corvane_core::AppState::global(cx).read(cx);
    let Some(rs) = state.selected.and_then(|id| state.repo_states.get(&id)) else {
        return json!({});
    };
    json!({
        "selected_file": rs.selected_file,
        "selected_commit": rs.selected_commit,
        "commits": rs.commits.iter().take(200).map(|c| c.sha.clone()).collect::<Vec<_>>(),
        "files": rs.status.as_ref().map(|s| s.files.len()),
        // the "Committed … Undo" bar moves the commit form up
        "undo_bar": rs.last_commit.is_some(),
    })
}

fn hook(request: &Value, popup: PopupHook, cx: &mut App) -> Result<Value, String> {
    let arg = request["arg"].as_str().unwrap_or_default();
    match request["name"].as_str().unwrap_or_default() {
        "complete-welcome" => Dispatcher::complete_welcome(cx),
        "add-repo" => Dispatcher::add_repository(std::path::PathBuf::from(arg), cx),
        "theme" => {
            let theme = match arg {
                "light" => corvane_core::ThemeSetting::Light,
                "high-contrast" => corvane_core::ThemeSetting::HighContrast,
                "system" => corvane_core::ThemeSetting::System,
                _ => corvane_core::ThemeSetting::Dark,
            };
            Dispatcher::update_settings(cx, |s| s.theme = theme);
        }
        "popup" => popup(arg, cx),
        // GHD's `focus` IPC: refresh the selected repository
        "refresh" => {
            if let Some(id) = corvane_core::AppState::global(cx).read(cx).selected {
                Dispatcher::refresh_repository(id, cx);
            }
        }
        "fake-accounts" => fake_accounts(arg, cx)?,
        other => return Err(format!("unknown hook {other:?}")),
    }
    Ok(json!({}))
}

/// `fake-accounts {accounts, repositories}` (`tools/parity/accounts.py`):
/// signed-in accounts with their repository lists, without tokens or API
/// calls (the lists are already there, so nothing is fetched).
fn fake_accounts(arg: &str, cx: &mut App) -> Result<(), String> {
    let fake: Value = serde_json::from_str(arg).map_err(|e| e.to_string())?;
    let accounts: Vec<corvane_core::Account> =
        serde_json::from_value(fake["accounts"].clone()).map_err(|e| e.to_string())?;
    let repositories: std::collections::HashMap<String, Vec<corvane_core::GitHubRepository>> =
        serde_json::from_value(fake["repositories"].clone()).map_err(|e| e.to_string())?;
    corvane_core::AppState::global(cx).update(cx, |s, cx| {
        s.accounts = accounts;
        s.api_repositories = repositories;
        s.api_repositories_loading.clear();
        cx.notify();
    });
    Ok(())
}

fn num(request: &Value, key: &str) -> f32 {
    request[key].as_f64().unwrap_or_default() as f32
}

fn parse_modifiers(spec: &str) -> Modifiers {
    let mut modifiers = Modifiers::default();
    for part in spec.split(['-', '+']) {
        match part {
            "cmd" | "meta" => modifiers.platform = true,
            "shift" => modifiers.shift = true,
            "alt" | "option" => modifiers.alt = true,
            "ctrl" | "control" => modifiers.control = true,
            _ => {}
        }
    }
    modifiers
}
