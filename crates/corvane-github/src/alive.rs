//! Alive: GitHub's push channel for Desktop notifications
//! (`@github/alive-client` `AliveSession`, GHD `lib/stores/alive-store.ts`).
//!
//! The session is a WebSocket to the URL `GET /alive_internal/websocket-url`
//! hands out. After connecting, the client sends
//! `{"subscribe": {"<signed channel>": "<offset>"}}` for the channel
//! `GET /desktop_internal/alive-channel` returned; the server answers
//! `{"e":"ack","off":"…"}` and pushes `{"e":"msg","ch":"<channel>","off":"…",
//! "data":{…}}` envelopes whose `data` is one of the Desktop events
//! ([`AliveEvent`]). The offset is remembered so a reconnect resumes where
//! the stream left off, as `AliveSession` does.
//!
//! This is a blocking client on its own thread (tungstenite + rustls);
//! presence channels and the shared-worker mode of the JavaScript client
//! are not needed and not implemented.

use std::io::ErrorKind;
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

/// `IAPIAliveSignedChannel`: the topic Desktop subscribes to.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct AliveChannel {
    pub channel_name: String,
    pub signed_channel: String,
}

/// `IAPIAliveWebSocket`
#[derive(Clone, Debug, Deserialize)]
pub struct AliveWebSocket {
    pub url: String,
}

/// `DesktopAliveEvent`: what GitHub pushes to Desktop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AliveEvent {
    /// `IDesktopChecksFailedAliveEvent`
    #[serde(rename = "pr-checks-failed")]
    ChecksFailed {
        #[serde(default)]
        timestamp: u64,
        owner: String,
        repo: String,
        pull_request_number: u64,
        check_suite_id: u64,
        commit_sha: String,
    },
    /// `IDesktopPullRequestReviewSubmitAliveEvent`
    #[serde(rename = "pr-review-submit")]
    ReviewSubmit {
        #[serde(default)]
        timestamp: u64,
        owner: String,
        repo: String,
        pull_request_number: u64,
        state: String,
        review_id: String,
    },
    /// `IDesktopPullRequestCommentAliveEvent`
    #[serde(rename = "pr-comment")]
    Comment {
        subtype: CommentSubtype,
        #[serde(default)]
        timestamp: u64,
        owner: String,
        repo: String,
        pull_request_number: u64,
        comment_id: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommentSubtype {
    #[serde(rename = "review-comment")]
    ReviewComment,
    #[serde(rename = "issue-comment")]
    IssueComment,
}

impl AliveEvent {
    /// `owner` / `repo` of the repository the event is about.
    pub fn repository(&self) -> (&str, &str) {
        match self {
            AliveEvent::ChecksFailed { owner, repo, .. }
            | AliveEvent::ReviewSubmit { owner, repo, .. }
            | AliveEvent::Comment { owner, repo, .. } => (owner, repo),
        }
    }

    pub fn pull_request_number(&self) -> u64 {
        match self {
            AliveEvent::ChecksFailed {
                pull_request_number,
                ..
            }
            | AliveEvent::ReviewSubmit {
                pull_request_number,
                ..
            }
            | AliveEvent::Comment {
                pull_request_number,
                ..
            } => *pull_request_number,
        }
    }
}

/// One server frame.
#[derive(Debug, Deserialize)]
struct Envelope {
    e: String,
    #[serde(default)]
    ch: Option<String>,
    #[serde(default)]
    off: Option<String>,
    #[serde(default)]
    data: Option<serde_json::Value>,
}

/// What a frame meant for the subscriber.
#[derive(Debug, PartialEq, Eq)]
pub enum Incoming {
    /// The server acknowledged the subscription (offset).
    Ack(String),
    /// An event on our channel.
    Event(AliveEvent),
    /// A frame on another channel, a presence update or an unknown event type.
    Other,
}

/// Parse a text frame from the server for `channel`.
pub fn parse_frame(text: &str, channel: &str) -> Result<Incoming, String> {
    let envelope: Envelope = serde_json::from_str(text).map_err(|err| err.to_string())?;
    match envelope.e.as_str() {
        "ack" => Ok(Incoming::Ack(envelope.off.unwrap_or_default())),
        "msg" => {
            if envelope.ch.as_deref() != Some(channel) {
                return Ok(Incoming::Other);
            }
            let Some(data) = envelope.data else {
                return Ok(Incoming::Other);
            };
            // presence payloads carry an `e` of their own
            if data.get("e").is_some() {
                return Ok(Incoming::Other);
            }
            match serde_json::from_value::<AliveEvent>(data) {
                Ok(event) => Ok(Incoming::Event(event)),
                Err(_) => Ok(Incoming::Other),
            }
        }
        _ => Ok(Incoming::Other),
    }
}

/// The `subscribe` frame for `topic` at `offset`.
pub fn subscribe_frame(signed_channel: &str, offset: &str) -> String {
    let mut subscribe = serde_json::Map::new();
    subscribe.insert(
        signed_channel.to_string(),
        serde_json::Value::String(offset.to_string()),
    );
    serde_json::json!({ "subscribe": subscribe }).to_string()
}

/// `getUrlWithPresenceId`: the socket URL with the client's presence id.
pub fn socket_url(base: &str, presence_id: &str, connection_count: u32) -> String {
    let separator = if base.contains('?') { '&' } else { '?' };
    format!("{base}{separator}shared=false&p={presence_id}.{connection_count}")
}

/// GHD `retry` / `StableSocket`: exponential backoff capped at ten minutes.
pub fn backoff(attempt: u32) -> Duration {
    let secs = 1u64 << attempt.min(9);
    Duration::from_secs(secs.min(600))
}

/// One subscription: keeps a socket open to the Alive service, resubscribes
/// after every reconnect and calls `on_event` for each Desktop event.
/// Returns when `stop` is set. `websocket_url` is asked for a fresh URL
/// before every connection, as `AliveSession` does.
pub fn run_session(
    channel: AliveChannel,
    websocket_url: impl Fn() -> Option<String>,
    stop: Arc<AtomicBool>,
    mut on_event: impl FnMut(AliveEvent),
) {
    let presence_id = format!("corvane-{}", std::process::id());
    let mut offset = String::new();
    let mut attempt: u32 = 0;
    let mut connection_count: u32 = 0;
    while !stop.load(Ordering::Relaxed) {
        let Some(url) = websocket_url() else {
            // 404: the endpoint does not offer Alive (GHD stops there)
            info!("Alive is not available for this endpoint");
            return;
        };
        connection_count += 1;
        let url = socket_url(&url, &presence_id, connection_count);
        let mut socket = match tungstenite::connect(&url) {
            Ok((socket, _)) => socket,
            Err(err) => {
                warn!(%err, "Alive connection failed");
                sleep_unless_stopped(&stop, backoff(attempt));
                attempt = attempt.saturating_add(1);
                continue;
            }
        };
        set_read_timeout(&mut socket, Duration::from_secs(30));
        if let Err(err) = socket.send(Message::Text(
            subscribe_frame(&channel.signed_channel, &offset).into(),
        )) {
            warn!(%err, "Alive subscribe failed");
            sleep_unless_stopped(&stop, backoff(attempt));
            attempt = attempt.saturating_add(1);
            continue;
        }
        info!(channel = %channel.channel_name, "subscribed to the Alive channel");
        attempt = 0;
        loop {
            if stop.load(Ordering::Relaxed) {
                let _ = socket.close(None);
                return;
            }
            match socket.read() {
                Ok(Message::Text(text)) => match parse_frame(&text, &channel.channel_name) {
                    Ok(Incoming::Ack(off)) => offset = off,
                    Ok(Incoming::Event(event)) => {
                        debug!(?event, "Alive event");
                        on_event(event);
                    }
                    Ok(Incoming::Other) => {}
                    Err(err) => debug!(%err, "unreadable Alive frame"),
                },
                Ok(Message::Close(frame)) => {
                    info!(?frame, "Alive socket closed by the server");
                    break;
                }
                // pings are answered by tungstenite on the next flush
                Ok(_) => {
                    let _ = socket.flush();
                }
                Err(tungstenite::Error::Io(err))
                    if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                {
                    let _ = socket.flush();
                }
                Err(err) => {
                    warn!(%err, "Alive socket error");
                    break;
                }
            }
        }
        if !stop.load(Ordering::Relaxed) {
            sleep_unless_stopped(&stop, backoff(attempt));
            attempt = attempt.saturating_add(1);
        }
    }
}

fn set_read_timeout(socket: &mut WebSocket<MaybeTlsStream<TcpStream>>, timeout: Duration) {
    let stream = match socket.get_mut() {
        MaybeTlsStream::Plain(stream) => stream,
        MaybeTlsStream::Rustls(tls) => tls.get_mut(),
        _ => return,
    };
    let _ = stream.set_read_timeout(Some(timeout));
}

fn sleep_unless_stopped(stop: &AtomicBool, duration: Duration) {
    let step = Duration::from_millis(250);
    let mut slept = Duration::ZERO;
    while slept < duration {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        std::thread::sleep(step);
        slept += step;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_desktop_events_from_msg_frames() {
        let frame = r#"{"e":"msg","ch":"desktop:1","off":"42","data":{"type":"pr-review-submit","timestamp":1,"owner":"octo","repo":"cat","pull_request_number":7,"state":"APPROVED","review_id":"99"}}"#;
        match parse_frame(frame, "desktop:1").unwrap() {
            Incoming::Event(AliveEvent::ReviewSubmit {
                owner,
                repo,
                pull_request_number,
                state,
                review_id,
                ..
            }) => {
                assert_eq!((owner.as_str(), repo.as_str()), ("octo", "cat"));
                assert_eq!(pull_request_number, 7);
                assert_eq!(state, "APPROVED");
                assert_eq!(review_id, "99");
            }
            other => panic!("unexpected {other:?}"),
        }
        let comment = r#"{"e":"msg","ch":"desktop:1","off":"43","data":{"type":"pr-comment","subtype":"issue-comment","timestamp":1,"owner":"o","repo":"r","pull_request_number":1,"comment_id":"5"}}"#;
        assert!(matches!(
            parse_frame(comment, "desktop:1").unwrap(),
            Incoming::Event(AliveEvent::Comment {
                subtype: CommentSubtype::IssueComment,
                ..
            })
        ));
        let checks = r#"{"e":"msg","ch":"desktop:1","off":"44","data":{"type":"pr-checks-failed","timestamp":1,"owner":"o","repo":"r","pull_request_number":1,"check_suite_id":8,"commit_sha":"abc"}}"#;
        assert!(matches!(
            parse_frame(checks, "desktop:1").unwrap(),
            Incoming::Event(AliveEvent::ChecksFailed {
                check_suite_id: 8,
                ..
            })
        ));
    }

    #[test]
    fn acks_other_channels_and_presence_are_not_events() {
        assert_eq!(
            parse_frame(r#"{"e":"ack","off":"12"}"#, "c").unwrap(),
            Incoming::Ack("12".into())
        );
        assert_eq!(
            parse_frame(
                r#"{"e":"msg","ch":"other","off":"1","data":{"type":"pr-comment"}}"#,
                "c"
            )
            .unwrap(),
            Incoming::Other
        );
        assert_eq!(
            parse_frame(
                r#"{"e":"msg","ch":"c","off":"1","data":{"e":"pf","d":[]}}"#,
                "c"
            )
            .unwrap(),
            Incoming::Other
        );
        assert_eq!(
            parse_frame(
                r#"{"e":"msg","ch":"c","off":"1","data":{"type":"unknown"}}"#,
                "c"
            )
            .unwrap(),
            Incoming::Other
        );
        assert!(parse_frame("not json", "c").is_err());
    }

    #[test]
    fn subscribe_frame_and_url_follow_the_client() {
        assert_eq!(
            subscribe_frame("abc--sig", "7"),
            r#"{"subscribe":{"abc--sig":"7"}}"#
        );
        assert_eq!(
            socket_url("wss://alive.github.com/_sockets/u/1/ws?x=1", "id", 2),
            "wss://alive.github.com/_sockets/u/1/ws?x=1&shared=false&p=id.2"
        );
        assert_eq!(backoff(0), Duration::from_secs(1));
        assert_eq!(backoff(3), Duration::from_secs(8));
        assert_eq!(backoff(20), Duration::from_secs(512));
    }
}
