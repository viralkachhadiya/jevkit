use std::collections::HashSet;

use crate::{Choice, Noul, Score, MAX_CHOICE_OPTIONS, MAX_SCORE_LEVELS, MIN_SCORE_LEVELS};
use serde::Serialize;
use serde_json::{json, Map, Value};

use super::Error;

/// A System One request: one state plus any number of typed questions.
///
/// Question ids are the `name` of each derived type (the enum name in
/// snake_case unless overridden), so answers are looked up by type, not by
/// string.
///
/// All questions in one request are answered against the same state, and the
/// state is billed once, so batch your questions.
#[derive(Debug, Clone)]
pub struct Request {
    state: Value,
    model: Option<String>,
    questions: Vec<(String, Value)>,
    problems: Vec<String>,
}

impl Request {
    /// Start a request. `state` can be text (`&str`, `String`) or a
    /// [`serde_json::Value`].
    pub fn new(state: impl Into<Value>) -> Self {
        Self {
            state: state.into(),
            model: None,
            questions: Vec::new(),
            problems: Vec::new(),
        }
    }

    /// Start a request from any serializable state (a struct, a list of chat
    /// messages, ...).
    pub fn from_serialize<S: Serialize>(state: &S) -> Result<Self, Error> {
        Ok(Self::new(serde_json::to_value(state)?))
    }

    /// Override the client's default model for this request.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Ask a pick-one question.
    pub fn choice<T: Choice>(mut self) -> Self {
        let spec = T::SPEC;
        if spec.options().len() < 2 || spec.options().len() > MAX_CHOICE_OPTIONS {
            self.problems.push(format!(
                "`{}`: a choice needs 2 to {MAX_CHOICE_OPTIONS} options, found {}",
                spec.name(),
                spec.options().len()
            ));
        }
        let mut criteria = Map::new();
        for o in spec.options() {
            criteria.insert(o.label().to_string(), Value::from(o.description()));
        }
        self.questions.push((
            spec.name().to_string(),
            json!({
                "type": "choice",
                "instructions": spec.question(),
                "criteria": Value::Object(criteria),
            }),
        ));
        self
    }

    /// Ask an ordered rating question.
    pub fn score<T: Score>(mut self) -> Self {
        let spec = T::SPEC;
        if spec.levels().len() < MIN_SCORE_LEVELS || spec.levels().len() > MAX_SCORE_LEVELS {
            self.problems.push(format!(
                "`{}`: a score needs {MIN_SCORE_LEVELS} to {MAX_SCORE_LEVELS} levels, found {}",
                spec.name(),
                spec.levels().len()
            ));
        }
        self.questions.push((
            spec.name().to_string(),
            json!({
                "type": "score",
                "instructions": spec.question(),
                "criteria": spec.levels().iter().map(|l| l.description()).collect::<Vec<_>>(),
            }),
        ));
        self
    }

    /// Ask a yes/no question.
    pub fn noul<T: Noul>(mut self) -> Self {
        let spec = T::SPEC;
        let mut question = json!({
            "type": "noul",
            "instructions": spec.question(),
        });
        let mut criteria = Map::new();
        if !spec.yes().is_empty() {
            criteria.insert("true".into(), Value::from(spec.yes()));
        }
        if !spec.no().is_empty() {
            criteria.insert("false".into(), Value::from(spec.no()));
        }
        if !criteria.is_empty() {
            question["criteria"] = Value::Object(criteria);
        }
        self.questions.push((spec.name().to_string(), question));
        self
    }

    /// The question ids in this request, in the order they were added.
    pub fn question_ids(&self) -> impl Iterator<Item = &str> {
        self.questions.iter().map(|(id, _)| id.as_str())
    }

    /// Check the request without sending it.
    pub fn validate(&self) -> Result<(), Error> {
        if let Some(p) = self.problems.first() {
            return Err(Error::InvalidRequest(p.clone()));
        }
        if self.questions.is_empty() {
            return Err(Error::InvalidRequest(
                "add at least one question (.choice::<T>(), .score::<T>() or .noul::<T>())".into(),
            ));
        }
        let mut seen = HashSet::new();
        for (id, _) in &self.questions {
            if !seen.insert(id.as_str()) {
                return Err(Error::InvalidRequest(format!(
                    "duplicate question id `{id}`; give each type a distinct #[jev(name = \"...\")]"
                )));
            }
        }
        Ok(())
    }

    /// The JSON body to `POST` to `/v1/systemone`. Sans-IO: use this with your
    /// own HTTP stack if you don't want [`Client`](crate::Client).
    pub fn to_json(&self, default_model: &str) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let mut questions = Map::new();
        for (id, q) in &self.questions {
            questions.insert(id.clone(), q.clone());
        }
        let body = json!({
            "state": self.state,
            "model": self.model.as_deref().unwrap_or(default_model),
            "questions": Value::Object(questions),
        });
        Ok(serde_json::to_vec(&body)?)
    }
}
