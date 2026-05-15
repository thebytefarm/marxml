//! Error types produced by parsing.

use thiserror::Error;

/// Errors returned from [`crate::parse`] and [`crate::parse_fragment`].
///
/// Every variant carries a 1-based `line` number pointing at the offending
/// position in the source document. The error's `Display` impl produces a
/// human-readable message that includes the line number.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[non_exhaustive]
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

    /// Element nesting exceeded the configured maximum depth.
    ///
    /// Acts as a guard against pathological inputs (recursive walks could
    /// otherwise stack-overflow on adversarial documents).
    #[error("line {line}: <{tag}> exceeds maximum nesting depth of {max}")]
    MaxDepthExceeded {
        /// Tag that pushed past the limit.
        tag: String,
        /// Configured maximum depth.
        max: u32,
        /// 1-based line of the offending opening tag.
        line: u32,
    },

    /// Two attributes with the same name appeared on a single element.
    ///
    /// XML requires attribute names to be unique per element; lenient parsing
    /// would yield first/last/middle ambiguity in downstream consumers, so
    /// duplicates are rejected up front.
    #[error("line {line}: duplicate attribute {attr} on <{tag}>")]
    DuplicateAttr {
        /// Tag carrying the duplicate.
        tag: String,
        /// Repeated attribute name.
        attr: String,
        /// 1-based line of the duplicate.
        line: u32,
    },

    /// Input exceeded the maximum byte length the parser can address.
    ///
    /// Source byte offsets are stored as `u32`, so inputs larger than
    /// `u32::MAX` bytes (4 GiB - 1) cannot have their spans tracked accurately
    /// and are refused at parse time.
    #[error("input is {size} bytes — exceeds maximum of {max} bytes")]
    InputTooLarge {
        /// Byte length of the offending input.
        size: u64,
        /// Configured maximum byte length.
        max: u64,
    },
}

impl ParseError {
    /// 1-based line number where the error was detected, when one is
    /// available.
    ///
    /// Returns `None` for [`ParseError::InputTooLarge`] — that variant is
    /// raised before the input is scanned, so no source position exists.
    #[must_use]
    pub fn line(&self) -> Option<u32> {
        match self {
            Self::UnclosedTag { line, .. }
            | Self::MismatchedClose { line, .. }
            | Self::StrayClose { line, .. }
            | Self::MalformedTag { line, .. }
            | Self::MalformedAttribute { line, .. }
            | Self::DuplicateId { line, .. }
            | Self::MaxDepthExceeded { line, .. }
            | Self::DuplicateAttr { line, .. } => Some(*line),
            Self::InputTooLarge { .. } => None,
        }
    }
}
