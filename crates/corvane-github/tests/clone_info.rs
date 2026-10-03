//! `Client::repository_clone_info` against a canned local HTTP server.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

use corvane_github::{Client, Endpoint, RepositoryCloneInfo};

/// Serve one response per request, sending each request head back.
fn serve(responses: Vec<(u16, &'static str)>) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for (status, body) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                head.push_str(&line);
            }
            tx.send(head).unwrap();
            let reason = if status == 200 { "OK" } else { "Not Found" };
            write!(
                stream,
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    (format!("http://127.0.0.1:{port}/api/v3"), rx)
}

const REPO: &str = r#"{"name":"app","owner":{"login":"team"},"html_url":"https://ghe.corp/team/app","clone_url":"https://ghe.corp/team/app.git","ssh_url":"git@ghe.corp:team/app.git","default_branch":"trunk"}"#;

#[test]
fn resolves_https_ssh_and_not_found() {
    let (base, heads) = serve(vec![
        (200, REPO),
        (200, REPO),
        (404, r#"{"message":"Not Found"}"#),
    ]);
    let anonymous = Client::new(Endpoint::from_api_base(&base), "");
    assert_eq!(
        anonymous
            .repository_clone_info("team", "app", false)
            .unwrap(),
        Some(RepositoryCloneInfo {
            url: "https://ghe.corp/team/app.git".into(),
            default_branch: Some("trunk".into()),
        })
    );
    let head = heads.recv().unwrap();
    assert!(head.starts_with("GET /api/v3/repos/team/app "), "{head}");
    // `Account.anonymous()` sends no Authorization header
    assert!(!head.to_lowercase().contains("authorization"), "{head}");

    let signed_in = Client::new(Endpoint::from_api_base(&base), "t0ken");
    let info = signed_in
        .repository_clone_info("team", "app", true)
        .unwrap()
        .unwrap();
    assert_eq!(info.url, "git@ghe.corp:team/app.git");
    assert!(
        heads
            .recv()
            .unwrap()
            .to_lowercase()
            .contains("authorization: bearer t0ken")
    );

    assert_eq!(
        signed_in
            .repository_clone_info("team", "missing", false)
            .unwrap(),
        None
    );
}
