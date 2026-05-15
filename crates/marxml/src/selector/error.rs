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

    /// Generic syntax error at byte offset `at`.
    #[error("syntax error at offset {at}: {reason}")]
    Syntax {
        /// Short description of what went wrong.
        reason: String,
        /// 0-based byte offset within the selector string.
        at: usize,
    },
}
