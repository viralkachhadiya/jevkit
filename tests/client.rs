use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use jevkit::client::{HttpRequest, HttpResponse, Transport, TransportError};
use jevkit::{Client, Error, Request, Response, RetryPolicy};
use serde_json::{json, Value};

jevkit::choice! {
    /// Which team should handle this?
    #[jev(name = "department")]
    enum Department {
        /// Payments, invoicing, refunds
        Billing = "billing",
        /// Bugs, outages, integrations
        Technical = "technical",
        /// Pricing, upgrades, new accounts
        Sales = "sales",
    }
}

jevkit::score! {
    /// How frustrated is the customer?
    #[jev(name = "frustration")]
    enum Frustration {
        Calm,
        Frustrated,
        Angry = "Very angry",
    }
}

jevkit::noul! {
    /// Does this convey urgency?
    #[jev(name = "is_urgent")]
    enum Urgent {
        /// Explicitly time-sensitive
        yes Yes,
        /// No urgency expressed
        no No,
    }
}

// ---- mock transport --------------------------------------------------------

#[derive(Default)]
struct Mock {
    calls: Mutex<Vec<HttpRequest>>,
    script: Mutex<VecDeque<Result<HttpResponse, TransportError>>>,
}

struct Shared(Arc<Mock>);

impl Transport for Shared {
    fn post(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.0.calls.lock().unwrap().push(request.clone());
        self.0
            .script
            .lock()
            .unwrap()
            .pop_front()
            .expect("mock script exhausted")
    }
}

fn reply(
    status: u16,
    headers: &[(&str, &str)],
    body: &str,
) -> Result<HttpResponse, TransportError> {
    Ok(HttpResponse {
        status,
        headers: headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        body: body.as_bytes().to_vec(),
    })
}

fn fast_retry(max_retries: u32) -> RetryPolicy {
    RetryPolicy {
        max_retries,
        base_delay: Duration::ZERO,
        max_delay: Duration::from_secs(1),
    }
}

fn client_with(
    script: Vec<Result<HttpResponse, TransportError>>,
    retries: u32,
) -> (Client, Arc<Mock>) {
    let mock = Arc::new(Mock {
        script: Mutex::new(script.into()),
        ..Default::default()
    });
    let client = Client::builder()
        .api_key("sk-secret")
        .transport(Shared(mock.clone()))
        .retry(fast_retry(retries))
        .build()
        .unwrap();
    (client, mock)
}

const FULL: &str = r#"{
  "model": "jev-1.13.0",
  "answers": {
    "department": {"type":"choice","choice":"billing",
      "probabilities":{"billing":0.88,"technical":0.12,"sales":0.0},"confidence":0.81},
    "frustration": {"type":"score","score":1.05,
      "legend":{"0":"Calm","1":"Frustrated","2":"Very angry"},
      "probabilities":{"0":0.0,"1":0.95,"2":0.05},"confidence":0.92},
    "is_urgent": {"type":"noul","noul":0.95}
  },
  "usage": {"input_tokens": 318, "output_tokens": 34}
}"#;

fn full_request() -> Request {
    Request::new("Help! My payouts have been failing for 3 days.")
        .choice::<Department>()
        .score::<Frustration>()
        .noul::<Urgent>()
}

// ---- request body ----------------------------------------------------------

#[test]
fn request_body_matches_the_documented_wire_format() {
    let body = full_request().to_json("jev-latest").unwrap();
    let got: Value = serde_json::from_slice(&body).unwrap();
    let want = json!({
        "state": "Help! My payouts have been failing for 3 days.",
        "model": "jev-latest",
        "questions": {
            "department": {
                "type": "choice",
                "instructions": "Which team should handle this?",
                "criteria": {
                    "billing": "Payments, invoicing, refunds",
                    "technical": "Bugs, outages, integrations",
                    "sales": "Pricing, upgrades, new accounts"
                }
            },
            "frustration": {
                "type": "score",
                "instructions": "How frustrated is the customer?",
                "criteria": ["Calm", "Frustrated", "Very angry"]
            },
            "is_urgent": {
                "type": "noul",
                "instructions": "Does this convey urgency?",
                "criteria": {"true": "Explicitly time-sensitive", "false": "No urgency expressed"}
            }
        }
    });
    assert_eq!(got, want);
}

#[test]
fn declared_order_is_preserved_on_the_wire() {
    let body = String::from_utf8(full_request().to_json("m").unwrap()).unwrap();
    let (b, t, s) = (
        body.find("\"billing\"").unwrap(),
        body.find("\"technical\"").unwrap(),
        body.find("\"sales\"").unwrap(),
    );
    assert!(b < t && t < s);
    let ids: Vec<_> = full_request().question_ids().map(str::to_owned).collect();
    assert_eq!(ids, ["department", "frustration", "is_urgent"]);
}

#[test]
fn request_model_overrides_default_and_structured_state_works() {
    #[derive(serde::Serialize)]
    struct Ticket {
        subject: &'static str,
    }
    let req = Request::from_serialize(&Ticket { subject: "refund" })
        .unwrap()
        .noul::<Urgent>()
        .model("jev-1.13.0");
    let v: Value = serde_json::from_slice(&req.to_json("jev-latest").unwrap()).unwrap();
    assert_eq!(v["model"], "jev-1.13.0");
    assert_eq!(v["state"], json!({"subject": "refund"}));
}

#[test]
fn invalid_requests_are_rejected_before_sending() {
    let (client, mock) = client_with(vec![], 0);
    let empty = client.send(&Request::new("x")).unwrap_err();
    assert!(matches!(empty, Error::InvalidRequest(_)), "{empty}");
    let dupes = client
        .send(&Request::new("x").noul::<Urgent>().noul::<Urgent>())
        .unwrap_err();
    assert!(
        dupes
            .to_string()
            .contains("duplicate question id `is_urgent`"),
        "{dupes}"
    );
    assert!(mock.calls.lock().unwrap().is_empty());
}

// ---- happy path ------------------------------------------------------------

#[test]
fn sends_expected_http_and_decodes_all_three_answers() {
    let (client, mock) = client_with(vec![reply(200, &[], FULL)], 0);
    let response = client.send(&full_request()).unwrap();

    let calls = mock.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].url, "https://api.typesafe.ai/v1/systemone");
    let header = |name: &str| {
        calls[0]
            .headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(header("Authorization"), Some("Bearer sk-secret"));
    assert_eq!(header("Content-Type"), Some("application/json"));
    assert!(header("User-Agent").unwrap().starts_with("jevkit/"));
    let sent: Value = serde_json::from_slice(&calls[0].body).unwrap();
    assert_eq!(sent["model"], "jev-latest");

    assert_eq!(response.model, "jev-1.13.0");
    assert_eq!(response.usage.input_tokens, 318);

    let dept = response.choice::<Department>().unwrap();
    assert_eq!(dept.value, Department::Billing);
    assert_eq!(dept.confidence, 0.81);
    assert_eq!(dept.ranked.best().value, Department::Billing);
    assert_eq!(dept.ranked.len(), 3);
    let decision = dept.decision();
    assert!(decision.is_confident(0.8));
    assert!(decision.into_confident(0.9).is_err());

    let mood = response.score::<Frustration>().unwrap();
    assert_eq!(mood.value, Frustration::Frustrated);
    assert_eq!(mood.score, 1.05);
    assert!((mood.normalized - 0.525).abs() < 1e-9);
    assert_eq!(mood.probabilities, vec![0.0, 0.95, 0.05]);

    let urgent = response.noul::<Urgent>().unwrap();
    assert_eq!(urgent.value, Urgent::Yes);
    assert_eq!(urgent.probability, 0.95);
    assert!((urgent.decision().confidence - 0.95).abs() < 1e-9);
}

// ---- answer edge cases -----------------------------------------------------

#[test]
fn absent_wrong_type_and_drifted_answers_are_distinct_errors() {
    let body = r#"{"model":"m","answers":{
        "department":{"type":"noul","noul":0.5},
        "frustration":{"type":"score","score":0.0,"probabilities":{"0":1.0,"7":0.0},"confidence":1.0},
        "is_urgent":{"type":"mystery","x":1}
    }}"#;
    let response = Response::from_slice(body.as_bytes()).unwrap();

    assert!(matches!(
        response.choice::<Department>().unwrap_err(),
        Error::WrongAnswerType {
            expected: "choice",
            found: "noul",
            ..
        }
    ));
    assert!(matches!(
        response.score::<Frustration>().unwrap_err(),
        Error::Decode { .. }
    ));
    assert!(matches!(
        response.noul::<Urgent>().unwrap_err(),
        Error::WrongAnswerType {
            found: "unknown",
            ..
        }
    ));

    let empty = Response::from_slice(br#"{"model":"m","answers":{}}"#).unwrap();
    assert!(matches!(
        empty.noul::<Urgent>().unwrap_err(),
        Error::MissingAnswer { .. }
    ));

    let drift = br#"{"model":"m","answers":{"department":{"type":"choice","choice":"legal",
        "probabilities":{"legal":1.0},"confidence":1.0}}}"#;
    let drift = Response::from_slice(drift).unwrap();
    let err = drift.choice::<Department>().unwrap_err();
    assert!(err.to_string().contains("unknown label `legal`"), "{err}");
}

#[test]
fn float_noise_in_probabilities_is_tolerated() {
    let body = br#"{"model":"m","answers":{"department":{"type":"choice","choice":"billing",
        "probabilities":{"billing":1.0000001,"technical":0.0,"sales":0.0},"confidence":1.0}}}"#;
    let r = Response::from_slice(body).unwrap();
    assert_eq!(
        r.choice::<Department>().unwrap().ranked.best().confidence,
        1.0
    );
}

// ---- errors and retries ----------------------------------------------------

#[test]
fn non_retryable_errors_return_immediately_with_parsed_body() {
    let (client, mock) = client_with(
        vec![reply(
            422,
            &[],
            r#"{"message":"questions.x.type: expected one of 'noul', 'choice', 'score'","error_type":"invalid_request"}"#,
        )],
        3,
    );
    let Error::Api(e) = client.send(&full_request()).unwrap_err() else {
        panic!("not an api error")
    };
    assert_eq!(e.status, 422);
    assert_eq!(e.error_type.as_deref(), Some("invalid_request"));
    assert!(e.message.starts_with("questions.x.type"));
    assert_eq!(mock.calls.lock().unwrap().len(), 1);
}

#[test]
fn retries_429_then_succeeds() {
    let (client, mock) = client_with(
        vec![
            reply(429, &[], "slow down"),
            reply(529, &[], "{}"),
            reply(200, &[], FULL),
        ],
        2,
    );
    assert!(client.send(&full_request()).is_ok());
    assert_eq!(mock.calls.lock().unwrap().len(), 3);
}

#[test]
fn gives_up_after_max_retries() {
    let (client, mock) = client_with(vec![reply(503, &[], "down"); 3].into_iter().collect(), 2);
    let Error::Api(e) = client.send(&full_request()).unwrap_err() else {
        panic!()
    };
    assert_eq!((e.status, e.message.as_str()), (503, "down"));
    assert_eq!(mock.calls.lock().unwrap().len(), 3);
}

#[test]
fn retries_transport_failures() {
    let (client, mock) = client_with(
        vec![
            Err(TransportError("connection reset".into())),
            reply(200, &[], FULL),
        ],
        1,
    );
    assert!(client.send(&full_request()).is_ok());
    assert_eq!(mock.calls.lock().unwrap().len(), 2);

    let (client, _) = client_with(vec![Err(TransportError("dns".into()))], 0);
    assert!(matches!(client.send(&full_request()).unwrap_err(), Error::Transport(m) if m == "dns"));
}

#[test]
fn long_retry_after_is_not_waited_for() {
    let (client, mock) = client_with(vec![reply(429, &[("Retry-After", "120")], "{}")], 2);
    let Error::Api(e) = client.send(&full_request()).unwrap_err() else {
        panic!()
    };
    assert_eq!(e.retry_after, Some(Duration::from_secs(120)));
    assert_eq!(mock.calls.lock().unwrap().len(), 1);
}

#[test]
fn retry_delay_grows_and_is_capped() {
    let p = RetryPolicy::default();
    assert!(p.delay(0, 0.5) < p.delay(1, 0.5));
    assert_eq!(p.delay(30, 1.0), p.max_delay);
    assert_eq!(RetryPolicy::none().max_retries, 0);
}

// ---- configuration ---------------------------------------------------------

#[test]
fn missing_api_key_and_redacted_debug() {
    let mock = Arc::new(Mock::default());
    let err = Client::builder()
        .transport(Shared(mock.clone()))
        .build()
        .unwrap_err();
    assert!(matches!(err, Error::MissingApiKey));
    let err = Client::builder()
        .api_key("  ")
        .transport(Shared(mock.clone()))
        .build()
        .unwrap_err();
    assert!(matches!(err, Error::MissingApiKey));

    let client = Client::builder()
        .api_key("sk-secret")
        .transport(Shared(mock))
        .build()
        .unwrap();
    let debug = format!("{client:?}");
    assert!(
        !debug.contains("sk-secret") && debug.contains("redacted"),
        "{debug}"
    );
    assert_eq!(client.default_model(), "jev-latest");
}

// ---- real HTTP through the built-in ureq transport -------------------------

/// Serve `responses` (raw HTTP) one per connection; return what each request looked like.
fn serve(responses: Vec<String>) -> (String, std::thread::JoinHandle<Vec<(String, String)>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let mut seen = Vec::new();
        for raw in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            let (head_end, content_length) = loop {
                let n = stream.read(&mut chunk).unwrap();
                buf.extend_from_slice(&chunk[..n]);
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..pos]).to_lowercase();
                    let len = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .map(|v| v.trim().parse::<usize>().unwrap())
                        .unwrap_or(0);
                    break (pos + 4, len);
                }
            };
            while buf.len() < head_end + content_length {
                let n = stream.read(&mut chunk).unwrap();
                buf.extend_from_slice(&chunk[..n]);
            }
            let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
            let body = String::from_utf8_lossy(&buf[head_end..]).to_string();
            seen.push((head, body));
            stream.write_all(raw.as_bytes()).unwrap();
        }
        seen
    });
    (base, handle)
}

fn http(status: &str, extra: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",
        body.len()
    )
}

#[test]
fn ureq_transport_end_to_end_with_retry_after() {
    let (base, server) = serve(vec![
        http(
            "429 Too Many Requests",
            "Retry-After: 0\r\n",
            r#"{"message":"slow"}"#,
        ),
        http("200 OK", "", FULL),
    ]);
    let client = Client::builder()
        .api_key("sk-live")
        .base_url(format!("{base}/"))
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();

    let response = client.send(&full_request()).unwrap();
    assert_eq!(
        response.choice::<Department>().unwrap().value,
        Department::Billing
    );

    let seen = server.join().unwrap();
    assert_eq!(seen.len(), 2);
    let (head, body) = &seen[1];
    assert!(head.starts_with("POST /v1/systemone HTTP/1.1"), "{head}");
    assert!(
        head.to_lowercase()
            .contains("authorization: bearer sk-live"),
        "{head}"
    );
    let sent: Value = serde_json::from_str(body).unwrap();
    assert_eq!(sent["questions"]["is_urgent"]["type"], "noul");
}

#[test]
fn ureq_transport_surfaces_api_errors() {
    let (base, server) = serve(vec![http(
        "401 Unauthorized",
        "",
        r#"{"message":"bad key","error_type":"authentication_error"}"#,
    )]);
    let client = Client::builder()
        .api_key("nope")
        .base_url(base)
        .build()
        .unwrap();
    let Error::Api(e) = client.send(&full_request()).unwrap_err() else {
        panic!()
    };
    assert_eq!(
        (e.status, e.error_type.as_deref()),
        (401, Some("authentication_error"))
    );
    server.join().unwrap();
}
