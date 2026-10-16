//! An OpenAPI description of the HTTP surface.
//! Hand-written rather than derived, but the test beside the router holds it
//! level: every route served appears here, and every route here is served.

use serde_json::{Value, json};

/// The routes the service serves, as an OpenAPI path template. This is the
/// single list the router and the description are held to, so a route added to
/// one and not the other fails the test rather than the reader.
pub const PATHS: &[&str] = &[
    "/healthz",
    "/openapi.json",
    "/api/v1/documents",
    "/api/v1/documents/{id}",
    "/api/v1/documents/{id}/pdf",
    "/api/v1/documents/{id}/search",
    "/api/v1/documents/{id}/verify",
    "/api/v1/documents/{id}/pages/{page}/png",
    "/api/v1/documents/{id}/pages/{page}/svg",
];

/// The OpenAPI document describing the service, as JSON.
pub fn document() -> Value {
    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "steleofd-server",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "An OFD document service. Uploading an .ofd yields an id; \
                            every other route works on that id, so a caller renders \
                            page after page without resending the file.",
        },
        "servers": [{ "url": "/" }],
        "components": {
            "securitySchemes": {
                "bearer": {
                    "type": "http",
                    "scheme": "bearer",
                    "description": "An API key, when the service is configured with STELEOFD_API_KEYS.",
                },
                "apiKey": {
                    "type": "apiKey",
                    "in": "header",
                    "name": "x-api-key",
                    "description": "The same key in a header, for callers that dislike Bearer.",
                },
            },
            "schemas": schemas(),
        },
        "security": [{ "bearer": [] }, { "apiKey": [] }],
        "paths": paths(),
    })
}

/// The schema for every shape the routes return or accept.
fn schemas() -> Value {
    json!({
        "DocumentInfo": {
            "type": "object",
            "description": "What an open document declares, at a glance.",
            "required": ["version", "pages", "width_mm", "height_mm", "embedded_fonts", "signatures"],
            "properties": {
                "version": { "type": "string", "example": "1.0" },
                "pages": { "type": "integer", "minimum": 0 },
                "width_mm": { "type": "number", "example": 210.0 },
                "height_mm": { "type": "number", "example": 297.0 },
                "embedded_fonts": { "type": "integer", "minimum": 0 },
                "signatures": { "type": "integer", "minimum": 0 },
            },
        },
        "Opened": {
            "type": "object",
            "description": "What an upload returns: the id every later route works on, and its facts.",
            "required": ["id", "info"],
            "properties": {
                "id": { "type": "string", "format": "uuid" },
                "info": { "$ref": "#/components/schemas/DocumentInfo" },
            },
        },
        "SearchHit": {
            "type": "object",
            "description": "One search match, trimmed for a result list.",
            "required": ["page", "snippet"],
            "properties": {
                "page": { "type": "integer", "minimum": 0 },
                "snippet": { "type": "string" },
            },
        },
        "SignatureStatus": {
            "type": "object",
            "description": "The outcome of verifying one signature.",
            "required": ["location", "valid"],
            "properties": {
                "location": { "type": "string" },
                "valid": { "type": "boolean" },
            },
        },
        "Health": {
            "type": "object",
            "required": ["status", "documents", "version"],
            "properties": {
                "status": { "type": "string", "enum": ["ok"] },
                "documents": { "type": "integer", "minimum": 0 },
                "version": { "type": "string" },
            },
        },
        "Failure": {
            "type": "object",
            "description": "The body every non-2xx answer carries.",
            "required": ["error"],
            "properties": { "error": { "type": "string" } },
        },
    })
}

/// Every route, with its methods, parameters and answers.
fn paths() -> Value {
    let id = || {
        json!({
            "name": "id",
            "in": "path",
            "required": true,
            "schema": { "type": "string", "format": "uuid" },
            "description": "The id an upload returned.",
        })
    };
    let page = || {
        json!({
            "name": "page",
            "in": "path",
            "required": true,
            "schema": { "type": "integer", "minimum": 0 },
            "description": "Zero-based page index.",
        })
    };
    let not_found = json!({
        "description": "No such document, or no page at that index.",
        "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Failure" } } },
    });
    let unauthorised = json!({
        "description": "A key is required, or the one sent is not recognised.",
        "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Failure" } } },
    });
    let too_many = json!({
        "description": "The caller is over its rate limit; Retry-After says how long.",
        "headers": {
            "Retry-After": { "schema": { "type": "integer" }, "description": "Seconds to wait." },
        },
        "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Failure" } } },
    });

    json!({
        "/healthz": {
            "get": {
                "summary": "Whether the service is up",
                "description": "Answers without a key, so a load balancer can probe it.",
                "security": [],
                "responses": {
                    "200": {
                        "description": "The service is up.",
                        "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Health" } } },
                    },
                },
            },
        },
        "/openapi.json": {
            "get": {
                "summary": "This description",
                "description": "The OpenAPI document for the service, as JSON.",
                "security": [],
                "responses": {
                    "200": { "description": "An OpenAPI 3.0 document." },
                },
            },
        },
        "/api/v1/documents": {
            "post": {
                "summary": "Open an uploaded document",
                "description": "The body is the .ofd file itself. The id it returns owns every later route.",
                "requestBody": {
                    "required": true,
                    "content": { "application/octet-stream": { "schema": { "type": "string", "format": "binary" } } },
                },
                "responses": {
                    "201": {
                        "description": "The document is open.",
                        "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Opened" } } },
                    },
                    "401": unauthorised.clone(),
                    "422": {
                        "description": "The bytes are not an OFD document.",
                        "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Failure" } } },
                    },
                    "429": too_many.clone(),
                    "503": {
                        "description": "The service holds as many documents as it will.",
                        "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Failure" } } },
                    },
                },
            },
        },
        "/api/v1/documents/{id}": {
            "get": {
                "summary": "Report the facts of an open document",
                "parameters": [id()],
                "responses": {
                    "200": {
                        "description": "The document's declaration-level facts.",
                        "content": { "application/json": { "schema": { "$ref": "#/components/schemas/DocumentInfo" } } },
                    },
                    "401": unauthorised.clone(),
                    "404": not_found.clone(),
                    "429": too_many.clone(),
                },
            },
            "delete": {
                "summary": "Release an open document",
                "parameters": [id()],
                "responses": {
                    "204": { "description": "The document is released." },
                    "401": unauthorised.clone(),
                    "404": not_found.clone(),
                    "429": too_many.clone(),
                },
            },
        },
        "/api/v1/documents/{id}/pdf": {
            "get": {
                "summary": "Convert the whole document to PDF",
                "parameters": [id()],
                "responses": {
                    "200": {
                        "description": "A PDF document.",
                        "content": { "application/pdf": { "schema": { "type": "string", "format": "binary" } } },
                    },
                    "401": unauthorised.clone(),
                    "404": not_found.clone(),
                    "429": too_many.clone(),
                },
            },
        },
        "/api/v1/documents/{id}/search": {
            "get": {
                "summary": "Search every page",
                "parameters": [
                    id(),
                    {
                        "name": "needle",
                        "in": "query",
                        "required": true,
                        "schema": { "type": "string" },
                        "description": "Text to look for; its absence is a bad request.",
                    },
                ],
                "responses": {
                    "200": {
                        "description": "The matches, in page order.",
                        "content": {
                            "application/json": {
                                "schema": { "type": "array", "items": { "$ref": "#/components/schemas/SearchHit" } },
                            },
                        },
                    },
                    "400": {
                        "description": "The needle is missing.",
                        "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Failure" } } },
                    },
                    "401": unauthorised.clone(),
                    "404": not_found.clone(),
                    "429": too_many.clone(),
                },
            },
        },
        "/api/v1/documents/{id}/verify": {
            "get": {
                "summary": "Verify every signature",
                "parameters": [id()],
                "responses": {
                    "200": {
                        "description": "One status per signature, in document order.",
                        "content": {
                            "application/json": {
                                "schema": { "type": "array", "items": { "$ref": "#/components/schemas/SignatureStatus" } },
                            },
                        },
                    },
                    "401": unauthorised.clone(),
                    "404": not_found.clone(),
                    "422": {
                        "description": "The document declares no signatures.",
                        "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Failure" } } },
                    },
                    "429": too_many.clone(),
                },
            },
        },
        "/api/v1/documents/{id}/pages/{page}/png": {
            "get": {
                "summary": "Rasterize one page",
                "parameters": [
                    id(),
                    page(),
                    {
                        "name": "dpi",
                        "in": "query",
                        "required": false,
                        "schema": { "type": "number", "default": 96.0, "minimum": 12.0, "maximum": 600.0 },
                        "description": "Rasterization resolution, clamped to a sane range.",
                    },
                ],
                "responses": {
                    "200": {
                        "description": "A PNG image.",
                        "content": { "image/png": { "schema": { "type": "string", "format": "binary" } } },
                    },
                    "401": unauthorised.clone(),
                    "404": not_found.clone(),
                    "429": too_many.clone(),
                },
            },
        },
        "/api/v1/documents/{id}/pages/{page}/svg": {
            "get": {
                "summary": "Convert one page to SVG",
                "parameters": [id(), page()],
                "responses": {
                    "200": {
                        "description": "An SVG document.",
                        "content": { "image/svg+xml": { "schema": { "type": "string", "format": "binary" } } },
                    },
                    "401": unauthorised.clone(),
                    "404": not_found.clone(),
                    "429": too_many.clone(),
                },
            },
        },
    })
}
