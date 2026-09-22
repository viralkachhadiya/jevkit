//! A blocking client for the System One API, with sans-IO request and
//! response types.
//!
//! Enabled by the `client` feature (on by default). The built-in HTTP
//! transport needs the `ureq` feature (also on by default).

mod blocking;
mod error;
mod request;
mod response;
mod transport;

pub use blocking::{Client, ClientBuilder, RetryPolicy, DEFAULT_BASE_URL, DEFAULT_MODEL};
pub use error::{ApiError, Error};
pub use request::Request;
pub use response::{Answer, ChoiceAnswer, NoulAnswer, Response, ScoreAnswer, Usage};
#[cfg(feature = "ureq")]
pub use transport::UreqTransport;
pub use transport::{HttpRequest, HttpResponse, Transport, TransportError};
