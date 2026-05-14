//! The [`Markdown`] document — the result of [`crate::parse`].

use regex::Regex;

use crate::mutate;
use crate::selector::{self, Selector};
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

    /// Query the document with a compiled selector.
    ///
    /// Returns every matching element in source order. Each element appears
    /// at most once even when multiple compounds in a union would match it.
    ///
    /// ```
    /// let doc = marxml::parse(r#"<task id="1"/><task id="2"/>"#)?;
    /// let sel = marxml::Selector::parse("task")?;
    /// let tasks: Vec<_> = doc.select(&sel).collect();
    /// assert_eq!(tasks.len(), 2);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn select(&self, sel: &Selector) -> impl Iterator<Item = ElementRef<'_>> {
        selector::select(&self.roots, &self.raw, sel).into_iter()
    }

    /// Update or insert attributes on every element matching `sel`. Returns
    /// the new raw document. The original [`Markdown`] is unchanged.
    ///
    /// If an attribute name in `new_attrs` is already present on a matched
    /// element, its value is replaced. Otherwise the attribute is appended
    /// at the end of the element's attribute list.
    #[must_use]
    pub fn update(&self, sel: &Selector, new_attrs: &[(&str, &str)]) -> String {
        mutate::update(self, sel, new_attrs)
    }

    /// Replace the inner content of every element matching `sel` with
    /// `new_body`. Returns the new raw document.
    #[must_use]
    pub fn replace_content(&self, sel: &Selector, new_body: &str) -> String {
        mutate::replace_content(self, sel, new_body)
    }

    /// Run a regex `replace_all` over the inner content of every element
    /// matching `sel`. Returns the new raw document.
    #[must_use]
    pub fn replace_in(&self, sel: &Selector, pattern: &Regex, replacement: &str) -> String {
        mutate::replace_in(self, sel, pattern, replacement)
    }

    /// Serialize the parsed XML elements back to a flat XML string.
    ///
    /// Surrounding markdown text is dropped — this is just the structured
    /// payload. Pass [`SerializeOpts::pretty`] for indented multi-line output.
    #[must_use]
    pub fn to_xml(&self, opts: &crate::SerializeOpts) -> String {
        crate::serialize::to_xml(self, opts)
    }

    /// Serialize the element tree as a `serde_json::Value`.
    ///
    /// Top-level result is an array of root elements. Each element is an
    /// object with `tag`, `attrs`, `content`, `children`, `selfClosing`, and
    /// `location` fields.
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        crate::serialize::to_json(self)
    }

    /// Crate-internal accessor for the parsed root elements.
    pub(crate) fn roots_internal(&self) -> &[ElementData] {
        &self.roots
    }
}
