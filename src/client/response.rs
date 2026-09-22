use std::collections::HashMap;

use crate::{
    decode_choice, decode_noul, decode_score, Choice, Decision, DecodeError, Noul, Ranked, Score,
};
use serde::Deserialize;

use super::Error;

/// Token usage reported by the API.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct Usage {
    /// Billable input tokens.
    #[serde(default)]
    pub input_tokens: u64,
    /// Output tokens (currently free).
    #[serde(default)]
    pub output_tokens: u64,
}

/// One raw answer exactly as the API returned it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Answer {
    /// Probability of "yes", `0.0..=1.0`.
    Noul {
        /// Probability the answer is yes.
        noul: f64,
    },
    /// A picked option with the full distribution.
    Choice {
        /// The highest-probability option.
        choice: String,
        /// Every option's probability.
        probabilities: HashMap<String, f64>,
        /// Confidence derived from the distribution.
        confidence: f64,
    },
    /// A probability-weighted level.
    Score {
        /// Weighted level, can land between levels.
        score: f64,
        /// Level index to description.
        #[serde(default)]
        legend: HashMap<String, String>,
        /// Level index (as a string) to probability.
        probabilities: HashMap<String, f64>,
        /// Confidence derived from the distribution.
        confidence: f64,
    },
    /// A type this client does not know yet.
    #[serde(other)]
    Unknown,
}

impl Answer {
    fn kind(&self) -> &'static str {
        match self {
            Self::Noul { .. } => "noul",
            Self::Choice { .. } => "choice",
            Self::Score { .. } => "score",
            Self::Unknown => "unknown",
        }
    }
}

/// A parsed System One response.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Response {
    /// The model that answered, for example `jev-1.13.0`.
    pub model: String,
    /// Raw answers keyed by question id.
    pub answers: HashMap<String, Answer>,
    /// Token usage.
    #[serde(default)]
    pub usage: Usage,
}

/// A decoded choice answer.
#[derive(Debug, Clone, PartialEq)]
pub struct ChoiceAnswer<T> {
    /// The option the API picked.
    pub value: T,
    /// The API's confidence, derived from the whole distribution.
    pub confidence: f64,
    /// Every option with its probability, most likely first.
    pub ranked: Ranked<T>,
}

impl<T> ChoiceAnswer<T> {
    /// Value plus API confidence, for `into_confident` gating.
    pub fn decision(self) -> Decision<T> {
        Decision::new(self.value, self.confidence)
    }
}

/// A decoded score answer.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreAnswer<T> {
    /// The most likely level.
    pub value: T,
    /// The API's confidence, derived from the whole distribution.
    pub confidence: f64,
    /// Probability-weighted level in level-index units (can be fractional).
    pub score: f64,
    /// `score` rescaled to `0.0..=1.0`, so rubrics of different lengths can
    /// be compared or combined.
    pub normalized: f64,
    /// Probability of each level, lowest first.
    pub probabilities: Vec<f64>,
}

impl<T> ScoreAnswer<T> {
    /// Value plus API confidence, for `into_confident` gating.
    pub fn decision(self) -> Decision<T> {
        Decision::new(self.value, self.confidence)
    }
}

/// A decoded yes/no answer.
#[derive(Debug, Clone, PartialEq)]
pub struct NoulAnswer<T> {
    /// "Yes" variant if `probability >= 0.5`, else the "no" variant.
    pub value: T,
    /// Probability of yes.
    pub probability: f64,
}

impl<T> NoulAnswer<T> {
    /// Value plus confidence (the probability of the side that won).
    pub fn decision(self) -> Decision<T> {
        let confidence = self.probability.max(1.0 - self.probability);
        Decision::new(self.value, confidence)
    }
}

impl Response {
    /// Parse a response body. Sans-IO counterpart of `Request::to_json`.
    pub fn from_slice(body: &[u8]) -> Result<Self, Error> {
        Ok(serde_json::from_slice(body)?)
    }

    /// The raw answer for a question id, if the API returned one.
    pub fn raw(&self, id: &str) -> Option<&Answer> {
        self.answers.get(id)
    }

    fn answer(&self, id: &str) -> Result<&Answer, Error> {
        self.answers
            .get(id)
            .ok_or_else(|| Error::MissingAnswer { id: id.into() })
    }

    fn wrong(id: &str, expected: &'static str, found: &Answer) -> Error {
        Error::WrongAnswerType {
            id: id.into(),
            expected,
            found: found.kind(),
        }
    }

    fn decode(id: &str, source: DecodeError) -> Error {
        Error::Decode {
            id: id.into(),
            source,
        }
    }

    /// The decoded answer for a [`Choice`] question you asked.
    pub fn choice<T: Choice>(&self) -> Result<ChoiceAnswer<T>, Error> {
        let id = T::SPEC.name();
        match self.answer(id)? {
            Answer::Choice {
                choice,
                probabilities,
                confidence,
            } => {
                let value = T::from_label(choice)
                    .ok_or_else(|| Self::decode(id, DecodeError::UnknownLabel(choice.clone())))?;
                let pairs: Vec<(&str, f64)> = probabilities
                    .iter()
                    .map(|(k, v)| (k.as_str(), *v))
                    .collect();
                let ranked = decode_choice::<T>(&pairs).map_err(|e| Self::decode(id, e))?;
                Ok(ChoiceAnswer {
                    value,
                    confidence: *confidence,
                    ranked,
                })
            }
            other => Err(Self::wrong(id, "choice", other)),
        }
    }

    /// The decoded answer for a [`Score`] question you asked.
    pub fn score<T: Score>(&self) -> Result<ScoreAnswer<T>, Error> {
        let id = T::SPEC.name();
        match self.answer(id)? {
            Answer::Score {
                score,
                probabilities,
                confidence,
                ..
            } => {
                let levels = T::SPEC.levels().len();
                let mut probs = vec![0.0; levels];
                for (key, p) in probabilities {
                    let i: usize = key
                        .parse()
                        .map_err(|_| Self::decode(id, DecodeError::UnknownLabel(key.clone())))?;
                    if i >= levels {
                        return Err(Self::decode(
                            id,
                            DecodeError::WrongLevelCount {
                                expected: levels,
                                found: i + 1,
                            },
                        ));
                    }
                    probs[i] = *p;
                }
                let best = decode_score::<T>(&probs).map_err(|e| Self::decode(id, e))?;
                let normalized = (score / (levels - 1).max(1) as f64).clamp(0.0, 1.0);
                Ok(ScoreAnswer {
                    value: best.value,
                    confidence: *confidence,
                    score: *score,
                    normalized,
                    probabilities: probs,
                })
            }
            other => Err(Self::wrong(id, "score", other)),
        }
    }

    /// The decoded answer for a [`Noul`] question you asked.
    pub fn noul<T: Noul>(&self) -> Result<NoulAnswer<T>, Error> {
        let id = T::SPEC.name();
        match self.answer(id)? {
            Answer::Noul { noul } => {
                let d = decode_noul::<T>(*noul).map_err(|e| Self::decode(id, e))?;
                Ok(NoulAnswer {
                    value: d.value,
                    probability: *noul,
                })
            }
            other => Err(Self::wrong(id, "noul", other)),
        }
    }
}
