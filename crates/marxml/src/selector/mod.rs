//! CSS-subset selectors.
//!
//! See [`Selector::parse`] for the supported grammar.

mod ast;
mod error;
mod matcher;
mod parser;

pub use error::{SelectorError, SyntaxKind};

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
    ///
    /// ```
    /// use marxml::{parse, Selector};
    ///
    /// let doc = parse(r#"<task id="a"/><task id="b"/><note/>"#)?;
    /// let sel = Selector::parse("task, note")?;
    /// assert_eq!(doc.select(&sel).count(), 3);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn parse(input: &str) -> Result<Self, SelectorError> {
        Ok(Self {
            compiled: parser::parse(input)?,
        })
    }
}

impl std::str::FromStr for Selector {
    type Err = SelectorError;

    /// Equivalent to [`Selector::parse`]. Lets callers use the standard
    /// `"selector-string".parse::<Selector>()` form.
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::parse(input)
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
