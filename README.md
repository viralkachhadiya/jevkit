# jevkit

Typed Rust for [TypeSafe AI's Jev](https://docs.typesafe.ai/api) (the System One API). Define each question as a plain enum, ask them in one request, and get typed answers back with confidence. Question ids, labels and JSON never appear in your code.

One crate: the macros, the decoding helpers and a blocking HTTP client.

```toml
[dependencies]
jevkit = "0.1"
```

Jev answers three kinds of question: **Choice** (pick one), **Score** (rate on 2 to 10 ordered levels) and **Noul** (yes/no). Each has a macro.

```rust,no_run
use jevkit::{Client, Request};

jevkit::choice! {
    /// Which team should handle this ticket?
    pub enum Department {
        /// Payments, invoicing, refunds
        Billing,
        /// Bugs, outages, integrations
        Technical,
    }
}

jevkit::score! {
    /// How frustrated is the customer?
    pub enum Frustration {
        Calm,
        Frustrated,
        Angry = "Very angry",
    }
}

jevkit::noul! {
    /// Does this convey urgency?
    pub enum Urgent {
        /// Explicitly time-sensitive
        yes Yes,
        /// No urgency expressed
        no No,
    }
}

fn main() -> Result<(), jevkit::Error> {
    // Reads TYPESAFE_API_KEY (and optionally TYPESAFE_BASE_URL, TYPESAFE_DEFAULT_MODEL).
    let client = Client::from_env()?;

    // One request, one billed state, three questions answered in parallel.
    let response = client.send(
        &Request::new("Help! My payouts have been failing for 3 days.")
            .choice::<Department>()
            .score::<Frustration>()
            .noul::<Urgent>(),
    )?;

    let dept = response.choice::<Department>()?;
    println!("{:?} (confidence {:.2})", dept.value, dept.confidence);

    // Only trust confident answers; otherwise fall back to something else.
    match dept.decision().into_confident(0.8) {
        Ok(team) => println!("route to {team:?}"),
        Err(unsure) => println!("unsure ({:.2}): send to a human", unsure.confidence),
    }

    let mood = response.score::<Frustration>()?;
    println!("frustration {:.2} on a 0..1 scale", mood.normalized);
    println!("urgent? {:?}", response.noul::<Urgent>()?.value);
    Ok(())
}
```

## The macros

| Macro | Defines | Notes |
|-------|---------|-------|
| `choice!` | pick-one enum, 2 to 255 variants | `Variant = "label"` overrides the label (default: the variant name) |
| `score!` | ordered levels, 2 to 10 variants, lowest first | `Variant = "meaning"` overrides the level description |
| `noul!` | yes/no enum with one `yes` and one `no` variant | `yes A, no B` in either order |

In all three, the **enum's doc comment is the question** and each **variant's doc comment is what it means**, so what you write for readers is what the model sees. Optional attributes go after the docs and before `enum`, in this order:

```rust
jevkit::choice! {
    /// Which team should handle this?
    #[jev(name = "team")]              // question id; default is the enum name
    #[derive(PartialOrd, Ord)]         // added to Debug, Clone, Copy, PartialEq, Eq, Hash
    pub enum Department { Billing, Technical }
}
```

Mistakes fail at `cargo build`, not at request time: a missing question, the wrong number of variants, and duplicate or empty labels are compile errors.

## Sans-IO: bring your own HTTP

`Request` and `Response` work without the client, so you can use `reqwest`, `hyper` or a test double. This example runs offline:

```rust
use jevkit::{Request, Response};

jevkit::noul! {
    /// Does this convey urgency?
    #[jev(name = "is_urgent")]
    pub enum Urgent {
        /// Explicitly time-sensitive
        yes Yes,
        /// No urgency expressed
        no No,
    }
}

// POST this to https://api.typesafe.ai/v1/systemone with `Authorization: Bearer <key>`.
let body = Request::new("Help! My payouts have been failing for 3 days.")
    .noul::<Urgent>()
    .to_json("jev-latest")
    .unwrap();
let sent: serde_json::Value = serde_json::from_slice(&body).unwrap();
assert_eq!(sent["questions"]["is_urgent"]["type"], "noul");

// Parse whatever came back.
let reply = br#"{"model":"jev-1.13.0","answers":{"is_urgent":{"type":"noul","noul":0.95}},
                "usage":{"input_tokens":296,"output_tokens":20}}"#;
let response = Response::from_slice(reply).unwrap();
assert_eq!(response.noul::<Urgent>().unwrap().value, Urgent::Yes);
assert_eq!(response.usage.input_tokens, 296);
```

To use your own HTTP stack inside a `Client`, implement `client::Transport` (one method) and pass it to `Client::builder().transport(...)`.

## Features

| Feature | Default | Adds |
|---------|---------|------|
| `ureq` | yes | The built-in blocking HTTP transport (rustls). Implies `client`. |
| `client` | via `ureq` | `Request`, `Response`, `Client`, retries, the `Transport` trait (`serde`, `serde_json`). |
| *(none)* | | Just the macros, the traits and `decode_*` helpers, with no dependencies. Use `default-features = false`. |

## Behaviour worth knowing

- **Question ids** are each enum's name unless you set `#[jev(name = "...")]`. Two questions with the same id are rejected before anything is sent.
- **Missing answers**: the API does not promise an answer to every question, so `response.choice::<T>()` returns `Error::MissingAnswer` when one is absent.
- **Drift**: if the API returns a label your enum does not know, you get `Error::Decode` rather than a silent wrong value.
- **Retries**: network failures and 429, 502, 503, 504 and 529 are retried (default 2 retries, exponential backoff with jitter, `Retry-After` honoured up to `max_delay`). Configure with `RetryPolicy`.
- **Secrets**: the API key is redacted from `Debug` output.
- **Blocking only** for now. Async users can use the sans-IO types above.
- **Rust version**: the macros and decoding (`default-features = false`) are tested on Rust 1.70. The client's HTTP dependencies need a recent stable toolchain.

## Why macros instead of `#[derive]`?

Derive macros must live in a separate proc-macro crate, which would mean publishing two crates. Declarative macros keep everything in one, at the cost of a slightly different syntax: the enum is defined inside the macro. Labels are the variant names as written (case preserved), since a declarative macro cannot rewrite identifiers.

This is an unofficial community crate, not a TypeSafe product.

License: MIT
