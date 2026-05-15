//! CSS-subset selectors.
//!
//! See [`Selector::parse`] for the supported grammar.

mod ast;
mod error;
mod matcher;
mod parser;

pub use error::SelectorError;

use crate::types::{ElementData, ElementRef};

/// A compiled CSS-subset selector.
///
/// Parse once with [`Selector::parse`], then reuse against many documents.
///
/// # Supported grammar
///
/// ```text
/// selector    := compound ( "," compound )*
/// compound    := simple ( ( " " | " > " ) simple )*
/// simple      := ( tag | "*" ) predicate*
/// predicate   := "[" name ( ( "=" | "^=" | "$=" | "*=" ) "value" )? "]"
///              | ":first-child"
///              | ":nth-child(" n ")"
///              | ":not(" simple ")"
/// ```
///
/// Not yet supported: sibling combinators (`~`, `+`), `:has()`, attribute
/// negation operators (`~=`, `|=`), and case-insensitive flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selector {
    compiled: ast::CompiledSelector,
}

impl Selector {
    /// Compile a selector string.
    ///
    /// # Errors
    ///
    /// Returns [`SelectorError`] if the string is empty, ends unexpectedly,
    /// or contains a syntax error.
    pub fn parse(input: &str) -> Result<Self, SelectorError> {
        Ok(Self {
            compiled: parser::parse(input)?,
        })
    }
}

pub(crate) fn select<'a>(
    roots: &'a [ElementData],
    raw: &'a str,
    trivia: &'a [core::ops::Range<usize>],
    sel: &Selector,
) -> Vec<ElementRef<'a>> {
    matcher::collect_matches(roots, raw, trivia, &sel.compiled)
}
