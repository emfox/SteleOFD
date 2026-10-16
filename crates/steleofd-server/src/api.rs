//! The HTTP surface: routes, handlers and the query shapes they read. An upload
//! is the only route that takes a body; every other route works on the id it
//! returns. CPU-bound work goes to a blocking thread, never the async runtime.

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use steleofd_core::{Document, RenderOptions};
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::dto::{self, DocumentInfo, Health, Opened, SearchHit, SignatureStatus};
use crate::error::ApiError;
use crate::store::{self, Shared, Store};

/// Resolution used when a PNG request leaves `dpi` out.
const DEFAULT_DPI: f64 = 96.0;
/// Lowest resolution the service will rasterize at.
const MIN_DPI: f64 = 12.0;
/// Highest resolution the service will rasterize at. Print quality tops out
/// well below this; the real ceiling is the renderer's own pixel budget, which
/// an A4 page passes at 600 dpi and fails at 1200.
const MAX_DPI: f64 = 600.0;

/// The query a PNG render reads.
#[derive(Debug, Clone, Deserialize)]
pub struct PngQuery {
    /// Rasterization resolution, clamped to a sane range.
    pub dpi: Option<f64>,
}

/// The query a search reads.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchQuery {
    /// Text to look for; its absence is a bad request.
    pub needle: String,
}

/// Build the router the service runs. CORS is permissive because the callers
/// are browser front ends and OA portals; the body limit is what keeps an
/// upload from filling memory.
pub fn router(store: Store, max_body_bytes: usize) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/api/v1/documents", post(open))
        .route("/api/v1/documents/{id}", get(info).delete(close))
        .route("/api/v1/documents/{id}/pdf", get(pdf))
        .route("/api/v1/documents/{id}/search", get(search))
        .route("/api/v1/documents/{id}/verify", get(verify))
        .route("/api/v1/documents/{id}/pages/{page}/png", get(png))
        .route("/api/v1/documents/{id}/pages/{page}/svg", get(svg))
        .layer(DefaultBodyLimit::max(max_body_bytes))
        .layer(RequestBodyLimitLayer::new(max_body_bytes))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(store)
}

/// Report that the service is up, and how much it is holding.
async fn health(State(store): State<Store>) -> Json<Health> {
    Json(Health {
        status: "ok",
        documents: store.len(),
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// Open an uploaded document and keep it under a fresh id.
async fn open(
    State(store): State<Store>,
    body: Bytes,
) -> Result<(StatusCode, Json<Opened>), ApiError> {
    if store.is_full() {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "too many open documents",
        ));
    }
    let opened = blocking(move || {
        let mut doc = Document::open_bytes(&body)?;
        let info = DocumentInfo::of(&mut doc);
        Ok::<_, steleofd_error::OfdError>((doc, info))
    })
    .await??;
    let id = store.insert(opened.0);
    Ok((StatusCode::CREATED, Json(Opened { id, info: opened.1 })))
}

/// Report the facts of an open document.
async fn info(
    State(store): State<Store>,
    Path(id): Path<Uuid>,
) -> Result<Json<DocumentInfo>, ApiError> {
    let doc = shared(store, id)?;
    let info = blocking(move || DocumentInfo::of(&mut store::lock(&doc))).await?;
    Ok(Json(info))
}

/// Release an open document.
async fn close(State(store): State<Store>, Path(id): Path<Uuid>) -> Result<StatusCode, ApiError> {
    if store.remove(id) {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::unknown_document(id))
    }
}

/// Rasterize one page and answer with PNG bytes.
async fn png(
    State(store): State<Store>,
    Path((id, page)): Path<(Uuid, usize)>,
    Query(query): Query<PngQuery>,
) -> Result<Response, ApiError> {
    let doc = shared(store, id)?;
    let dpi = query.dpi.unwrap_or(DEFAULT_DPI).clamp(MIN_DPI, MAX_DPI);
    let bytes = blocking(move || {
        let options = RenderOptions {
            dpi,
            ..Default::default()
        };
        store::lock(&doc).to_png(page, &options)
    })
    .await??;
    Ok(png_response(bytes))
}

/// Convert one page and answer with an SVG document.
async fn svg(
    State(store): State<Store>,
    Path((id, page)): Path<(Uuid, usize)>,
) -> Result<Response, ApiError> {
    let doc = shared(store, id)?;
    let text = blocking(move || store::lock(&doc).to_svg(page)).await??;
    Ok(([(header::CONTENT_TYPE, "image/svg+xml")], text).into_response())
}

/// Convert the whole document and answer with PDF bytes.
async fn pdf(State(store): State<Store>, Path(id): Path<Uuid>) -> Result<Response, ApiError> {
    let doc = shared(store, id)?;
    let bytes = blocking(move || store::lock(&doc).to_pdf()).await??;
    Ok(([(header::CONTENT_TYPE, "application/pdf")], bytes).into_response())
}

/// Search every page of an open document.
async fn search(
    State(store): State<Store>,
    Path(id): Path<Uuid>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<SearchHit>>, ApiError> {
    let doc = shared(store, id)?;
    let hits = blocking(move || dto::search_hits(&store::lock(&doc), &query.needle)).await?;
    Ok(Json(hits))
}

/// Verify every signature of an open document.
async fn verify(
    State(store): State<Store>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<SignatureStatus>>, ApiError> {
    let doc = shared(store, id)?;
    let statuses = blocking(move || dto::signature_statuses(&mut store::lock(&doc))).await??;
    Ok(Json(statuses))
}

/// Answer with PNG bytes under the image content type.
fn png_response(bytes: Vec<u8>) -> Response {
    ([(header::CONTENT_TYPE, "image/png")], bytes).into_response()
}

/// The document behind an id, or a 404 naming what was missing.
fn shared(store: Store, id: Uuid) -> Result<Shared, ApiError> {
    store.get(id).ok_or_else(|| ApiError::unknown_document(id))
}

/// Hand CPU-bound work to a blocking thread, where it belongs.
async fn blocking<T, F>(work: F) -> Result<T, ApiError>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    Ok(tokio::task::spawn_blocking(work).await?)
}
