//! Errors from [`Selector::parse`](super::Selector::parse).

use thiserror::Error;

/// An error returned when a selector string fails to parse.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum SelectorError {
    /// Selector string was empty.
    #[error("selector is empty")]
    Empty,

    /// Unexpected end of input.
    #[error("unexpected end of selector")]
    UnexpectedEnd,

    /// Syntax error at byte offset `at`. `kind` discriminates the specific
    /// failure mode.
    #[error("syntax error at offset {at}: {kind}")]
    Syntax {
        /// Specific failure mode.
        kind: SyntaxKind,
        /// 0-based byte offset within the selector string.
        at: usize,
    },
}

/// Specific kinds of [`SelectorError::Syntax`].
///
/// The size/depth-cap variants are machine-actionable: a caller can match
/// them to suggest "your selector exceeded our limits". The structural
/// `Expected` variant covers the long tail of "parser wanted token X next"
/// cases — `what` is a `&'static str` we control, not interpolated user
/// input.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum SyntaxKind {
    /// Selector union (`a, b, c, …`) exceeded the implementation cap.
    #[error("selector union exceeds maximum size of {max}")]
    UnionTooLarge {
        /// Implementation cap on union size.
        max: usize,
    },
    /// Compound chain (`a b c …`) exceeded the implementation cap.
    #[error("compound chain exceeds maximum length of {max}")]
    CompoundTooLong {
        /// Implementation cap on compound length.
        max: usize,
    },
    /// A single simple selector carried more predicates (`[…]` / `:…`) than
    /// the implementation cap allows.
    #[error("simple selector carries more than {max} predicates")]
    TooManyPredicates {
        /// Implementation cap on predicates per simple selector.
        max: usize,
    },
    /// `:not(:not(…))` nested deeper than the implementation cap allows.
    #[error(":not nesting exceeds maximum of {max}")]
    NotNestingTooDeep {
        /// Implementation cap on `:not` nesting depth.
        max: u32,
    },
    /// `:foo` where `foo` is not a pseudo-class this engine implements.
    #[error("unsupported pseudo-class :{name}")]
    UnsupportedPseudoClass {
        /// Pseudo-class identifier that follows the `:`.
        name: String,
    },
    /// `:nth-child(N)` was called with `N == 0`; siblings are 1-indexed.
    #[error(":nth-child argument must be 1 or greater (siblings are 1-indexed)")]
    NthChildMustBeOneOrGreater,
    /// An integer literal exceeded `u32::MAX` or its scan budget.
    #[error("integer out of range")]
    IntegerOutOfRange,
    /// Expected an ASCII digit but found another byte.
    #[error("expected digit")]
    ExpectedDigit,
    /// Parser expected a specific token or structure here. `what` describes
    /// the expectation in human terms — a fixed `&'static str` written by
    /// this crate, never interpolated runtime input.
    #[error("expected {what}")]
    Expected {
        /// Human description of the expected token, e.g. `"','"`, `"']'"`,
        /// `"combinator or ','"`, `"attribute name after '['"`.
        what: &'static str,
    },
}
