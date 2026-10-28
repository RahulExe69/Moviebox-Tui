//! Loopback-only HTTP transport for the mobile provider facade.
//!
//! The caller must provide a cryptographically random per-process token of at
//! least 32 characters and pass it to the frontend through the native bridge.
//! This server refuses non-loopback bind addresses and does not log that token.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::Incoming;
use hyper::header::{
    ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN,
    CACHE_CONTROL, CONTENT_TYPE, ORIGIN, VARY,
};
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

use crate::mobile_api::{MOBILE_API_VERSION, MobileApi, MobileApiError};
use crate::providers::models::ProviderError;

const TOKEN_HEADER: &str = "x-7xcinema-token";
const MAX_REQUEST_BODY_BYTES: usize = 64 * 1024;
const MAX_CONNECTIONS: usize = 24;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Error)]
pub enum MobileServerError {
    #[error("The mobile API must bind to a loopback address")]
    NonLoopbackAddress,
    #[error("The mobile API token must be at least 32 characters")]
    InvalidToken,
    #[error("Failed to bind the local API: {0}")]
    Io(#[from] std::io::Error),
}

/// Handle for a running mobile API. Dropping the handle stops the accept loop.
pub struct MobileServerHandle {
    local_addr: SocketAddr,
    task: tokio::task::JoinHandle<()>,
}

impl MobileServerHandle {
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn stop(&self) {
        self.task.abort();
    }
}

impl Drop for MobileServerHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Start the HTTP API on a loopback-only address.
///
/// Pass port 0 to let the OS select an available port, then read it from
/// `MobileServerHandle::local_addr`. The caller is responsible for generating
/// the token securely and sharing it with the bundled frontend via a native
/// bridge, never via a URL or log message.
pub async fn start_loopback(
    address: SocketAddr,
    api: MobileApi,
    token: String,
) -> Result<MobileServerHandle, MobileServerError> {
    validate_server_config(address, &token)?;
    let listener = TcpListener::bind(address).await?;
    let local_addr = listener.local_addr()?;
    let semaphore = Arc::new(Semaphore::new(MAX_CONNECTIONS));

    let task = tokio::spawn(async move {
        loop {
            let (stream, peer) = match listener.accept().await {
                Ok(pair) => pair,
                Err(error) => {
                    log::warn!("mobile API accept failed: {}", error.kind());
                    continue;
                }
            };

            // Defense in depth: the listener is loopback-bound, and accepted
            // peers must also be loopback addresses.
            if !peer.ip().is_loopback() {
                continue;
            }

            let permit = match Arc::clone(&semaphore).try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => continue,
            };
            let api = api.clone();
            let token = token.clone();

            tokio::spawn(async move {
                let _permit = permit;
                let io = TokioIo::new(stream);
                let service = service_fn(move |request| {
                    let api = api.clone();
                    let token = token.clone();
                    async move { Ok::<_, Infallible>(handle_request(request, api, token).await) }
                });
                let builder = hyper::server::conn::http1::Builder::new();
                let connection = builder.serve_connection(io, service);
                let _ = tokio::time::timeout(CONNECTION_TIMEOUT, connection).await;
            });
        }
    });

    Ok(MobileServerHandle { local_addr, task })
}

fn validate_server_config(address: SocketAddr, token: &str) -> Result<(), MobileServerError> {
    if !address.ip().is_loopback() {
        return Err(MobileServerError::NonLoopbackAddress);
    }
    if token.chars().count() < 32 {
        return Err(MobileServerError::InvalidToken);
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct SearchRequest {
    provider: String,
    query: String,
    #[serde(default)]
    page: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct DetailsRequest {
    provider: String,
    media_id: String,
}

#[derive(Debug, Deserialize)]
struct PlaybackRequest {
    provider: String,
    media_id: String,
    #[serde(default)]
    season: Option<usize>,
    #[serde(default)]
    episode: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ResolvePlaybackRequest {
    provider: String,
    media_id: String,
    #[serde(default)]
    season: Option<usize>,
    #[serde(default)]
    episode: Option<usize>,
    release_index: usize,
    #[serde(default)]
    subtitle_index: Option<usize>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
    api_version: u16,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

async fn handle_request(
    request: Request<Incoming>,
    api: MobileApi,
    token: String,
) -> Response<Full<Bytes>> {
    let origin = request
        .headers()
        .get(ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);

    if origin
        .as_deref()
        .is_some_and(|value| !is_allowed_origin(value))
    {
        return error_response(StatusCode::FORBIDDEN, "Origin not allowed", None);
    }

    if request.method() == Method::OPTIONS {
        return match origin.as_deref() {
            Some(value) if is_allowed_origin(value) => {
                empty_response(StatusCode::NO_CONTENT, Some(value))
            }
            _ => error_response(StatusCode::FORBIDDEN, "Origin not allowed", None),
        };
    }

    let supplied_token = request
        .headers()
        .get(TOKEN_HEADER)
        .map(|value| value.as_bytes());
    if !supplied_token.is_some_and(|value| value == token.as_bytes()) {
        return error_response(StatusCode::UNAUTHORIZED, "Unauthorized", origin.as_deref());
    }

    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let query = request.uri().query().unwrap_or_default().to_owned();

    match (method, path.as_str()) {
        (Method::GET, "/api/v1/health") => json_response(
            StatusCode::OK,
            &HealthResponse {
                ok: true,
                api_version: MOBILE_API_VERSION,
            },
            origin.as_deref(),
        ),
        (Method::GET, "/api/v1/providers") => {
            json_response(StatusCode::OK, &api.providers(), origin.as_deref())
        }
        (Method::GET, "/api/v1/homepage") => {
            let tab = query_value(&query, "tab").unwrap_or_default();
            let page = query_value(&query, "page")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(1)
                .clamp(1, 1000);
            match api.homepage(&tab, page).await {
                Ok(homepage) => json_response(StatusCode::OK, &homepage, origin.as_deref()),
                Err(error) => api_error_response(error, origin.as_deref()),
            }
        }
        (Method::POST, "/api/v1/search") => {
            let payload = match read_json::<SearchRequest>(request.into_body()).await {
                Ok(payload) => payload,
                Err(()) => {
                    return error_response(
                        StatusCode::BAD_REQUEST,
                        "Invalid JSON request",
                        origin.as_deref(),
                    );
                }
            };
            match api
                .search_catalog(&payload.provider, &payload.query, payload.page.unwrap_or(1))
                .await
            {
                Ok(items) => json_response(StatusCode::OK, &items, origin.as_deref()),
                Err(error) => api_error_response(error, origin.as_deref()),
            }
        }
        (Method::POST, "/api/v1/details") => {
            let payload = match read_json::<DetailsRequest>(request.into_body()).await {
                Ok(payload) => payload,
                Err(()) => {
                    return error_response(
                        StatusCode::BAD_REQUEST,
                        "Invalid JSON request",
                        origin.as_deref(),
                    );
                }
            };
            match api.details(&payload.provider, &payload.media_id).await {
                Ok(details) => json_response(StatusCode::OK, &details, origin.as_deref()),
                Err(error) => api_error_response(error, origin.as_deref()),
            }
        }
        (Method::POST, "/api/v1/playback/sources") => {
            let payload = match read_json::<PlaybackRequest>(request.into_body()).await {
                Ok(payload) => payload,
                Err(()) => {
                    return error_response(
                        StatusCode::BAD_REQUEST,
                        "Invalid JSON request",
                        origin.as_deref(),
                    );
                }
            };
            match api
                .playback_sources(
                    &payload.provider,
                    &payload.media_id,
                    payload.season.unwrap_or(0),
                    payload.episode.unwrap_or(0),
                )
                .await
            {
                Ok(sources) => json_response(StatusCode::OK, &sources, origin.as_deref()),
                Err(error) => api_error_response(error, origin.as_deref()),
            }
        }
        (Method::POST, "/api/v1/playback/resolve") => {
            let payload = match read_json::<ResolvePlaybackRequest>(request.into_body()).await {
                Ok(payload) => payload,
                Err(()) => {
                    return error_response(
                        StatusCode::BAD_REQUEST,
                        "Invalid JSON request",
                        origin.as_deref(),
                    );
                }
            };
            match api
                .resolve_playback(
                    &payload.provider,
                    &payload.media_id,
                    payload.season.unwrap_or(0),
                    payload.episode.unwrap_or(0),
                    payload.release_index,
                    payload.subtitle_index,
                )
                .await
            {
                Ok(source) => json_response(StatusCode::OK, &source, origin.as_deref()),
                Err(error) => api_error_response(error, origin.as_deref()),
            }
        }
        _ => error_response(StatusCode::NOT_FOUND, "Route not found", origin.as_deref()),
    }
}

async fn read_json<T: DeserializeOwned>(body: Incoming) -> Result<T, ()> {
    let collected = Limited::new(body, MAX_REQUEST_BODY_BYTES)
        .collect()
        .await
        .map_err(|_| ())?;
    serde_json::from_slice(&collected.to_bytes()).map_err(|_| ())
}

fn api_error_response(error: MobileApiError, origin: Option<&str>) -> Response<Full<Bytes>> {
    let (status, message) = match error {
        MobileApiError::PlaybackProvider { provider, error } => {
            let status = match error {
                ProviderError::NotFound => StatusCode::NOT_FOUND,
                ProviderError::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
                _ => StatusCode::BAD_GATEWAY,
            };
            return error_response(status, error.user_message(provider), origin);
        }
        MobileApiError::UnknownProvider(_) => (StatusCode::BAD_REQUEST, "Unknown provider"),
        MobileApiError::EmptyQuery => (StatusCode::BAD_REQUEST, "Search query must not be empty"),
        MobileApiError::EmptyMediaId => (StatusCode::BAD_REQUEST, "Media ID must not be empty"),
        MobileApiError::EmptyTabId => {
            (StatusCode::BAD_REQUEST, "Homepage tab ID must not be empty")
        }
        MobileApiError::InvalidEpisodeSelection => (
            StatusCode::BAD_REQUEST,
            "Choose either a movie or a specific season and episode",
        ),
        MobileApiError::InvalidReleaseIndex => (
            StatusCode::BAD_REQUEST,
            "Playback release index is out of range",
        ),
        MobileApiError::InvalidSubtitleIndex => {
            (StatusCode::BAD_REQUEST, "Subtitle index is out of range")
        }
        MobileApiError::Provider(ProviderError::NotFound) => {
            (StatusCode::NOT_FOUND, "No results found")
        }
        MobileApiError::Provider(ProviderError::RateLimited(_)) => (
            StatusCode::TOO_MANY_REQUESTS,
            "Provider rate limited the request",
        ),
        MobileApiError::Provider(ProviderError::Network(_)) => (
            StatusCode::BAD_GATEWAY,
            "Provider network request failed. Check your connection and retry.",
        ),
        MobileApiError::Provider(ProviderError::Parsing(_)) => (
            StatusCode::BAD_GATEWAY,
            "Provider returned an unexpected response. The source integration may need updating.",
        ),
        MobileApiError::Provider(ProviderError::Unavailable(_)) => (
            StatusCode::BAD_GATEWAY,
            "Provider could not supply playback data for this title or episode.",
        ),
        MobileApiError::Service(_) => (StatusCode::BAD_GATEWAY, "Homepage request failed"),
    };
    error_response(status, message, origin)
}

fn json_response<T: Serialize>(
    status: StatusCode,
    payload: &T,
    origin: Option<&str>,
) -> Response<Full<Bytes>> {
    match serde_json::to_vec(payload) {
        Ok(bytes) => response(
            status,
            Bytes::from(bytes),
            Some("application/json; charset=utf-8"),
            origin,
        ),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Response serialization failed",
            origin,
        ),
    }
}

fn error_response(
    status: StatusCode,
    message: impl Into<String>,
    origin: Option<&str>,
) -> Response<Full<Bytes>> {
    json_response(
        status,
        &ErrorResponse {
            error: message.into(),
        },
        origin,
    )
}

fn empty_response(status: StatusCode, origin: Option<&str>) -> Response<Full<Bytes>> {
    response(status, Bytes::new(), None, origin)
}

fn response(
    status: StatusCode,
    body: Bytes,
    content_type: Option<&'static str>,
    origin: Option<&str>,
) -> Response<Full<Bytes>> {
    let mut builder = Response::builder()
        .status(status)
        .header(CACHE_CONTROL, "no-store")
        .header(VARY, "Origin");

    if let Some(content_type) = content_type {
        builder = builder.header(CONTENT_TYPE, content_type);
    }
    if let Some(origin) = origin {
        if let Ok(value) = hyper::header::HeaderValue::from_str(origin) {
            builder = builder.header(ACCESS_CONTROL_ALLOW_ORIGIN, value);
        }
        builder = builder
            .header(ACCESS_CONTROL_ALLOW_METHODS, "GET, POST, OPTIONS")
            .header(
                ACCESS_CONTROL_ALLOW_HEADERS,
                "Content-Type, X-7xCinema-Token",
            );
    }

    builder
        .body(Full::new(body))
        .unwrap_or_else(|_| Response::new(Full::new(Bytes::new())))
}

fn is_allowed_origin(origin: &str) -> bool {
    matches!(
        origin,
        "capacitor://localhost"
            | "http://localhost"
            | "https://localhost"
            | "http://localhost:5173"
            | "http://127.0.0.1:5173"
    )
}

fn query_value(query: &str, key: &str) -> Option<String> {
    url::form_urlencoded::parse(query.as_bytes())
        .find_map(|(name, value)| (name == key).then(|| value.into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_loopback_bind_addresses() {
        let address = SocketAddr::from(([0, 0, 0, 0], 8080));
        let token = "a".repeat(32);
        assert!(matches!(
            validate_server_config(address, &token),
            Err(MobileServerError::NonLoopbackAddress)
        ));
    }

    #[test]
    fn rejects_short_tokens() {
        let address = SocketAddr::from(([127, 0, 0, 1], 0));
        assert!(matches!(
            validate_server_config(address, "too-short"),
            Err(MobileServerError::InvalidToken)
        ));
    }

    #[test]
    fn accepts_loopback_with_long_token() {
        let address = SocketAddr::from(([127, 0, 0, 1], 0));
        assert!(validate_server_config(address, &"x".repeat(32)).is_ok());
    }

    #[test]
    fn accepts_only_expected_webview_origins() {
        assert!(is_allowed_origin("https://localhost"));
        assert!(is_allowed_origin("capacitor://localhost"));
        assert!(!is_allowed_origin("https://example.com"));
        assert!(!is_allowed_origin("http://localhost.evil.example"));
    }

    #[test]
    fn decodes_query_parameters() {
        assert_eq!(
            query_value("tab=home&page=2", "tab").as_deref(),
            Some("home")
        );
        assert_eq!(query_value("tab=home&page=2", "page").as_deref(), Some("2"));
        assert_eq!(query_value("tab=home&page=2", "missing"), None);
    }
}
