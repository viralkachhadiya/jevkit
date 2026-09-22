//! Declarative macros that define Jev questions as enums.
//!
//! These replace `#[derive]` so the whole library ships as a single crate
//! (derive macros must live in a separate proc-macro crate).

#[doc(hidden)]
#[macro_export]
macro_rules! __jev_name {
    ([$n:literal] $ident:ident) => {
        $n
    };
    ([] $ident:ident) => {
        stringify!($ident)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __jev_label {
    ([$l:literal] $variant:ident) => {
        $l
    };
    ([] $variant:ident) => {
        stringify!($variant)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __jev_text {
    ([$t:literal] [$($doc:literal)*]) => {
        $t
    };
    ([] [$($doc:literal)*]) => {
        concat!($($doc),*)
    };
}

/// Define a pick-one question as an enum.
///
/// The enum's doc comment is the question. Each variant's doc comment is
/// what that option means. A variant's label is its name unless you write
/// `Variant = "label"`.
///
/// ```
/// use jevkit::Choice;
///
/// jevkit::choice! {
///     /// Which team should handle this ticket?
///     pub enum Department {
///         /// Payments, invoicing, refunds
///         Billing,
///         /// Bugs, outages, integrations
///         Technical = "tech",
///         Other,
///     }
/// }
///
/// let spec = Department::SPEC;
/// assert_eq!(spec.name(), "Department");
/// assert_eq!(spec.question(), "Which team should handle this ticket?");
/// assert_eq!(spec.options()[0].label(), "Billing");
/// assert_eq!(spec.options()[0].description(), "Payments, invoicing, refunds");
/// assert_eq!(Department::from_label("tech"), Some(Department::Technical));
/// assert_eq!(Department::Other.label(), "Other");
/// ```
///
/// Optional, in this order and all before `enum`: `#[jev(name = "id")]` sets
/// the question id (default: the enum name), and `#[derive(...)]` adds
/// derives on top of `Debug, Clone, Copy, PartialEq, Eq, Hash`.
///
/// # Compile-time checks
///
/// A missing question, fewer than 2 or more than 255 variants, and duplicate
/// labels are compile errors:
///
/// ```compile_fail
/// jevkit::choice! {
///     /// Pick one
///     enum OnlyOne { A }
/// }
/// ```
///
/// ```compile_fail
/// jevkit::choice! {
///     /// Pick one
///     enum Dupes { A = "x", B = "x" }
/// }
/// ```
///
/// ```compile_fail
/// jevkit::choice! {
///     enum NoQuestion { A, B }
/// }
/// ```
///
/// The enum also gets a `VARIANTS` constant listing every variant in order.
#[macro_export]
macro_rules! choice {
    (
        $(#[doc = $qdoc:literal])*
        $(#[jev(name = $name:literal)])?
        $(#[derive($($extra:path),* $(,)?)])*
        $vis:vis enum $ident:ident {
            $(
                $(#[doc = $vdoc:literal])*
                $variant:ident $(= $label:literal)?
            ),+ $(,)?
        }
    ) => {
        $(#[doc = $qdoc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash $($(, $extra)*)*)]
        $vis enum $ident {
            $( $(#[doc = $vdoc])* $variant ),+
        }

        impl $ident {
            /// Every variant, in declaration order.
            pub const VARIANTS: &'static [Self] = &[ $(Self::$variant),+ ];
        }

        impl $crate::Choice for $ident {
            const SPEC: $crate::ChoiceSpec = $crate::ChoiceSpec::new(
                $crate::__jev_name!([$($name)?] $ident),
                concat!($($qdoc),*),
                &[ $(
                    $crate::ChoiceOption::new(
                        $crate::__jev_label!([$($label)?] $variant),
                        concat!($($vdoc),*),
                    )
                ),+ ],
            );

            fn label(&self) -> &'static str {
                match self {
                    $( Self::$variant => $crate::__jev_label!([$($label)?] $variant) ),+
                }
            }

            fn from_label(label: &str) -> ::core::option::Option<Self> {
                $(
                    if label == $crate::__jev_label!([$($label)?] $variant) {
                        return ::core::option::Option::Some(Self::$variant);
                    }
                )+
                ::core::option::Option::None
            }
        }

        const _: () = $crate::__private::check_choice(&<$ident as $crate::Choice>::SPEC);
    };
}

/// Define an ordered rating question as an enum. Variants are the levels,
/// lowest first.
///
/// The enum's doc comment is the question. A level's meaning is its doc
/// comment, or `Variant = "meaning"`, or (failing both) its name.
///
/// ```
/// use jevkit::Score;
///
/// jevkit::score! {
///     /// How frustrated is the customer?
///     pub enum Frustration {
///         Calm,
///         Frustrated,
///         Angry = "Very angry",
///     }
/// }
///
/// let levels = Frustration::SPEC.levels();
/// assert_eq!(levels[1].description(), "Frustrated");
/// assert_eq!(levels[2].description(), "Very angry");
/// assert_eq!(Frustration::Angry.index(), 2);
/// assert_eq!(Frustration::from_index(1), Some(Frustration::Frustrated));
/// assert_eq!(Frustration::Frustrated.normalized(), 0.5);
/// ```
///
/// Fewer than 2 or more than 10 levels is a compile error:
///
/// ```compile_fail
/// jevkit::score! {
///     /// Too few
///     enum One { Only }
/// }
/// ```
///
/// Accepts the same optional `#[jev(name = "id")]` and `#[derive(...)]`
/// attributes as [`choice!`](crate::choice).
#[macro_export]
macro_rules! score {
    (
        $(#[doc = $qdoc:literal])*
        $(#[jev(name = $name:literal)])?
        $(#[derive($($extra:path),* $(,)?)])*
        $vis:vis enum $ident:ident {
            $(
                $(#[doc = $vdoc:literal])*
                $variant:ident $(= $text:literal)?
            ),+ $(,)?
        }
    ) => {
        $(#[doc = $qdoc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash $($(, $extra)*)*)]
        $vis enum $ident {
            $( $(#[doc = $vdoc])* $variant ),+
        }

        impl $ident {
            /// Every level, lowest first.
            pub const VARIANTS: &'static [Self] = &[ $(Self::$variant),+ ];
        }

        impl $crate::Score for $ident {
            const SPEC: $crate::ScoreSpec = $crate::ScoreSpec::new(
                $crate::__jev_name!([$($name)?] $ident),
                concat!($($qdoc),*),
                &[ $(
                    $crate::ScoreLevel::new(
                        stringify!($variant),
                        $crate::__jev_text!([$($text)?] [$($vdoc)*]),
                    )
                ),+ ],
            );

            fn index(&self) -> usize {
                *self as usize
            }

            fn from_index(index: usize) -> ::core::option::Option<Self> {
                Self::VARIANTS.get(index).copied()
            }
        }

        const _: () = $crate::__private::check_score(&<$ident as $crate::Score>::SPEC);
    };
}

/// Define a yes/no question as an enum with exactly two variants, one marked
/// `yes` and one marked `no` (either order).
///
/// The enum's doc comment is the question. Each variant's doc comment says
/// what that answer means (optional).
///
/// ```
/// use jevkit::Noul;
///
/// jevkit::noul! {
///     /// Does this convey urgency?
///     #[jev(name = "is_urgent")]
///     pub enum Urgency {
///         /// Explicitly time-sensitive
///         yes Urgent,
///         /// No urgency expressed
///         no Routine,
///     }
/// }
///
/// assert_eq!(Urgency::SPEC.name(), "is_urgent");
/// assert_eq!(Urgency::SPEC.yes(), "Explicitly time-sensitive");
/// assert!(Urgency::Urgent.as_bool());
/// assert_eq!(Urgency::from_bool(false), Urgency::Routine);
/// ```
///
/// Accepts the same optional `#[jev(name = "id")]` and `#[derive(...)]`
/// attributes as [`choice!`](crate::choice).
#[macro_export]
macro_rules! noul {
    // `yes` variant first.
    (
        $(#[doc = $qdoc:literal])*
        $(#[jev(name = $name:literal)])?
        $(#[derive($($extra:path),* $(,)?)])*
        $vis:vis enum $ident:ident {
            $(#[doc = $ydoc:literal])* yes $yes:ident,
            $(#[doc = $ndoc:literal])* no $no:ident $(,)?
        }
    ) => {
        $(#[doc = $qdoc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash $($(, $extra)*)*)]
        $vis enum $ident {
            $(#[doc = $ydoc])* $yes,
            $(#[doc = $ndoc])* $no,
        }
        $crate::__jev_noul_impl! {
            $ident, [$($name)?], [$($qdoc)*], $yes [$($ydoc)*], $no [$($ndoc)*]
        }
    };
    // `no` variant first.
    (
        $(#[doc = $qdoc:literal])*
        $(#[jev(name = $name:literal)])?
        $(#[derive($($extra:path),* $(,)?)])*
        $vis:vis enum $ident:ident {
            $(#[doc = $ndoc:literal])* no $no:ident,
            $(#[doc = $ydoc:literal])* yes $yes:ident $(,)?
        }
    ) => {
        $(#[doc = $qdoc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash $($(, $extra)*)*)]
        $vis enum $ident {
            $(#[doc = $ndoc])* $no,
            $(#[doc = $ydoc])* $yes,
        }
        $crate::__jev_noul_impl! {
            $ident, [$($name)?], [$($qdoc)*], $yes [$($ydoc)*], $no [$($ndoc)*]
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __jev_noul_impl {
    (
        $ident:ident, [$($name:literal)?], [$($qdoc:literal)*],
        $yes:ident [$($ydoc:literal)*], $no:ident [$($ndoc:literal)*]
    ) => {
        impl $crate::Noul for $ident {
            const SPEC: $crate::NoulSpec = $crate::NoulSpec::new(
                $crate::__jev_name!([$($name)?] $ident),
                concat!($($qdoc),*),
                concat!($($ydoc),*),
                concat!($($ndoc),*),
            );

            fn as_bool(&self) -> bool {
                match self {
                    Self::$yes => true,
                    Self::$no => false,
                }
            }

            fn from_bool(value: bool) -> Self {
                if value { Self::$yes } else { Self::$no }
            }
        }

        const _: () = $crate::__private::check_noul(&<$ident as $crate::Noul>::SPEC);
    };
}
