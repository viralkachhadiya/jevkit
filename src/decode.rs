//! Turning raw probabilities into typed decisions.

use core::fmt;

use crate::{Choice, Noul, Score};

/// A typed answer together with the model's confidence in it (`0.0..=1.0`).
#[derive(Debug, Clone, PartialEq)]
pub struct Decision<T> {
    /// The decided value.
    pub value: T,
    /// Confidence in `value`, `0.0..=1.0`. From the `decode_*` helpers this is
    /// the probability of the picked option; clients may supply the API's
    /// own confidence instead.
    pub confidence: f64,
}

impl<T> Decision<T> {
    /// Create a decision.
    pub fn new(value: T, confidence: f64) -> Self {
        Self { value, confidence }
    }

    /// `true` if confidence is at least `min`.
    pub fn is_confident(&self, min: f64) -> bool {
        self.confidence >= min
    }

    /// Unwrap the value if confidence is at least `min`, otherwise hand the
    /// decision back so you can fall back to a rule or a slower model.
    pub fn into_confident(self, min: f64) -> Result<T, Self> {
        if self.is_confident(min) {
            Ok(self.value)
        } else {
            Err(self)
        }
    }

    /// Transform the value, keeping the confidence.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Decision<U> {
        Decision {
            value: f(self.value),
            confidence: self.confidence,
        }
    }
}

/// Choice answers, most likely first. Never empty.
#[derive(Debug, Clone, PartialEq)]
pub struct Ranked<T>(Vec<Decision<T>>);

impl<T> Ranked<T> {
    /// The most likely answer.
    pub fn best(&self) -> &Decision<T> {
        &self.0[0]
    }

    /// Consume the ranking and keep only the most likely answer.
    pub fn into_best(mut self) -> Decision<T> {
        self.0.swap_remove(0)
    }

    /// Iterate from most to least likely.
    pub fn iter(&self) -> core::slice::Iter<'_, Decision<T>> {
        self.0.iter()
    }

    /// Number of ranked answers.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Always `false`; a `Ranked` holds at least one answer.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Take the underlying vector.
    pub fn into_vec(self) -> Vec<Decision<T>> {
        self.0
    }
}

/// Why a set of probabilities could not be decoded.
#[derive(Debug, Clone, PartialEq)]
pub enum DecodeError {
    /// No probabilities were supplied.
    Empty,
    /// A label the type does not know about.
    UnknownLabel(String),
    /// A probability outside `0.0..=1.0`, or not finite.
    InvalidProbability(f64),
    /// A score had the wrong number of level probabilities.
    WrongLevelCount {
        /// Levels the type declares.
        expected: usize,
        /// Probabilities supplied.
        found: usize,
    },
    /// A level index the type cannot construct.
    BadIndex(usize),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("no probabilities supplied"),
            Self::UnknownLabel(l) => write!(f, "unknown label `{l}`"),
            Self::InvalidProbability(p) => write!(f, "invalid probability {p}"),
            Self::WrongLevelCount { expected, found } => {
                write!(f, "expected {expected} level probabilities, found {found}")
            }
            Self::BadIndex(i) => write!(f, "no level with index {i}"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Tolerance for float noise such as `1.0000001` in API responses.
const EPSILON: f64 = 1e-6;

fn check(p: f64) -> Result<f64, DecodeError> {
    if p.is_finite() && (-EPSILON..=1.0 + EPSILON).contains(&p) {
        Ok(p.clamp(0.0, 1.0))
    } else {
        Err(DecodeError::InvalidProbability(p))
    }
}

/// Decode `(label, probability)` pairs into ranked typed answers.
pub fn decode_choice<T: Choice>(probabilities: &[(&str, f64)]) -> Result<Ranked<T>, DecodeError> {
    if probabilities.is_empty() {
        return Err(DecodeError::Empty);
    }
    let mut out = Vec::with_capacity(probabilities.len());
    for &(label, p) in probabilities {
        let p = check(p)?;
        let value = T::from_label(label).ok_or_else(|| DecodeError::UnknownLabel(label.into()))?;
        out.push(Decision::new(value, p));
    }
    out.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(core::cmp::Ordering::Equal)
    });
    Ok(Ranked(out))
}

/// Decode per-level probabilities (lowest level first) into the most likely level.
pub fn decode_score<T: Score>(probabilities: &[f64]) -> Result<Decision<T>, DecodeError> {
    let expected = T::SPEC.levels().len();
    if probabilities.len() != expected {
        return Err(DecodeError::WrongLevelCount {
            expected,
            found: probabilities.len(),
        });
    }
    let mut best = (0usize, check(probabilities[0])?);
    for (i, &p) in probabilities.iter().enumerate().skip(1) {
        let p = check(p)?;
        if p > best.1 {
            best = (i, p);
        }
    }
    let value = T::from_index(best.0).ok_or(DecodeError::BadIndex(best.0))?;
    Ok(Decision::new(value, best.1))
}

/// Expected score on a `0.0..=1.0` scale, weighting every level by its
/// probability. Smoother than the argmax from [`decode_score`].
pub fn expected_normalized<T: Score>(probabilities: &[f64]) -> Result<f64, DecodeError> {
    let expected = T::SPEC.levels().len();
    if probabilities.len() != expected {
        return Err(DecodeError::WrongLevelCount {
            expected,
            found: probabilities.len(),
        });
    }
    let mut total = 0.0;
    let mut weighted = 0.0;
    for (i, &p) in probabilities.iter().enumerate() {
        let p = check(p)?;
        total += p;
        weighted += p * i as f64 / (expected - 1).max(1) as f64;
    }
    if total <= 0.0 {
        return Err(DecodeError::InvalidProbability(total));
    }
    Ok(weighted / total)
}

/// Decode the probability of "yes" into a typed value with confidence.
pub fn decode_noul<T: Noul>(probability_yes: f64) -> Result<Decision<T>, DecodeError> {
    let p = check(probability_yes)?;
    let yes = p >= 0.5;
    let confidence = if yes { p } else { 1.0 - p };
    Ok(Decision::new(T::from_bool(yes), confidence))
}
