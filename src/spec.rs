//! Static descriptions of typed questions, and the traits that carry them.

/// Maximum number of options a [`Choice`] may have.
pub const MAX_CHOICE_OPTIONS: usize = 255;
/// Minimum number of levels a [`Score`] may have.
pub const MIN_SCORE_LEVELS: usize = 2;
/// Maximum number of levels a [`Score`] may have.
pub const MAX_SCORE_LEVELS: usize = 10;

/// One option of a [`Choice`] question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChoiceOption {
    label: &'static str,
    description: &'static str,
}

impl ChoiceOption {
    /// Create an option. Used by the [`choice!`](crate::choice) macro.
    pub const fn new(label: &'static str, description: &'static str) -> Self {
        Self { label, description }
    }

    /// Stable machine label, e.g. `"billing"`.
    pub fn label(&self) -> &'static str {
        self.label
    }

    /// What this option means, shown to the model. Falls back to the label
    /// when no description was given.
    pub fn description(&self) -> &'static str {
        let d = self.description.trim();
        if d.is_empty() {
            self.label
        } else {
            d
        }
    }
}

/// Static description of a [`Choice`] question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChoiceSpec {
    name: &'static str,
    question: &'static str,
    options: &'static [ChoiceOption],
}

impl ChoiceSpec {
    /// Create a spec. Used by the [`choice!`](crate::choice) macro.
    pub const fn new(
        name: &'static str,
        question: &'static str,
        options: &'static [ChoiceOption],
    ) -> Self {
        Self {
            name,
            question,
            options,
        }
    }

    /// Question identifier, unique within one request.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The question text.
    pub fn question(&self) -> &'static str {
        self.question.trim()
    }

    /// The options, in declaration order.
    pub fn options(&self) -> &'static [ChoiceOption] {
        self.options
    }
}

/// One level of a [`Score`] question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoreLevel {
    name: &'static str,
    description: &'static str,
}

impl ScoreLevel {
    /// Create a level. Used by the [`score!`](crate::score) macro.
    pub const fn new(name: &'static str, description: &'static str) -> Self {
        Self { name, description }
    }

    /// The variant name of this level.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// What this level means, shown to the model. Falls back to the name.
    pub fn description(&self) -> &'static str {
        let d = self.description.trim();
        if d.is_empty() {
            self.name
        } else {
            d
        }
    }
}

/// Static description of a [`Score`] question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoreSpec {
    name: &'static str,
    question: &'static str,
    levels: &'static [ScoreLevel],
}

impl ScoreSpec {
    /// Create a spec. Used by the [`score!`](crate::score) macro.
    pub const fn new(
        name: &'static str,
        question: &'static str,
        levels: &'static [ScoreLevel],
    ) -> Self {
        Self {
            name,
            question,
            levels,
        }
    }

    /// Question identifier, unique within one request.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The question text.
    pub fn question(&self) -> &'static str {
        self.question.trim()
    }

    /// The levels, lowest first.
    pub fn levels(&self) -> &'static [ScoreLevel] {
        self.levels
    }
}

/// Static description of a [`Noul`] (yes/no) question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoulSpec {
    name: &'static str,
    question: &'static str,
    yes: &'static str,
    no: &'static str,
}

impl NoulSpec {
    /// Create a spec. Used by the [`noul!`](crate::noul) macro.
    pub const fn new(
        name: &'static str,
        question: &'static str,
        yes: &'static str,
        no: &'static str,
    ) -> Self {
        Self {
            name,
            question,
            yes,
            no,
        }
    }

    /// Question identifier, unique within one request.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The question text.
    pub fn question(&self) -> &'static str {
        self.question.trim()
    }

    /// What "yes" means; empty if not described.
    pub fn yes(&self) -> &'static str {
        self.yes.trim()
    }

    /// What "no" means; empty if not described.
    pub fn no(&self) -> &'static str {
        self.no.trim()
    }
}

/// A pick-one question. Define one with the [`choice!`](crate::choice) macro.
pub trait Choice: Sized {
    /// The question description to send to Jev.
    const SPEC: ChoiceSpec;
    /// The machine label of this value.
    fn label(&self) -> &'static str;
    /// Parse a label returned by Jev.
    fn from_label(label: &str) -> Option<Self>;
}

/// An ordered rating question. Define one with the [`score!`](crate::score)
/// macro; variants are the levels, lowest first.
pub trait Score: Sized {
    /// The question description to send to Jev.
    const SPEC: ScoreSpec;
    /// Zero-based level index of this value.
    fn index(&self) -> usize;
    /// Build a value from a zero-based level index.
    fn from_index(index: usize) -> Option<Self>;

    /// Position on a `0.0..=1.0` scale, so rubrics of different lengths can
    /// be compared or combined.
    fn normalized(&self) -> f64 {
        let last = Self::SPEC.levels.len().saturating_sub(1).max(1);
        self.index() as f64 / last as f64
    }
}

/// A yes/no question. Define one with the [`noul!`](crate::noul) macro.
pub trait Noul: Sized {
    /// The question description to send to Jev.
    const SPEC: NoulSpec;
    /// `true` for the "yes" variant.
    fn as_bool(&self) -> bool;
    /// Build the "yes" variant from `true`, the "no" variant from `false`.
    fn from_bool(value: bool) -> Self;
}

/// Compile-time validation used by the macros. Not public API.
#[doc(hidden)]
pub mod __private {
    use super::*;

    const fn str_eq(a: &str, b: &str) -> bool {
        let (a, b) = (a.as_bytes(), b.as_bytes());
        if a.len() != b.len() {
            return false;
        }
        let mut i = 0;
        while i < a.len() {
            if a[i] != b[i] {
                return false;
            }
            i += 1;
        }
        true
    }

    const fn has_text(s: &str) -> bool {
        let b = s.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if !matches!(b[i], b' ' | b'\t' | b'\n' | b'\r') {
                return true;
            }
            i += 1;
        }
        false
    }

    const fn check_question(name: &str, question: &str) {
        assert!(
            has_text(name),
            "jevkit: the question name must not be empty"
        );
        assert!(
            has_text(question),
            "jevkit: a question is required. Put a doc comment (///) above the enum"
        );
    }

    pub const fn check_choice(spec: &ChoiceSpec) {
        check_question(spec.name, spec.question);
        let o = spec.options;
        assert!(
            o.len() >= 2 && o.len() <= MAX_CHOICE_OPTIONS,
            "jevkit: a choice! enum needs between 2 and 255 variants"
        );
        let mut i = 0;
        while i < o.len() {
            assert!(
                has_text(o[i].label),
                "jevkit: a choice label must not be empty"
            );
            let mut j = i + 1;
            while j < o.len() {
                assert!(
                    !str_eq(o[i].label, o[j].label),
                    "jevkit: duplicate label in choice! enum; labels must be unique"
                );
                j += 1;
            }
            i += 1;
        }
    }

    pub const fn check_score(spec: &ScoreSpec) {
        check_question(spec.name, spec.question);
        assert!(
            spec.levels.len() >= MIN_SCORE_LEVELS && spec.levels.len() <= MAX_SCORE_LEVELS,
            "jevkit: a score! enum needs between 2 and 10 variants (levels)"
        );
    }

    pub const fn check_noul(spec: &NoulSpec) {
        check_question(spec.name, spec.question);
    }
}
