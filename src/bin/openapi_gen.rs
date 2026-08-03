//! Generator for the canonical `/api/v2` OpenAPI artifact.
//!
//! Writes exactly what `docs/openapi.yaml` must contain. The bytes come from
//! `conflux::web::openapi::document_yaml`, which a running instance also serves
//! at `/api/v2/openapi.yaml`, so the tracked file and the live contract are the
//! same document by construction rather than by convention.

fn main() {
    print!("{}", conflux::web::openapi::document_yaml());
}
