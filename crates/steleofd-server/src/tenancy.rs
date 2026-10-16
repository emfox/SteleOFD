//! Who is calling, and which documents they may see.
//! A document belongs to the tenant that uploaded it and no other tenant can
//! read it; with no keys configured every caller shares one implicit tenant.

use std::collections::HashMap;

use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::request::Parts;

use crate::error::ApiError;

/// The tenant attributed to a caller when no keys are configured.
pub const ANONYMOUS: &str = "anonymous";

/// The header a bearer key travels in.
const AUTHORIZATION: &str = "authorization";
/// The header a key may travel in instead, for callers that dislike Bearer.
const API_KEY: &str = "x-api-key";

/// The keys the service accepts, and the tenant each one stands for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyRing {
    tenants: HashMap<String, String>,
}

impl KeyRing {
    /// A ring that accepts nobody, which is what an unconfigured service is.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse `tenant:key,tenant:key` into a ring. A pair without a colon names
    /// a tenant whose key is the whole text. An empty key is refused, since it
    /// cannot be told from a missing header.
    pub fn parse(spec: &str) -> Result<Self, BadKey> {
        let mut tenants = HashMap::new();
        for pair in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (tenant, key) = match pair.split_once(':') {
                Some((tenant, key)) => (tenant.trim(), key.trim()),
                None => (pair, ""),
            };
            if tenant.is_empty() {
                return Err(BadKey::new(pair));
            }
            if key.is_empty() {
                return Err(BadKey::new(pair));
            }
            if tenants.insert(key.to_owned(), tenant.to_owned()).is_some() {
                return Err(BadKey::new(pair));
            }
        }
        Ok(Self { tenants })
    }

    /// Whether the ring knows no key, so every caller is the same tenant.
    pub fn is_open(&self) -> bool {
        self.tenants.is_empty()
    }

    /// The tenant a key stands for, if the ring knows it.
    pub fn tenant_of(&self, key: &str) -> Option<&str> {
        self.tenants.get(key).map(String::as_str)
    }

    /// How many keys the ring holds.
    pub fn len(&self) -> usize {
        self.tenants.len()
    }

    /// Whether the ring holds no key.
    pub fn is_empty(&self) -> bool {
        self.tenants.is_empty()
    }
}

/// A key that could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadKey {
    value: String,
}

impl BadKey {
    /// The offending pair, for the message.
    pub fn new(value: &str) -> Self {
        Self {
            value: value.to_owned(),
        }
    }
}

impl std::fmt::Display for BadKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unusable API key entry: {}", self.value)
    }
}

impl std::error::Error for BadKey {}

/// The tenant a request is attributed to, extracted from its key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tenant {
    id: String,
}

impl Tenant {
    /// The tenant id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Whether this is the shared tenant of an open service.
    pub fn is_anonymous(&self) -> bool {
        self.id == ANONYMOUS
    }
}

impl FromRequestParts<crate::AppState> for Tenant {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &crate::AppState,
    ) -> Result<Self, Self::Rejection> {
        let ring = &state.keys;
        let presented = bearer(&parts.headers);
        // An open service has one tenant and asks for no key; a key is still
        // accepted if one is offered and known, so a caller can be explicit
        // without the service insisting.
        if ring.is_open() {
            return Ok(Tenant {
                id: match presented.and_then(|key| ring.tenant_of(key)) {
                    Some(tenant) => tenant.to_owned(),
                    None => ANONYMOUS.to_owned(),
                },
            });
        }
        let key = presented.ok_or_else(missing_key)?;
        let tenant = ring.tenant_of(key).ok_or_else(unknown_key)?;
        Ok(Tenant {
            id: tenant.to_owned(),
        })
    }
}

/// The key a request carries, from either header.
fn bearer(headers: &axum::http::HeaderMap) -> Option<&str> {
    if let Some(value) = headers.get(API_KEY).and_then(|v| v.to_str().ok()) {
        let value = value.trim();
        if !value.is_empty() {
            return Some(value);
        }
    }
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?.trim();
    let token = value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
        .unwrap_or(value);
    let token = token.trim();
    (!token.is_empty()).then_some(token)
}

/// The caller sent no key, and the service needs one.
fn missing_key() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "this service requires an API key; send it as `Authorization: Bearer <key>`",
    )
}

/// The caller sent a key the service does not know.
fn unknown_key() -> ApiError {
    ApiError::new(StatusCode::UNAUTHORIZED, "the API key is not recognised")
}

/// One caller naming a tenant it is not: a 403 rather than a 404 would leak
/// that the id exists, so the route answers as if the document were missing.
pub fn foreign_document() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "no such document")
}
