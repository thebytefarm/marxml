//! Core data types used across the crate.

use core::ops::Range;

/// A position in the source document.
///
/// `line` is 1-based to match what humans (and most editors) expect.
/// `offset` is the 0-based byte offset from the start of the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourcePosition {
    /// 1-based line number.
    pub line: u32,
    /// 0-based byte offset from the start of the document.
    pub offset: u32,
}

/// A half-open span of source positions: `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceSpan {
    /// Inclusive start position.
    pub start: SourcePosition,
    /// Exclusive end position.
    pub end: SourcePosition,
}

/// Owned data backing every parsed element.
///
/// Stored once in the [`Markdown`](crate::Markdown) document tree. Callers do not
/// interact with `ElementData` directly — they use [`ElementRef`] which borrows
/// from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElementData {
    pub(crate) tag: String,
    /// Attributes in source order. Linear scan; tags rarely have many.
    pub(crate) attrs: Vec<(String, String)>,
    /// Byte range within the document's raw string covering this element's
    /// inner content (the body between opening and closing tags, or empty
    /// for self-closing tags).
    pub(crate) content_range: Range<usize>,
    /// Direct child elements, in source order.
    pub(crate) children: Vec<ElementData>,
    /// Span of the full element including its tags.
    pub(crate) span: SourceSpan,
    /// `true` for `<tag/>`; `false` for `<tag>…</tag>`.
    pub(crate) self_closing: bool,
}

/// A cheap reference to a parsed element, borrowing from the owning
/// [`Markdown`](crate::Markdown) document.
///
/// Cloning is free (it's two pointers). Methods return references into the
/// original document where possible.
#[derive(Debug, Clone, Copy)]
pub struct ElementRef<'a> {
    pub(crate) data: &'a ElementData,
    pub(crate) raw: &'a str,
}

impl<'a> ElementRef<'a> {
    /// The element's tag name.
    #[must_use]
    pub fn tag(&self) -> &'a str {
        &self.data.tag
    }

    /// Look up an attribute by name. Returns the attribute's value, or
    /// `None` if the element has no attribute with that name.
    #[must_use]
    pub fn attr(&self, name: &str) -> Option<&'a str> {
        self.data
            .attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// Iterate every attribute on this element in source order.
    pub fn attrs(&self) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
        self.data
            .attrs
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Inner content as a borrowed slice of the original document.
    ///
    /// For `<tag>body</tag>`, returns `"body"`. For self-closing tags,
    /// returns an empty string.
    #[must_use]
    pub fn content(&self) -> &'a str {
        &self.raw[self.data.content_range.clone()]
    }

    /// Iterate the element's direct children.
    pub fn children(&self) -> impl Iterator<Item = ElementRef<'a>> + 'a {
        let raw = self.raw;
        self.data
            .children
            .iter()
            .map(move |child| ElementRef { data: child, raw })
    }

    /// Source span covering the full element (opening tag through closing tag,
    /// or the entire self-closing tag).
    #[must_use]
    pub fn location(&self) -> &'a SourceSpan {
        &self.data.span
    }

    /// `true` if this element was written as `<tag/>` rather than
    /// `<tag>…</tag>`.
    #[must_use]
    pub fn is_self_closing(&self) -> bool {
        self.data.self_closing
    }
}
