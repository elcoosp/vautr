//! Server middleware stack. arch-design §3.3: tracing, rate limiting, auth
//! extractor, CORS. Rate values per NFR-SEC-04 (cloud defaults; self-host TBD-5).
//!
//! Components:
//! - Request tracing via `tower_http::trace::TraceLayer` (composed in `main`,
//!   never logs bodies, per server-scaling.md §3 no-plaintext rule).
//! - `rate_limiter()` — an in-memory fixed-window limiter keyed by client IP
//!   (or `X-Forwarded-For`). Redis-backed sliding windows are a later
//!   refinement (VTR-046); this bounds CPU-expensive OPAQUE and I/O-heavy sync
//!   endpoints. Returns `429 rate_limited` with `Retry-After` when exceeded.
//! - `cors()` — permissive CORS for the client core.

use std::convert::Infallible;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, Response, StatusCode, header};
use tower::{Layer, Service};
use tower_http::cors::CorsLayer;

/// Permissive CORS (client core is a native/extension app; tightened in prod).
pub fn cors() -> CorsLayer {
    CorsLayer::permissive()
}

/// Build the rate-limiter layer from env-configurable bounds.
///
/// Env: `VAUTR_RATE_LIMIT_MAX` (default 100) and
/// `VAUTR_RATE_LIMIT_WINDOW_SECS` (default 60). A single global per-IP limit is
/// applied; per-route limits (auth 5/min, sync-pull 30/min, push-batch 10/min)
/// are a Redis-backed follow-up (VTR-046).
pub fn rate_limiter() -> RateLimitLayer {
    let max = std::env::var("VAUTR_RATE_LIMIT_MAX")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);
    let window = std::env::var("VAUTR_RATE_LIMIT_WINDOW_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    RateLimitLayer::new(max, window)
}

/// The CORS layer (kept for backward compatibility with the original API).
pub fn layer() -> CorsLayer {
    cors()
}

// ---------------------------------------------------------------------------
// Fixed-window rate limiter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Window {
    start_secs: u64,
    count: u64,
}

#[derive(Default)]
struct RateState {
    windows: HashMap<String, Window>,
}

/// Shared, thread-safe fixed-window counter keyed by client identity.
#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<Mutex<RateState>>,
    max_per_window: u64,
    window_secs: u64,
}

impl RateLimiter {
    pub fn new(max_per_window: u64, window_secs: u64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(RateState::default())),
            max_per_window,
            window_secs: window_secs.max(1),
        }
    }

    /// Seconds since the Unix epoch (injectable for tests via `check_at`).
    pub fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Check a request against the window. `Ok(info)` if allowed; `Err(limited)`
    /// carries the `Retry-After` seconds if the limit was exceeded.
    pub fn check(&self, key: &str) -> Result<RateInfo, RateLimited> {
        self.check_at(key, Self::now_secs())
    }

    fn check_at(&self, key: &str, now_secs: u64) -> Result<RateInfo, RateLimited> {
        let mut state = self.inner.lock().unwrap();
        let window_start = now_secs - (now_secs % self.window_secs);

        // Opportunistically bound the map so a burst of distinct IPs can't grow
        // it unboundedly.
        if state.windows.len() > 4096 {
            state
                .windows
                .retain(|_, w| w.start_secs + self.window_secs >= now_secs);
        }

        let w = state
            .windows
            .entry(key.to_string())
            .or_insert(Window { start_secs: window_start, count: 0 });
        if w.start_secs != window_start {
            w.start_secs = window_start;
            w.count = 0;
        }
        w.count += 1;

        let reset_unix_secs = window_start + self.window_secs;
        if w.count > self.max_per_window {
            Err(RateLimited {
                retry_after_secs: reset_unix_secs.saturating_sub(now_secs).max(1),
            })
        } else {
            Ok(RateInfo {
                limit: self.max_per_window,
                remaining: self.max_per_window - w.count,
                reset_unix_secs,
            })
        }
    }
}

/// Outcome of an allowed request.
#[derive(Debug, Clone, Copy)]
pub struct RateInfo {
    pub limit: u64,
    pub remaining: u64,
    pub reset_unix_secs: u64,
}

/// The request was rate-limited.
#[derive(Debug, Clone, Copy)]
pub struct RateLimited {
    pub retry_after_secs: u64,
}

/// `tower::Layer` wrapping a service with rate limiting.
#[derive(Clone)]
pub struct RateLimitLayer {
    limiter: RateLimiter,
}

impl RateLimitLayer {
    pub fn new(max_per_window: u64, window_secs: u64) -> Self {
        Self {
            limiter: RateLimiter::new(max_per_window, window_secs),
        }
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitService {
            inner,
            limiter: self.limiter.clone(),
        }
    }
}

/// A rate-limiting service that returns `429` once a client exceeds its window.
#[derive(Clone)]
pub struct RateLimitService<S> {
    inner: S,
    limiter: RateLimiter,
}

fn client_key(req: &Request<Body>) -> String {
    if let Some(ci) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
        return ci.0.ip().to_string();
    }
    if let Some(xff) = req.headers().get("x-forwarded-for") {
        if let Ok(s) = xff.to_str() {
            if let Some(ip) = s.split(',').next().map(str::trim).filter(|ip| !ip.is_empty()) {
                return ip.to_string();
            }
        }
    }
    "unknown".to_string()
}

fn rate_limited_response(limited: &RateLimited) -> Response<Body> {
    let body = serde_json::json!({
        "error": "rate_limited",
        "message": "Too many requests. Please retry after the Retry-After window.",
    });
    Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .header(header::RETRY_AFTER, limited.retry_after_secs.to_string())
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap_or_else(|_| Response::new(Body::from(body.to_string())))
}

impl<S> Service<Request<Body>> for RateLimitService<S>
where
    S: Service<Request<Body>, Response = Response<Body>, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let key = client_key(&req);
        match self.limiter.check(&key) {
            Ok(info) => {
                let mut inner = self.inner.clone();
                Box::pin(async move {
                    let mut resp = inner
                        .call(req)
                        .await
                        .unwrap_or_else(|never| match never {});
                    let headers = resp.headers_mut();
                    headers.insert(
                        "x-ratelimit-limit",
                        info.limit.to_string().parse().unwrap(),
                    );
                    headers.insert(
                        "x-ratelimit-remaining",
                        info.remaining.to_string().parse().unwrap(),
                    );
                    headers.insert(
                        "x-ratelimit-reset",
                        info.reset_unix_secs.to_string().parse().unwrap(),
                    );
                    Ok(resp)
                })
            }
            Err(limited) => {
                tracing::warn!(
                    retry_after_secs = limited.retry_after_secs,
                    client = %key,
                    "request rate limited"
                );
                Box::pin(async move { Ok(rate_limited_response(&limited)) })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    fn req(ip: &str) -> Request<Body> {
        Request::builder()
            .uri("/")
            .header("x-forwarded-for", ip)
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn rate_limiter_returns_429_after_limit() {
        let app = Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(RateLimitLayer::new(2, 60));

        for i in 0..2 {
            let resp = app
                .clone()
                .oneshot(req("1.2.3.4"))
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK, "request {i} should pass");
        }
        let resp = app.clone().oneshot(req("1.2.3.4")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(resp.headers().contains_key("retry-after"));
        // 429 body is the api.md error envelope.
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"], "rate_limited");
    }

    #[tokio::test]
    async fn rate_limiter_is_per_client() {
        let app = Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(RateLimitLayer::new(2, 60));

        // Client A exhausts its budget.
        app.clone().oneshot(req("10.0.0.1")).await.unwrap();
        app.clone().oneshot(req("10.0.0.1")).await.unwrap();
        // Client B is unaffected.
        let resp = app.clone().oneshot(req("10.0.0.2")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
