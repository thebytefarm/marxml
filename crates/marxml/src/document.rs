//! The [`Markdown`] document — the result of [`crate::parse`].

use regex::Regex;

use crate::mutate;
use crate::selector::{self, Selector};
use crate::types::{ElementData, ElementRef};

/// A parsed markdown + embedded XML document.
///
/// Returned by [`crate::parse`] / [`crate::parse_fragment`]. Holds the
/// original source, the parsed element tree, and the byte ranges of any
/// XML trivia (comments, CDATA sections) the parser skipped — those ranges
/// are excluded from `text()` so consumers don't see comment markers as
/// content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Markdown {
    raw: String,
    roots: Vec<ElementData>,
    trivia: Vec<core::ops::Range<usize>>,
}

impl std::str::FromStr for Markdown {
    type Err = crate::ParseError;

    /// Equivalent to [`crate::parse`]. Lets callers use the standard
    /// `"...".parse::<Markdown>()` form.
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        crate::parse(input)
    }
}

impl Markdown {
    pub(crate) fn from_parts(
        raw: String,
        roots: Vec<ElementData>,
        trivia: Vec<core::ops::Range<usize>>,
    ) -> Self {
        Self { raw, roots, trivia }
    }

    /// The original document source, byte-for-byte.
    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Iterate the top-level (root) elements of the document, in source order.
    pub fn root_elements(&self) -> impl Iterator<Item = ElementRef<'_>> + '_ {
        let raw: &str = &self.raw;
        let trivia: &[core::ops::Range<usize>] = &self.trivia;
        self.roots
            .iter()
            .map(move |data| ElementRef { data, raw, trivia })
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
        selector::select(&self.roots, &self.raw, &self.trivia, sel).into_iter()
    }

    /// Update or insert attributes on every element matching `sel`. Returns
    /// the new raw document. The original [`Markdown`] is unchanged.
    ///
    /// If an attribute name in `new_attrs` is already present on a matched
    /// element, its value is replaced. Otherwise the attribute is appended
    /// at the end of the element's attribute list.
    ///
    /// The rewritten opening tag uses canonical whitespace: a single space
    /// between attributes, with the closing `>` (or `/>`) attached. Authors
    /// of pretty-printed source may notice spacing changes on touched tags.
    ///
    /// Use [`crate::escape_attr`] when the value contains user-controlled
    /// bytes — `update` escapes for you, but the helper documents that
    /// intent at the call site.
    ///
    /// # Panics
    ///
    /// Panics when `new_attrs` contains an entry whose name is not a valid
    /// XML name (see [`crate::is_valid_name`]) or repeats an earlier name.
    /// Both conditions are programmer errors; use [`Self::try_update`] for
    /// runtime-sourced attribute slices that may carry bad input.
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
    ///
    /// `replacement` is written verbatim; `$1` / `$name` / `${name}` are not
    /// interpreted as capture references.
    #[must_use]
    pub fn replace_in(&self, sel: &Selector, pattern: &Regex, replacement: &str) -> String {
        mutate::replace_in(self, sel, pattern, replacement)
    }

    /// Like [`Self::replace_content`], but `new_body` is run through
    /// [`crate::escape_text`] before being spliced. Use this for replacement
    /// strings sourced from untrusted text.
    #[must_use]
    pub fn replace_text(&self, sel: &Selector, new_body: &str) -> String {
        mutate::replace_text(self, sel, new_body)
    }

    /// Like [`Self::replace_in`], but `replacement` is run through
    /// [`crate::escape_text`] before being spliced.
    #[must_use]
    pub fn replace_text_in(&self, sel: &Selector, pattern: &Regex, replacement: &str) -> String {
        mutate::replace_text_in(self, sel, pattern, replacement)
    }

    /// Fallible variant of [`Self::update`]. Returns a [`crate::MutationReport`]
    /// (with the rewritten document and applied/skipped counts) on success,
    /// or a [`crate::MutateError`] on programmer error (invalid XML name or
    /// duplicate key in `new_attrs`).
    ///
    /// # Errors
    ///
    /// See [`crate::MutateError`].
    pub fn try_update(
        &self,
        sel: &Selector,
        new_attrs: &[(&str, &str)],
    ) -> Result<crate::MutationReport, crate::MutateError> {
        mutate::try_update(self, sel, new_attrs)
    }

    /// Like [`Self::replace_content`] but returns a [`crate::MutationReport`]
    /// so callers can see how many matches were applied vs. skipped because
    /// of overlap with an outer match. Never fails — the report carries the
    /// rewritten output alongside the counts.
    #[must_use]
    pub fn replace_content_report(&self, sel: &Selector, new_body: &str) -> crate::MutationReport {
        mutate::try_replace_content(self, sel, new_body)
    }

    /// Like [`Self::replace_in`] but returns a [`crate::MutationReport`].
    /// Never fails — the report carries the rewritten output alongside the
    /// applied/skipped counts.
    #[must_use]
    pub fn replace_in_report(
        &self,
        sel: &Selector,
        pattern: &Regex,
        replacement: &str,
    ) -> crate::MutationReport {
        mutate::try_replace_in(self, sel, pattern, replacement)
    }

    /// Serialize the parsed XML elements back to a flat XML string.
    ///
    /// Surrounding markdown text is dropped — this is just the structured
    /// payload. Pass [`crate::SerializeOpts::pretty`] for indented multi-line output.
    #[must_use]
    pub fn to_xml(&self, opts: &crate::SerializeOpts) -> String {
        crate::serialize::to_xml(self, opts)
    }

    /// Serialize the element tree as a `serde_json::Value`.
    ///
    /// Top-level result is an array of root elements. Each element is an
    /// object with these fields:
    /// - `tag` — element tag name
    /// - `attrs` — object of attribute key/value pairs (values decoded)
    /// - `text` — direct text content of the element, with child-element
    ///   markup excluded. Decoded entity references appear as their literal
    ///   characters.
    /// - `children` — array of recursively-serialized child elements
    /// - `selfClosing` — `true` for `<tag/>`, `false` for `<tag>…</tag>`
    /// - `location` — `{start: {line, offset}, end: {line, offset}}`
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        crate::serialize::to_json(self)
    }

    /// Crate-internal accessor for the parsed root elements.
    pub(crate) fn roots_internal(&self) -> &[ElementData] {
        &self.roots
    }

    /// Crate-internal accessor for the trivia (comment + CDATA) byte ranges.
    pub(crate) fn trivia(&self) -> &[core::ops::Range<usize>] {
        &self.trivia
    }
}
