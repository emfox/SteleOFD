//! Coverage for the HTTP surface: every route driven through the router, the
//! error statuses included, plus one request over a real socket. The document
//! shape comes from the shared testkit, so this suite cannot drift.

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode};
use sofd_core::Document;
use sofd_server::config::Settings;
use sofd_server::dto::{Opened, SearchHit, SignatureStatus};
use sofd_server::{AppState, KeyRing, RateLimit, Store, api};
use sofd_testkit::{SIGN_FILE, fixture_bytes};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower::ServiceExt;
use uuid::Uuid;

const MAX_BODY: usize = 8 << 20;

/// A router over an open (unauthenticated) service, which is the default.
fn app(capacity: usize) -> Router {
    api::router(AppState::open(Store::new(capacity)), MAX_BODY)
}

/// A router that requires one of `spec`, parsed as `tenant:key,tenant:key`.
fn guarded(capacity: usize, spec: &str) -> Router {
    let keys = KeyRing::parse(spec).expect("a usable key ring");
    api::router(AppState::guarded(Store::new(capacity), keys), MAX_BODY)
}

/// A router holding every caller to `spec`, written `rate,burst`.
fn throttled(capacity: usize, spec: &str) -> Router {
    let (rate, burst) = spec.split_once(',').expect("rate,burst");
    let limit = RateLimit::new(
        rate.trim().parse().expect("a rate"),
        burst.trim().parse().expect("a burst"),
    );
    api::router(
        AppState::open(Store::new(capacity)).limited(limit),
        MAX_BODY,
    )
}

/// The same request as [`get`], carrying a bearer key.
fn get_as(uri: &str, key: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header("authorization", format!("Bearer {key}"))
        .body(Body::empty())
        .expect("request")
}

/// An upload carrying a key, so an authenticated caller can post.
fn upload_as(uri: &str, bytes: Vec<u8>, key: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/octet-stream")
        .header("authorization", format!("Bearer {key}"))
        .body(Body::from(bytes))
        .expect("request")
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

    let uri = format!("/api/v1/documents/{id}/search?needle=sofd");
    let (status, _, body) = call(&router, get(&uri)).await;
    assert_eq!(status, StatusCode::OK);
    let hits: Vec<SearchHit> = serde_json::from_slice(&body).expect("hits");
    assert_eq!(hits.len(), 1, "the page carries the text once");
    assert_eq!(hits[0].page, 0);
    assert!(hits[0].snippet.contains("sOFD"));
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
async fn an_uploaded_id_still_answers_after_a_restart() {
    // The end-to-end form of the persistence guarantee: upload through the
    // real route, throw the store away, build a new one over the same
    // directory, and the id the caller holds still renders.
    let dir = std::env::temp_dir().join(format!("sofd-restart-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let id = {
        let store = Store::persistent(4, &dir).expect("a store");
        let router = api::router(AppState::open(store), MAX_BODY);
        open_fixture(&router, false).await
    };

    let restarted = Store::persistent(4, &dir).expect("a new store");
    assert_eq!(restarted.len(), 1, "the earlier upload was adopted");
    let router = api::router(AppState::open(restarted), MAX_BODY);
    let (status, _, body) =
        call(&router, get(&format!("/api/v1/documents/{id}/pages/0/png"))).await;
    assert_eq!(status, StatusCode::OK, "the restarted store renders it");
    assert!(body.starts_with(b"\x89PNG\r\n\x1a\n"), "PNG magic");

    let _ = std::fs::remove_dir_all(&dir);
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
        "SOFD_BIND" => Some("0.0.0.0:9000".to_owned()),
        "SOFD_MAX_BODY" => Some("1024".to_owned()),
        "SOFD_MAX_DOCUMENTS" => Some("4".to_owned()),
        "SOFD_DATA_DIR" => Some("/var/lib/sofd".to_owned()),
        "SOFD_API_KEYS" => Some("acme:secret".to_owned()),
        "SOFD_RATE_LIMIT" => Some("5,10".to_owned()),
        "SOFD_DRAIN_TIMEOUT" => Some("30".to_owned()),
        _ => None,
    })
    .expect("a full environment");
    assert_eq!(configured.bind.port(), 9000);
    assert_eq!(configured.max_body_bytes, 1024);
    assert_eq!(configured.max_documents, 4);
    assert_eq!(
        configured.data_dir.as_deref(),
        Some(std::path::Path::new("/var/lib/sofd")),
        "the data directory is read when set"
    );
    assert_eq!(configured.api_keys.tenant_of("secret"), Some("acme"));
    assert_eq!(configured.rate_limit.rate, 5.0);
    assert_eq!(configured.rate_limit.burst, 10.0);
    assert_eq!(
        configured.drain.duration(),
        std::time::Duration::from_secs(30),
        "the drain grace period is read when set"
    );
    assert_eq!(
        empty.data_dir, None,
        "and left unset when the environment says nothing, so the store stays in memory"
    );
    assert!(
        empty.api_keys.is_open(),
        "no keys means an open, single-tenant service"
    );

    assert!(
        Settings::from(|name| (name == "SOFD_API_KEYS").then(|| "acme".to_owned())).is_err(),
        "a key entry without a secret is refused"
    );
    assert!(
        Settings::from(|name| (name == "SOFD_RATE_LIMIT").then(|| "fast".to_owned())).is_err(),
        "a rate that does not parse is refused"
    );
    assert!(
        Settings::from(|name| (name == "SOFD_RATE_LIMIT").then(|| "0,1".to_owned())).is_err(),
        "a rate of zero would refuse every request"
    );
    assert!(
        Settings::from(|name| (name == "SOFD_DRAIN_TIMEOUT").then(|| "0".to_owned())).is_err(),
        "a grace period of zero is a kill, not a drain"
    );
    assert!(
        Settings::from(|name| (name == "SOFD_DRAIN_TIMEOUT").then(|| "soon".to_owned())).is_err(),
        "a grace period that does not parse is refused"
    );

    assert!(
        Settings::from(|name| (name == "SOFD_BIND").then(|| "nowhere".to_owned())).is_err(),
        "an unparseable address is refused"
    );
    assert!(
        Settings::from(|name| (name == "SOFD_MAX_DOCUMENTS").then(|| "0".to_owned())).is_err(),
        "a zero capacity is refused"
    );
}

#[tokio::test]
async fn a_guarded_service_answers_health_without_a_key() {
    // Health is how a load balancer decides to route, so it stays open even
    // when the rest of the service is closed.
    let router = guarded(4, "acme:secret");
    let (status, _, body) = call(&router, get("/healthz")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(String::from_utf8_lossy(&body).contains(r#""status":"ok""#));
}

#[tokio::test]
async fn a_guarded_service_refuses_a_call_with_no_key() {
    let router = guarded(4, "acme:secret");
    let (status, _, body) = call(&router, upload("/api/v1/documents", fixture_bytes(false))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let failure: serde_json::Value = serde_json::from_slice(&body).expect("a failure body");
    assert!(
        failure["error"].as_str().unwrap().contains("API key"),
        "the message says what to send: {body:?}"
    );
}

#[tokio::test]
async fn a_guarded_service_refuses_a_key_it_does_not_know() {
    let router = guarded(4, "acme:secret");
    let (status, _, _) = call(
        &router,
        upload_as("/api/v1/documents", fixture_bytes(false), "wrong"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_known_key_opens_and_reads_a_document() {
    let router = guarded(4, "acme:secret");
    let (status, _, body) = call(
        &router,
        upload_as("/api/v1/documents", fixture_bytes(false), "secret"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&body)
    );
    let opened: Opened = serde_json::from_slice(&body).expect("an id and its facts");

    let (status, _, _) = call(
        &router,
        get_as(&format!("/api/v1/documents/{}", opened.id), "secret"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "the owner reads its own document");
}

#[tokio::test]
async fn one_tenant_cannot_read_anothers_document() {
    // The point of tenancy: a valid key for one organisation is not a valid
    // key for another organisation's document, and the refusal is a 404 so
    // the id's existence does not leak.
    let router = guarded(4, "acme:secret,rival:other");
    let (_, _, body) = call(
        &router,
        upload_as("/api/v1/documents", fixture_bytes(false), "secret"),
    )
    .await;
    let opened: Opened = serde_json::from_slice(&body).expect("an id and its facts");

    let uri = format!("/api/v1/documents/{}", opened.id);
    let (status, _, _) = call(&router, get_as(&uri, "other")).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "another tenant sees nothing, not a 403 that reveals the id"
    );
    let (status, _, _) = call(&router, get_as(&uri, "secret")).await;
    assert_eq!(status, StatusCode::OK, "the owner still reads it");
}

#[tokio::test]
async fn one_tenant_cannot_release_anothers_document() {
    let router = guarded(4, "acme:secret,rival:other");
    let (_, _, body) = call(
        &router,
        upload_as("/api/v1/documents", fixture_bytes(false), "secret"),
    )
    .await;
    let opened: Opened = serde_json::from_slice(&body).expect("an id and its facts");
    let uri = format!("/api/v1/documents/{}", opened.id);

    let (status, _, _) = call(
        &router,
        Request::builder()
            .method("DELETE")
            .uri(&uri)
            .header("authorization", "Bearer other")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "the delete is refused");
    let (status, _, _) = call(&router, get_as(&uri, "secret")).await;
    assert_eq!(status, StatusCode::OK, "and the document is still there");
}

#[tokio::test]
async fn an_open_service_keeps_working_when_a_key_is_offered() {
    // An open service must not break for a caller that always sends a key;
    // the key is simply not required, so every caller is the same tenant.
    let router = app(4);
    let (status, _, _) = call(
        &router,
        upload_as("/api/v1/documents", fixture_bytes(false), "anything"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn a_flood_is_slowed_with_a_retry_after() {
    // The burst lets a small flurry through at once and the limit catches the
    // flood behind it, so a caller that hammers the service is told to wait
    // rather than served without end.
    let router = throttled(4, "0.001,2");
    let (first, _, _) = call(&router, get("/healthz")).await;
    let (second, _, _) = call(&router, get("/healthz")).await;
    assert_eq!(first, StatusCode::OK);
    assert_eq!(second, StatusCode::OK, "the burst covers two requests");

    let (third, headers, body) = call(&router, get("/healthz")).await;
    assert_eq!(third, StatusCode::TOO_MANY_REQUESTS, "the third is refused");
    assert!(
        headers.contains_key("retry-after"),
        "the answer says how long to wait"
    );
    let failure: serde_json::Value = serde_json::from_slice(&body).expect("a failure body");
    assert!(
        failure["error"].as_str().unwrap().contains("too many"),
        "and explains itself: {body:?}"
    );
}

#[tokio::test]
async fn one_caller_flooding_does_not_slow_another() {
    // The bucket is per caller, so a key that floods is throttled without the
    // next key noticing.
    let router = throttled(4, "0.001,1");
    let uri = "/healthz";
    let (_, _, _) = call(&router, get_as(uri, "a")).await;
    let (flooded, _, _) = call(&router, get_as(uri, "a")).await;
    assert_eq!(flooded, StatusCode::TOO_MANY_REQUESTS);

    let (other, _, _) = call(&router, get_as(uri, "b")).await;
    assert_eq!(other, StatusCode::OK, "another caller has its own bucket");
}
