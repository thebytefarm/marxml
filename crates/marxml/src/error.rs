//! Error types produced by parsing.

use thiserror::Error;

/// Errors returned from [`crate::parse`] and [`crate::parse_fragment`].
///
/// Every variant carries a 1-based `line` number pointing at the offending
/// position in the source document. The error's `Display` impl produces a
/// human-readable message that includes the line number.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ParseError {
    /// An opening tag had no matching closing tag.
    #[error("line {line}: <{tag}> has no matching </{tag}>")]
    UnclosedTag {
        /// Tag name that was never closed.
        tag: String,
        /// 1-based line of the opening tag.
        line: u32,
    },

    /// A closing tag did not match the most recent open tag.
    #[error("line {line}: </{found}> does not match the most recent open tag <{expected}>")]
    MismatchedClose {
        /// Tag name that was found in the closing position.
        found: String,
        /// Tag name expected based on the most recent open tag.
        expected: String,
        /// 1-based line of the closing tag.
        line: u32,
    },

    /// A closing tag appeared with no corresponding open tag.
    #[error("line {line}: </{tag}> has no matching open tag")]
    StrayClose {
        /// Tag name in the closing position.
        tag: String,
        /// 1-based line of the closing tag.
        line: u32,
    },

    /// A tag was malformed — for example, an unterminated `<` at end of input
    /// or an unquoted attribute value.
    #[error("line {line}: malformed tag — {reason}")]
    MalformedTag {
        /// Short description of what went wrong.
        reason: String,
        /// 1-based line of the malformed tag.
        line: u32,
    },

    /// An element's attribute value was malformed (e.g. missing closing quote).
    #[error("line {line}: malformed attribute on <{tag}> — {reason}")]
    MalformedAttribute {
        /// Tag name carrying the attribute.
        tag: String,
        /// Short description of what went wrong.
        reason: String,
        /// 1-based line of the offending attribute.
        line: u32,
    },

    /// Two sibling elements with the same tag carried the same `id` attribute.
    #[error("line {line}: duplicate id=\"{id}\" on <{tag}>")]
    DuplicateId {
        /// Tag name where the duplicate was detected.
        tag: String,
        /// The repeated `id` value.
        id: String,
        /// 1-based line of the duplicate.
        line: u32,
    },
}

impl ParseError {
    /// 1-based line number where the error was detected.
    #[must_use]
    pub fn line(&self) -> u32 {
        match self {
            Self::UnclosedTag { line, .. }
            | Self::MismatchedClose { line, .. }
            | Self::StrayClose { line, .. }
            | Self::MalformedTag { line, .. }
            | Self::MalformedAttribute { line, .. }
            | Self::DuplicateId { line, .. } => *line,
        }
    }
}
