use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::error::ApiError;
use super::transport::{HttpRequest, Transport};
use super::{Error, Request, Response};

/// Default API base URL.
pub const DEFAULT_BASE_URL: &str = "https://api.typesafe.ai";
/// Default model alias.
pub const DEFAULT_MODEL: &str = "jev-latest";

const ENV_API_KEY: &str = "TYPESAFE_API_KEY";
const ENV_BASE_URL: &str = "TYPESAFE_BASE_URL";
const ENV_MODEL: &str = "TYPESAFE_DEFAULT_MODEL";

/// How failed calls are retried.
///
/// Retried: network failures and the statuses 429, 502, 503, 504 and 529.
/// Evaluations have no side effects, so retrying a `POST` is safe. If the
/// server sends `Retry-After` and it exceeds `max_delay`, the error is
/// returned immediately instead of waiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Retries after the first attempt. `0` disables retrying.
    pub max_retries: u32,
    /// Delay before the first retry; doubles each time.
    pub base_delay: Duration,
    /// Upper bound for any single wait.
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(8),
        }
    }
}

impl RetryPolicy {
    /// Never retry.
    pub fn none() -> Self {
        Self {
            max_retries: 0,
            ..Self::default()
        }
    }

    /// Exponential delay for the given zero-based retry, with +/-25% jitter
    /// (`jitter` in `0.0..=1.0`), capped at `max_delay`.
    pub fn delay(&self, retry: u32, jitter: f64) -> Duration {
        let exp = self
            .base_delay
            .saturating_mul(1u32.checked_shl(retry).unwrap_or(u32::MAX));
        let factor = 0.75 + 0.5 * jitter.clamp(0.0, 1.0);
        exp.mul_f64(factor).min(self.max_delay)
    }
}

fn jitter() -> f64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    f64::from(nanos % 1000) / 1000.0
}

/// A blocking System One client. Cheap to clone; share one across threads.
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

struct Inner {
    api_key: String,
    base_url: String,
    model: String,
    timeout: Duration,
    retry: RetryPolicy,
    transport: Box<dyn Transport>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.inner.base_url)
            .field("model", &self.inner.model)
            .field("timeout", &self.inner.timeout)
            .field("retry", &self.inner.retry)
            .field("api_key", &"<redacted>")
            .finish()
    }
}

impl Client {
    /// Start configuring a client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// A client with defaults and the given API key.
    pub fn new(api_key: impl Into<String>) -> Result<Self, Error> {
        Self::builder().api_key(api_key).build()
    }

    /// A client configured from `TYPESAFE_API_KEY`, and optionally
    /// `TYPESAFE_BASE_URL` and `TYPESAFE_DEFAULT_MODEL`.
    pub fn from_env() -> Result<Self, Error> {
        Self::builder().from_env().build()
    }

    /// The model used when a request does not set one.
    pub fn default_model(&self) -> &str {
        &self.inner.model
    }

    /// Send a request and parse the response, retrying per the [`RetryPolicy`].
    pub fn send(&self, request: &Request) -> Result<Response, Error> {
        let inner = &self.inner;
        let http = HttpRequest {
            url: format!("{}/v1/systemone", inner.base_url.trim_end_matches('/')),
            headers: vec![
                ("Authorization".into(), format!("Bearer {}", inner.api_key)),
                ("Content-Type".into(), "application/json".into()),
                ("Accept".into(), "application/json".into()),
                (
                    "User-Agent".into(),
                    concat!("jevkit/", env!("CARGO_PKG_VERSION")).into(),
                ),
            ],
            body: request.to_json(&inner.model)?,
            timeout: inner.timeout,
        };

        let mut retry = 0u32;
        loop {
            let wait = match inner.transport.post(&http) {
                Ok(resp) if (200..300).contains(&resp.status) => {
                    return Response::from_slice(&resp.body);
                }
                Ok(resp) => {
                    let err = ApiError::from_parts(resp.status, &resp.headers, &resp.body);
                    if !err.is_retryable() || retry >= inner.retry.max_retries {
                        return Err(Error::Api(err));
                    }
                    let wait = err
                        .retry_after
                        .unwrap_or_else(|| inner.retry.delay(retry, jitter()));
                    if wait > inner.retry.max_delay {
                        return Err(Error::Api(err));
                    }
                    wait
                }
                Err(e) => {
                    if retry >= inner.retry.max_retries {
                        return Err(Error::Transport(e.0));
                    }
                    inner.retry.delay(retry, jitter())
                }
            };
            retry += 1;
            std::thread::sleep(wait);
        }
    }
}

/// Builder for [`Client`].
#[derive(Default)]
pub struct ClientBuilder {
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    timeout: Option<Duration>,
    retry: Option<RetryPolicy>,
    transport: Option<Box<dyn Transport>>,
}

impl ClientBuilder {
    /// Fill unset options from `TYPESAFE_API_KEY`, `TYPESAFE_BASE_URL` and
    /// `TYPESAFE_DEFAULT_MODEL`. Explicit setters called earlier win.
    pub fn from_env(mut self) -> Self {
        let get = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        self.api_key = self.api_key.or_else(|| get(ENV_API_KEY));
        self.base_url = self.base_url.or_else(|| get(ENV_BASE_URL));
        self.model = self.model.or_else(|| get(ENV_MODEL));
        self
    }

    /// Set the API key.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set the base URL (a gateway or test server), without `/v1/systemone`.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Set the default model (default `jev-latest`).
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Whole-request timeout (default 30 s).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Retry behaviour (default: 2 retries, exponential backoff).
    pub fn retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = Some(policy);
        self
    }

    /// Use your own HTTP stack instead of the built-in one.
    pub fn transport(mut self, transport: impl Transport + 'static) -> Self {
        self.transport = Some(Box::new(transport));
        self
    }

    /// Build the client.
    pub fn build(self) -> Result<Client, Error> {
        let api_key = self
            .api_key
            .filter(|k| !k.trim().is_empty())
            .ok_or(Error::MissingApiKey)?;
        let transport = match self.transport {
            Some(t) => t,
            None => default_transport()?,
        };
        Ok(Client {
            inner: Arc::new(Inner {
                api_key,
                base_url: self.base_url.unwrap_or_else(|| DEFAULT_BASE_URL.into()),
                model: self.model.unwrap_or_else(|| DEFAULT_MODEL.into()),
                timeout: self.timeout.unwrap_or(Duration::from_secs(30)),
                retry: self.retry.unwrap_or_default(),
                transport,
            }),
        })
    }
}

#[cfg(feature = "ureq")]
fn default_transport() -> Result<Box<dyn Transport>, Error> {
    Ok(Box::new(super::transport::UreqTransport::default()))
}

#[cfg(not(feature = "ureq"))]
fn default_transport() -> Result<Box<dyn Transport>, Error> {
    Err(Error::Config(
        "no transport: enable the `ureq` feature or call ClientBuilder::transport".into(),
    ))
}
