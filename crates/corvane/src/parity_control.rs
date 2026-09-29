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
//!   `CORVANE_POPUP` name, opened now)
//! - `snap {path}` → draws a fresh frame and saves it as PNG
//! - `menu` → the items of the last native menu (while the control socket is
//!   on, menus still pop up but are recorded and close themselves after
//!   `CORVANE_MENU_HOLD_MS`, default 1500); `menu-pick {label}` runs one
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
    #[cfg(target_os = "macos")]
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
            // render_to_image uses the last drawn scene
            window.refresh();
            window.draw(cx).clear(cx);
            let image = window.render_to_image().map_err(|err| err.to_string())?;
            image.save(path).map_err(|err| err.to_string())?;
            return Ok(json!({"w": image.width(), "h": image.height()}));
        }
        // the last native menu the app tried to show (recorded headless)
        #[cfg(target_os = "macos")]
        "menu" => return Ok(json!({"items": corvane_ui::native_menu::recorded_menu()})),
        #[cfg(target_os = "macos")]
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
        other => return Err(format!("unknown hook {other:?}")),
    }
    Ok(json!({}))
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
