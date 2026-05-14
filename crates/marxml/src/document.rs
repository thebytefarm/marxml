//! The [`Markdown`] document — the result of [`crate::parse`].

use crate::types::{ElementData, ElementRef};

/// A parsed markdown + embedded XML document.
///
/// Returned by [`crate::parse`] / [`crate::parse_fragment`]. Holds the
/// original source and the parsed element tree. Query methods (added in
/// Phase 3) return [`ElementRef`] handles that borrow from this document.
#[derive(Debug, Clone)]
pub struct Markdown {
    raw: String,
    roots: Vec<ElementData>,
}

impl Markdown {
    pub(crate) fn from_parts(raw: String, roots: Vec<ElementData>) -> Self {
        Self { raw, roots }
    }

    /// The original document source, byte-for-byte.
    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Iterate the top-level (root) elements of the document, in source order.
    pub fn root_elements(&self) -> impl Iterator<Item = ElementRef<'_>> + '_ {
        let raw: &str = &self.raw;
        self.roots.iter().map(move |data| ElementRef { data, raw })
    }

    /// Count of top-level elements.
    #[must_use]
    pub fn root_count(&self) -> usize {
        self.roots.len()
    }
}
