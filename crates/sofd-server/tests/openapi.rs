//! The OpenAPI description stays level with the service it describes.
//! A hand-written spec rots the moment a route is added and the spec is not, so
//! these tests hold the router's paths and the spec's paths together.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use sofd_server::api;
use sofd_server::openapi;
use sofd_server::{AppState, Store};
use tower::ServiceExt;

/// A router over an open service, which is all the spec route needs.
fn app() -> axum::Router {
    api::router(AppState::open(Store::new(4)), 1 << 20)
}

#[tokio::test]
async fn the_service_serves_its_own_description() {
    let request = Request::builder()
        .uri("/openapi.json")
        .body(Body::empty())
        .expect("request");
    let response = app().oneshot(request).await.expect("the router answers");
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("a body");
    let document: serde_json::Value = serde_json::from_slice(&body).expect("a JSON document");
    assert_eq!(document["openapi"], "3.0.3");
    assert_eq!(document["info"]["title"], "sofd-server");
}

#[test]
fn the_spec_is_a_well_formed_openapi_document() {
    let document = openapi::document();
    assert!(
        document["openapi"].as_str().unwrap().starts_with("3."),
        "it declares a 3.x version"
    );
    let info = &document["info"];
    for field in ["title", "version"] {
        assert!(
            info[field].as_str().is_some_and(|v| !v.is_empty()),
            "info.{field} is required"
        );
    }
    assert!(
        document["paths"].as_object().is_some_and(|p| !p.is_empty()),
        "the document names at least one path"
    );
}

#[test]
fn every_route_the_service_serves_is_described() {
    // The list is the contract; a route added to the router and not to it, or
    // the other way round, is what this catches.
    let document = openapi::document();
    let described: Vec<&str> = document["paths"]
        .as_object()
        .expect("paths")
        .keys()
        .map(String::as_str)
        .collect();
    for path in openapi::PATHS {
        assert!(
            described.contains(path),
            "{path} is served but not described"
        );
    }
    assert_eq!(
        described.len(),
        openapi::PATHS.len(),
        "and nothing is described that is not served: {described:?}"
    );
}

#[test]
fn every_operation_says_what_it_answers() {
    let document = openapi::document();
    for (path, item) in document["paths"].as_object().expect("paths") {
        for (method, operation) in item.as_object().expect("an item") {
            assert!(
                ["get", "post", "put", "delete", "patch"].contains(&method.as_str()),
                "{path} has an unexpected method {method}"
            );
            let responses = operation["responses"]
                .as_object()
                .unwrap_or_else(|| panic!("{method} {path} names no responses"));
            assert!(
                !responses.is_empty(),
                "{method} {path} names an empty response set"
            );
            assert!(
                operation["summary"].as_str().is_some_and(|s| !s.is_empty()),
                "{method} {path} has no summary"
            );
        }
    }
}

#[test]
fn a_route_naming_an_id_says_it_is_required() {
    // A path parameter that is not marked required is one a generated client
    // would leave out, so every `{...}` in a path has to be a required one.
    let document = openapi::document();
    for (path, item) in document["paths"].as_object().expect("paths") {
        let named: Vec<&str> = path
            .split('/')
            .filter(|part| part.starts_with('{') && part.ends_with('}'))
            .map(|part| &part[1..part.len() - 1])
            .collect();
        if named.is_empty() {
            continue;
        }
        for (method, operation) in item.as_object().expect("an item") {
            let declared: Vec<&str> = operation["parameters"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|p| p["in"] == "path")
                .map(|p| p["name"].as_str().unwrap_or_default())
                .collect();
            for name in &named {
                assert!(
                    declared.contains(name),
                    "{method} {path} does not declare path parameter {name}"
                );
            }
            for parameter in operation["parameters"].as_array().into_iter().flatten() {
                if parameter["in"] == "path" {
                    assert_eq!(
                        parameter["required"], true,
                        "a path parameter is always required: {method} {path}"
                    );
                }
            }
        }
    }
}

#[test]
fn every_referenced_schema_is_defined() {
    let document = openapi::document();
    let text = serde_json::to_string(&document).expect("serialise");
    let schemas = document["components"]["schemas"]
        .as_object()
        .expect("schemas");
    // Every `#/components/schemas/X` must name a schema that exists.
    for reference in text.split("#/components/schemas/").skip(1) {
        let name: String = reference
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        assert!(
            schemas.contains_key(&name),
            "a reference names {name}, which is not defined"
        );
    }
}

#[test]
fn the_spec_covers_authentication_and_rate_limiting() {
    // The two things a client has to know to talk to a real deployment: a key
    // may be required, and it may be told to slow down.
    let document = openapi::document();
    let schemes = &document["components"]["securitySchemes"];
    assert_eq!(schemes["bearer"]["scheme"], "bearer");
    assert_eq!(schemes["apiKey"]["name"], "x-api-key");
    let text = serde_json::to_string(&document).expect("serialise");
    assert!(text.contains("Retry-After"), "the 429 answer is described");
}
