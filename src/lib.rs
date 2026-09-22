#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod decode;
mod macros;
mod spec;

#[cfg(feature = "client")]
pub mod client;

pub use decode::{
    decode_choice, decode_noul, decode_score, expected_normalized, Decision, DecodeError, Ranked,
};
#[doc(hidden)]
pub use spec::__private;
pub use spec::{
    Choice, ChoiceOption, ChoiceSpec, Noul, NoulSpec, Score, ScoreLevel, ScoreSpec,
    MAX_CHOICE_OPTIONS, MAX_SCORE_LEVELS, MIN_SCORE_LEVELS,
};

#[cfg(feature = "client")]
pub use client::{Client, ClientBuilder, Error, Request, Response, RetryPolicy};
