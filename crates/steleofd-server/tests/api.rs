//! Coverage for the HTTP surface: every route driven through the router, the
//! error statuses included, plus one request over a real socket. The document
//! shape comes from the shared testkit, so this suite cannot drift.

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode};
use steleofd_core::Document;
use steleofd_server::Store;
use steleofd_server::api;
use steleofd_server::config::Settings;
use steleofd_server::dto::{Opened, SearchHit, SignatureStatus};
use steleofd_testkit::{SIGN_FILE, fixture_bytes};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower::ServiceExt;
use uuid::Uuid;

const MAX_BODY: usize = 8 << 20;

fn app(capacity: usize) -> Router {
    api::router(Store::new(capacity), MAX_BODY)
}

/// Send one request through the router and split the answer apart.
async fn call(router: &Router, request: Request<Body>) -> (StatusCode, HeaderMap, Bytes) {
    let response = router
        .clone()
        .oneshot(request)
        .await
        .expect("the router answers");
    let status = response.status();
    let headers = response.headers().clone();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("a body");
    (status, headers, body)
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request")
}

fn upload(uri: &str, bytes: Vec<u8>) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/octet-stream")
        .body(Body::from(bytes))
        .expect("request")
}

/// Upload a fixture and hand back the id the service assigned it.
async fn open_fixture(router: &Router, signed: bool) -> Uuid {
    let (status, _, body) = call(router, upload("/api/v1/documents", fixture_bytes(signed))).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&body)
    );
    let opened: Opened = serde_json::from_slice(&body).expect("an id and its facts");
    opened.id
}

#[test]
fn the_document_handle_is_send_so_the_store_can_share_it() {
    fn assert_send<T: Send>() {}
    assert_send::<Document>();
}

#[tokio::test]
async fn health_reports_the_service_and_its_load() {
    let router = app(4);
    let (status, _, body) = call(&router, get("/healthz")).await;
    assert_eq!(status, StatusCode::OK);
    let text = String::from_utf8(body.to_vec()).expect("json");
    assert!(text.contains(r#""status":"ok""#), "{text}");
    assert!(text.contains(r#""documents":0"#), "{text}");
}

#[tokio::test]
async fn an_upload_returns_an_id_and_the_facts() {
    let router = app(4);
    let id = open_fixture(&router, false).await;

    let (status, _, body) = call(&router, get(&format!("/api/v1/documents/{id}"))).await;
    assert_eq!(status, StatusCode::OK);
    let text = String::from_utf8(body.to_vec()).expect("json");
    assert!(text.contains(r#""version":"1.0""#), "{text}");
    assert!(text.contains(r#""pages":1"#), "{text}");
    assert!(text.contains(r#""width_mm":210"#), "{text}");
    assert!(text.contains(r#""height_mm":297"#), "{text}");
    assert!(text.contains(r#""embedded_fonts":1"#), "{text}");

    let (_, _, body) = call(&router, get("/healthz")).await;
    let text = String::from_utf8(body.to_vec()).expect("json");
    assert!(text.contains(r#""documents":1"#), "the upload is counted");
}

#[tokio::test]
async fn a_release_frees_the_id() {
    let router = app(4);
    let id = open_fixture(&router, false).await;

    let (status, _, _) = call(
        &router,
        Request::builder()
            .method("DELETE")
            .uri(format!("/api/v1/documents/{id}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _, _) = call(&router, get(&format!("/api/v1/documents/{id}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "the id is gone");
}

#[tokio::test]
async fn pages_rasterize_convert_and_search() {
    let router = app(4);
    let id = open_fixture(&router, false).await;

    let (status, headers, body) =
        call(&router, get(&format!("/api/v1/documents/{id}/pages/0/png"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-type"], "image/png");
    assert!(body.starts_with(b"\x89PNG\r\n\x1a\n"), "PNG magic");

    let (status, headers, body) =
        call(&router, get(&format!("/api/v1/documents/{id}/pages/0/svg"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-type"], "image/svg+xml");
    assert!(
        String::from_utf8_lossy(&body).contains("<svg"),
        "an SVG document"
    );

    let (status, headers, body) = call(&router, get(&format!("/api/v1/documents/{id}/pdf"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-type"], "application/pdf");
    assert!(body.starts_with(b"%PDF"), "PDF magic");

    let uri = format!("/api/v1/documents/{id}/search?needle=steleofd");
    let (status, _, body) = call(&router, get(&uri)).await;
    assert_eq!(status, StatusCode::OK);
    let hits: Vec<SearchHit> = serde_json::from_slice(&body).expect("hits");
    assert_eq!(hits.len(), 1, "the page carries the text once");
    assert_eq!(hits[0].page, 0);
    assert!(hits[0].snippet.contains("SteleOFD"));
}

#[tokio::test]
async fn the_dpi_query_is_clamped_to_a_sane_range() {
    let router = app(4);
    let id = open_fixture(&router, false).await;
    let render = |dpi: f64| {
        let router = router.clone();
        let uri = format!("/api/v1/documents/{id}/pages/0/png?dpi={dpi}");
        async move {
            let (status, _, body) = call(&router, get(&uri)).await;
            assert_eq!(status, StatusCode::OK);
            body.len()
        }
    };

    assert_eq!(render(1.0).await, render(12.0).await, "below the floor");
    assert_eq!(
        render(100_000.0).await,
        render(600.0).await,
        "above the ceiling"
    );
}

#[tokio::test]
async fn a_page_out_of_range_is_a_404() {
    let router = app(4);
    let id = open_fixture(&router, false).await;
    let (status, _, body) =
        call(&router, get(&format!("/api/v1/documents/{id}/pages/5/png"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let failure: serde_json::Value = serde_json::from_slice(&body).expect("a failure body");
    assert!(failure["error"].as_str().unwrap().contains("page"));
}

#[tokio::test]
async fn a_search_without_a_needle_is_a_bad_request() {
    let router = app(4);
    let id = open_fixture(&router, false).await;
    let (status, _, _) = call(&router, get(&format!("/api/v1/documents/{id}/search"))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn verification_needs_a_signature_list() {
    let router = app(4);
    let unsigned = open_fixture(&router, false).await;
    let (status, _, body) = call(
        &router,
        get(&format!("/api/v1/documents/{unsigned}/verify")),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let failure: serde_json::Value = serde_json::from_slice(&body).expect("a failure body");
    assert!(!failure["error"].as_str().unwrap().is_empty());

    let signed = open_fixture(&router, true).await;
    let (status, _, body) = call(&router, get(&format!("/api/v1/documents/{signed}/verify"))).await;
    assert_eq!(status, StatusCode::OK);
    let statuses: Vec<SignatureStatus> = serde_json::from_slice(&body).expect("statuses");
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].location, SIGN_FILE);
    assert!(statuses[0].valid, "the embedded certificate verifies");
}

#[tokio::test]
async fn damaged_bytes_are_refused_with_the_engine_message() {
    let router = app(4);
    let (status, _, body) =
        call(&router, upload("/api/v1/documents", b"not an ofd".to_vec())).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let failure: serde_json::Value = serde_json::from_slice(&body).expect("a failure body");
    assert!(
        failure["error"].as_str().unwrap().contains("container"),
        "the container layer explains itself: {body:?}"
    );
}

#[tokio::test]
async fn every_route_reports_an_unknown_id_as_missing() {
    let router = app(4);
    let ghost = Uuid::new_v4();
    for uri in [
        format!("/api/v1/documents/{ghost}"),
        format!("/api/v1/documents/{ghost}/pdf"),
        format!("/api/v1/documents/{ghost}/verify"),
        format!("/api/v1/documents/{ghost}/search?needle=x"),
        format!("/api/v1/documents/{ghost}/pages/0/png"),
    ] {
        let (status, _, _) = call(&router, get(&uri)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
}

#[tokio::test]
async fn the_store_refuses_past_its_capacity() {
    let router = app(1);
    open_fixture(&router, false).await;
    let (status, _, body) = call(&router, upload("/api/v1/documents", fixture_bytes(false))).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let failure: serde_json::Value = serde_json::from_slice(&body).expect("a failure body");
    assert!(failure["error"].as_str().unwrap().contains("too many"));
}

#[tokio::test]
async fn the_service_answers_over_a_socket() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let address = listener.local_addr().expect("an address");
    tokio::spawn(async move {
        axum::serve(listener, app(4))
            .await
            .expect("the server runs");
    });

    let mut stream = tokio::net::TcpStream::connect(address)
        .await
        .expect("connect");
    let request = b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    stream.write_all(request).await.expect("write");
    let mut answer = String::new();
    stream.read_to_string(&mut answer).await.expect("read");

    assert!(answer.starts_with("HTTP/1.1 200 OK"), "{answer}");
    assert!(answer.contains(r#""status":"ok""#), "{answer}");
}

#[test]
fn settings_come_from_the_environment_or_the_defaults() {
    let empty = Settings::from(|_| None).expect("defaults are usable");
    assert_eq!(empty.bind, Settings::default().bind);
    assert_eq!(empty.max_body_bytes, 64 << 20);
    assert_eq!(empty.max_documents, 256);

    let configured = Settings::from(|name| match name {
        "STELEOFD_BIND" => Some("0.0.0.0:9000".to_owned()),
        "STELEOFD_MAX_BODY" => Some("1024".to_owned()),
        "STELEOFD_MAX_DOCUMENTS" => Some("4".to_owned()),
        _ => None,
    })
    .expect("a full environment");
    assert_eq!(configured.bind.port(), 9000);
    assert_eq!(configured.max_body_bytes, 1024);
    assert_eq!(configured.max_documents, 4);

    assert!(
        Settings::from(|name| (name == "STELEOFD_BIND").then(|| "nowhere".to_owned())).is_err(),
        "an unparseable address is refused"
    );
    assert!(
        Settings::from(|name| (name == "STELEOFD_MAX_DOCUMENTS").then(|| "0".to_owned())).is_err(),
        "a zero capacity is refused"
    );
}
