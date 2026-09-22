use jevkit::{
    decode_choice, decode_noul, decode_score, expected_normalized, Choice, DecodeError, Noul, Score,
};

jevkit::choice! {
    /// Which team should handle this?
    /// Pick the closest match.
    enum Team {
        /// Payments
        Billing,
        TechnicalSupport,
        #[doc = "Everything else"]
        Other = "misc",
    }
}

jevkit::score! {
    /// How frustrated is the customer?
    #[jev(name = "frustration")]
    #[derive(PartialOrd, Ord)]
    pub(crate) enum Frustration {
        Calm,
        Annoyed = "Visibly annoyed",
        Furious,
    }
}

jevkit::noul! {
    /// Is this urgent?
    #[jev(name = "urgent")]
    enum Urgent {
        /// There is a deadline
        yes Yes,
        /// Routine
        no No,
    }
}

// `no` first must work too, and docs on the variants are optional.
jevkit::noul! {
    /// Is this spam?
    enum Spam {
        no Ham,
        yes Junk,
    }
}

#[test]
fn choice_spec_uses_docs_labels_and_fallbacks() {
    let spec = Team::SPEC;
    assert_eq!(spec.name(), "Team");
    // Multi-line doc comments are joined, and leading spaces trimmed.
    assert_eq!(
        spec.question(),
        "Which team should handle this? Pick the closest match."
    );
    let labels: Vec<_> = spec.options().iter().map(|o| o.label()).collect();
    assert_eq!(labels, ["Billing", "TechnicalSupport", "misc"]);
    assert_eq!(spec.options()[0].description(), "Payments");
    // No doc comment: falls back to the label.
    assert_eq!(spec.options()[1].description(), "TechnicalSupport");
    assert_eq!(spec.options()[2].description(), "Everything else");
}

#[test]
fn choice_round_trips_and_lists_variants() {
    for t in Team::VARIANTS {
        assert_eq!(Team::from_label(t.label()), Some(*t));
    }
    assert_eq!(Team::VARIANTS.len(), 3);
    assert_eq!(Team::from_label("nope"), None);
    assert_eq!(Team::Other.label(), "misc");
}

#[test]
fn choice_decode_ranks_and_rejects() {
    let ranked =
        decode_choice::<Team>(&[("misc", 0.1), ("Billing", 0.7), ("TechnicalSupport", 0.2)])
            .unwrap();
    let order: Vec<_> = ranked.iter().map(|d| d.value).collect();
    assert_eq!(order, [Team::Billing, Team::TechnicalSupport, Team::Other]);
    assert_eq!(ranked.best().confidence, 0.7);
    assert_eq!(ranked.into_best().value, Team::Billing);

    assert_eq!(decode_choice::<Team>(&[]).unwrap_err(), DecodeError::Empty);
    assert_eq!(
        decode_choice::<Team>(&[("ghost", 0.5)]).unwrap_err(),
        DecodeError::UnknownLabel("ghost".into())
    );
    assert!(matches!(
        decode_choice::<Team>(&[("Billing", 1.5)]),
        Err(DecodeError::InvalidProbability(_))
    ));
}

#[test]
fn score_spec_index_and_derive_passthrough() {
    let spec = Frustration::SPEC;
    assert_eq!(spec.name(), "frustration");
    let descriptions: Vec<_> = spec.levels().iter().map(|l| l.description()).collect();
    // No doc or override: falls back to the variant name.
    assert_eq!(descriptions, ["Calm", "Visibly annoyed", "Furious"]);
    assert_eq!(Frustration::Furious.index(), 2);
    assert_eq!(Frustration::from_index(1), Some(Frustration::Annoyed));
    assert_eq!(Frustration::from_index(3), None);
    assert_eq!(Frustration::Annoyed.normalized(), 0.5);
    // `#[derive(PartialOrd, Ord)]` was passed through.
    assert!(Frustration::Calm < Frustration::Furious);
}

#[test]
fn score_decoding() {
    let d = decode_score::<Frustration>(&[0.1, 0.2, 0.7]).unwrap();
    assert_eq!((d.value, d.confidence), (Frustration::Furious, 0.7));
    assert_eq!(
        decode_score::<Frustration>(&[0.5, 0.5]).unwrap_err(),
        DecodeError::WrongLevelCount {
            expected: 3,
            found: 2
        }
    );
    let e = expected_normalized::<Frustration>(&[0.0, 0.5, 0.5]).unwrap();
    assert!((e - 0.75).abs() < 1e-9);
}

#[test]
fn noul_specs_in_either_order() {
    let s = Urgent::SPEC;
    assert_eq!(
        (s.name(), s.yes(), s.no()),
        ("urgent", "There is a deadline", "Routine")
    );
    assert!(Urgent::Yes.as_bool());
    assert_eq!(Urgent::from_bool(false), Urgent::No);

    assert_eq!(Spam::SPEC.name(), "Spam");
    assert_eq!((Spam::SPEC.yes(), Spam::SPEC.no()), ("", ""));
    assert!(Spam::Junk.as_bool());
    assert_eq!(Spam::from_bool(false), Spam::Ham);
}

#[test]
fn noul_decoding_and_confidence_gate() {
    let d = decode_noul::<Urgent>(0.2).unwrap();
    assert_eq!(d.value, Urgent::No);
    assert!((d.confidence - 0.8).abs() < 1e-9);
    assert!(decode_noul::<Urgent>(f64::NAN).is_err());

    let unsure = decode_noul::<Urgent>(0.6)
        .unwrap()
        .into_confident(0.9)
        .unwrap_err();
    assert_eq!(unsure.value, Urgent::Yes);
    assert_eq!(unsure.into_confident(0.5), Ok(Urgent::Yes));
}

#[test]
fn spec_is_const_usable() {
    const SPEC: jevkit::ChoiceSpec = Team::SPEC;
    assert_eq!(SPEC.options().len(), 3);
}
