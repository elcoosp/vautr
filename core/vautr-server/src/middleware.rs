//! Server middleware stack. arch-design §3.3: tracing, rate limiting, auth
//! extractor, CORS. Rate values per NFR-SEC-04 (cloud defaults; self-host TBD-5).

#![allow(unused)]

/// Assemble the layered middleware. Returns the base CORS layer; the full
/// stack (tracing, rate limiting, auth extractor) is composed in `main`/`router`.
pub fn layer() -> tower_http::cors::CorsLayer {
    tower_http::cors::CorsLayer::permissive()
}
