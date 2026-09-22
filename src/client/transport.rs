use std::fmt;
use std::time::Duration;

/// An HTTP `POST` ready to send.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// Full URL.
    pub url: String,
    /// Header name/value pairs.
    pub headers: Vec<(String, String)>,
    /// Request body (JSON).
    pub body: Vec<u8>,
    /// Whole-request timeout.
    pub timeout: Duration,
}

/// An HTTP response, whatever its status.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// Status code.
    pub status: u16,
    /// Header name/value pairs.
    pub headers: Vec<(String, String)>,
    /// Response body.
    pub body: Vec<u8>,
}

/// The network failed before a response arrived (DNS, connect, TLS, timeout).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportError(pub String);

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TransportError {}

/// Sends one HTTP `POST`.
///
/// Implement this to use your own HTTP stack (or a mock in tests). A non-2xx
/// status is **not** an error here: return it as `Ok(HttpResponse)` and the
/// client will interpret and retry it. Return `Err` only when no response
/// arrived at all.
pub trait Transport: Send + Sync {
    /// Perform the request.
    fn post(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError>;
}

/// Built-in blocking transport backed by [`ureq`] with rustls.
#[cfg(feature = "ureq")]
#[derive(Debug, Clone)]
pub struct UreqTransport {
    agent: ureq::Agent,
}

#[cfg(feature = "ureq")]
impl Default for UreqTransport {
    fn default() -> Self {
        Self {
            agent: ureq::AgentBuilder::new().build(),
        }
    }
}

#[cfg(feature = "ureq")]
impl Transport for UreqTransport {
    fn post(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        use std::io::Read;

        let mut req = self.agent.post(&request.url).timeout(request.timeout);
        for (k, v) in &request.headers {
            req = req.set(k, v);
        }
        let resp = match req.send_bytes(&request.body) {
            Ok(resp) | Err(ureq::Error::Status(_, resp)) => resp,
            Err(ureq::Error::Transport(t)) => return Err(TransportError(t.to_string())),
        };
        let status = resp.status();
        let headers = resp
            .headers_names()
            .into_iter()
            .filter_map(|name| resp.header(&name).map(|v| (name.clone(), v.to_string())))
            .collect();
        let mut body = Vec::new();
        resp.into_reader()
            .take(16 * 1024 * 1024)
            .read_to_end(&mut body)
            .map_err(|e| TransportError(e.to_string()))?;
        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }
}
