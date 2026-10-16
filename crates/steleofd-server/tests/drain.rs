//! Stopping the service without cutting work in half.
//! The unit tests cover the signal's own rules; these drive a real listener, so
//! what is pinned is that a stop resolves the serve future and closes the port.

use std::time::Duration;

use steleofd_server::api;
use steleofd_server::{AppState, Drain, Store};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// How long a test waits for something that should be quick, so a hang fails
/// the test rather than the suite.
const PATIENCE: Duration = Duration::from_secs(5);

/// Start the service on an ephemeral port, with a drain the caller can trip.
/// Returns the address and a handle that resolves when serving stops.
async fn serving() -> (std::net::SocketAddr, Drain, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let address = listener.local_addr().expect("an address");
    let drain = Drain::new();
    let app = api::router(AppState::open(Store::new(4)), 1 << 20);
    let stop = drain.clone();
    let serving =
        axum::serve(listener, app).with_graceful_shutdown(async move { stop.wait().await });
    let handle = tokio::spawn(async move {
        serving.await.expect("the server runs");
    });
    (address, drain, handle)
}

/// One whole request over a socket, returning the status line it got back.
async fn one_request(address: std::net::SocketAddr, path: &str) -> String {
    let mut stream = tokio::net::TcpStream::connect(address)
        .await
        .expect("connect");
    let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.expect("write");
    let mut answer = String::new();
    stream.read_to_string(&mut answer).await.expect("read");
    answer.lines().next().unwrap_or_default().to_owned()
}

#[tokio::test]
async fn a_stop_ends_the_serve_future() {
    // The half a rolling deploy depends on: a stop makes the serve future
    // resolve rather than hang, which is what lets the process exit and the
    // replacement take over.
    let (address, drain, mut serving) = serving().await;
    assert_eq!(one_request(address, "/healthz").await, "HTTP/1.1 200 OK");
    assert!(!drain.is_stopping(), "the service runs until asked to stop");

    drain.stop();
    tokio::time::timeout(PATIENCE, &mut serving)
        .await
        .expect("the serve future resolves on the stop")
        .expect("the serving task did not panic");
    assert!(drain.is_stopping());
}

#[tokio::test]
async fn the_port_stops_accepting_after_a_stop() {
    // With the serve future resolved the listener is closed, so a new caller
    // is refused rather than connected to a service that is going away.
    let (address, drain, mut serving) = serving().await;
    assert_eq!(one_request(address, "/healthz").await, "HTTP/1.1 200 OK");

    drain.stop();
    tokio::time::timeout(PATIENCE, &mut serving)
        .await
        .expect("the serve future resolves")
        .expect("the serving task did not panic");

    let refused = tokio::time::timeout(PATIENCE, async {
        loop {
            match one_request_opt(address, "/healthz").await {
                Some(line) => {
                    // Still answering: give the listener a moment to close.
                    assert!(line.starts_with("HTTP/1.1"), "unexpected answer {line:?}");
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                None => return,
            }
        }
    })
    .await;
    assert!(refused.is_ok(), "the port eventually stops answering");
}

/// One request, or `None` when the connection could not be made or was cut.
async fn one_request_opt(address: std::net::SocketAddr, path: &str) -> Option<String> {
    let mut stream = tokio::net::TcpStream::connect(address).await.ok()?;
    let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.ok()?;
    let mut answer = String::new();
    stream.read_to_string(&mut answer).await.ok()?;
    (!answer.is_empty()).then(|| answer.lines().next().unwrap_or_default().to_owned())
}
