use std::fmt;
use std::time::Duration;

use crate::DecodeError;

/// Everything that can go wrong building, sending or reading a request.
#[derive(Debug)]
pub enum Error {
    /// No API key: set `TYPESAFE_API_KEY` or call `ClientBuilder::api_key`.
    MissingApiKey,
    /// The client is misconfigured (for example no transport is available).
    Config(String),
    /// The request is invalid; nothing was sent.
    InvalidRequest(String),
    /// The network call failed (after retries).
    Transport(String),
    /// The API answered with a non-2xx status (after retries).
    Api(ApiError),
    /// The state could not be serialized, or the response was not valid JSON
    /// of the expected shape.
    Json(serde_json::Error),
    /// The API returned no answer for this question. It does not promise one
    /// for every question.
    MissingAnswer {
        /// The question id.
        id: String,
    },
    /// The answer for this id has a different type than the question asked.
    WrongAnswerType {
        /// The question id.
        id: String,
        /// Type you asked for.
        expected: &'static str,
        /// Type the API returned.
        found: &'static str,
    },
    /// The answer could not be mapped onto your Rust type, for example the
    /// API returned a label your enum does not have.
    Decode {
        /// The question id.
        id: String,
        /// What went wrong.
        source: DecodeError,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey => f.write_str("missing API key (set TYPESAFE_API_KEY)"),
            Self::Config(m) => write!(f, "invalid client configuration: {m}"),
            Self::InvalidRequest(m) => write!(f, "invalid request: {m}"),
            Self::Transport(m) => write!(f, "transport error: {m}"),
            Self::Api(e) => e.fmt(f),
            Self::Json(e) => write!(f, "json error: {e}"),
            Self::MissingAnswer { id } => write!(f, "no answer returned for question `{id}`"),
            Self::WrongAnswerType {
                id,
                expected,
                found,
            } => write!(
                f,
                "question `{id}`: expected a {expected} answer, got {found}"
            ),
            Self::Decode { id, source } => write!(f, "question `{id}`: {source}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(e) => Some(e),
            Self::Decode { source, .. } => Some(source),
            Self::Api(e) => Some(e),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

/// A non-2xx API response.
#[derive(Debug, Clone, PartialEq)]
pub struct ApiError {
    /// HTTP status code.
    pub status: u16,
    /// The `error_type` field of the JSON body, if present.
    pub error_type: Option<String>,
    /// The `message` of the JSON body, or the (truncated) raw body.
    pub message: String,
    /// The `Retry-After` header, when given in seconds.
    pub retry_after: Option<Duration>,
}

impl ApiError {
    /// `true` for statuses worth retrying: 429, 529, 502, 503 and 504.
    pub fn is_retryable(&self) -> bool {
        matches!(self.status, 429 | 529 | 502 | 503 | 504)
    }

    pub(crate) fn from_parts(status: u16, headers: &[(String, String)], body: &[u8]) -> Self {
        let retry_after = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("retry-after"))
            .and_then(|(_, v)| v.trim().parse::<u64>().ok())
            .map(Duration::from_secs);

        let parsed: Option<serde_json::Value> = serde_json::from_slice(body).ok();
        let field = |name: &str| {
            parsed.as_ref().and_then(|v| v.get(name)).map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
        };
        let message = field("message")
            .or_else(|| field("detail"))
            .unwrap_or_else(|| {
                let text = String::from_utf8_lossy(body);
                text.chars().take(300).collect()
            });
        Self {
            status,
            error_type: field("error_type"),
            message,
            retry_after,
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "API error {}", self.status)?;
        if let Some(t) = &self.error_type {
            write!(f, " ({t})")?;
        }
        if !self.message.is_empty() {
            write!(f, ": {}", self.message)?;
        }
        Ok(())
    }
}

impl std::error::Error for ApiError {}
