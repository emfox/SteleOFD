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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_spec_is_an_open_ring() {
        let ring = KeyRing::parse("").expect("an empty ring");
        assert!(ring.is_open());
        assert!(ring.is_empty());
        assert_eq!(ring.len(), 0);
    }

    #[test]
    fn a_pair_maps_a_key_to_its_tenant() {
        let ring = KeyRing::parse("acme:secret,rival:other").expect("a ring");
        assert!(!ring.is_open());
        assert_eq!(ring.len(), 2);
        assert_eq!(ring.tenant_of("secret"), Some("acme"));
        assert_eq!(ring.tenant_of("other"), Some("rival"));
        assert_eq!(ring.tenant_of("missing"), None);
    }

    #[test]
    fn whitespace_around_pairs_is_ignored() {
        let ring = KeyRing::parse(" acme : secret , rival:other ").expect("a ring");
        assert_eq!(ring.tenant_of("secret"), Some("acme"));
        assert_eq!(ring.tenant_of("other"), Some("rival"));
    }

    #[test]
    fn a_pair_without_a_key_is_refused() {
        // A tenant with an empty key cannot be distinguished from no header at
        // all, which would let the tenant in without presenting anything.
        assert!(KeyRing::parse("acme").is_err());
        assert!(KeyRing::parse("acme:").is_err());
        assert!(KeyRing::parse(":secret").is_err());
    }

    #[test]
    fn a_duplicated_key_is_refused() {
        assert!(
            KeyRing::parse("acme:secret,rival:secret").is_err(),
            "one key cannot stand for two tenants"
        );
    }

    #[test]
    fn a_bearer_header_is_read_with_or_without_the_scheme() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(AUTHORIZATION, "Bearer secret".parse().expect("a header"));
        assert_eq!(bearer(&headers), Some("secret"));

        let mut headers = axum::http::HeaderMap::new();
        headers.insert(AUTHORIZATION, "secret".parse().expect("a header"));
        assert_eq!(bearer(&headers), Some("secret"));

        let mut headers = axum::http::HeaderMap::new();
        headers.insert(API_KEY, "secret".parse().expect("a header"));
        assert_eq!(bearer(&headers), Some("secret"));

        let headers = axum::http::HeaderMap::new();
        assert_eq!(bearer(&headers), None, "no header means no key");
    }

    #[test]
    fn the_explicit_key_header_wins_over_the_bearer_one() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(AUTHORIZATION, "Bearer bearer".parse().expect("a header"));
        headers.insert(API_KEY, "explicit".parse().expect("a header"));
        assert_eq!(bearer(&headers), Some("explicit"));
    }
}
