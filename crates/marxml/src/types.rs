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

    /// Query this element's subtree with a compiled selector.
    ///
    /// Returns matches within the element's descendants, in source order.
    pub fn select(&self, sel: &crate::Selector) -> impl Iterator<Item = ElementRef<'a>> + 'a {
        crate::selector::select(&self.data.children, self.raw, sel).into_iter()
    }

    /// Inner text segments, in source order, with child element markup stripped.
    ///
    /// For `<task>do <em>thing</em> now</task>`, this yields `"do "`,
    /// `" now"` (the text between child element open tags, plus the
    /// suffix after the last child).
    ///
    /// Returns an empty iterator for self-closing tags.
    pub fn text(&self) -> impl Iterator<Item = &'a str> + 'a {
        TextSegments::new(self.raw, self.data)
    }
}

/// Iterator over `ElementRef::text()` — the segments of raw text inside an
/// element, with child element markup omitted.
pub struct TextSegments<'a> {
    raw: &'a str,
    cursor: usize,
    end: usize,
    children: core::slice::Iter<'a, ElementData>,
}

impl<'a> TextSegments<'a> {
    fn new(raw: &'a str, data: &'a ElementData) -> Self {
        Self {
            raw,
            cursor: data.content_range.start,
            end: data.content_range.end,
            children: data.children.iter(),
        }
    }
}

impl<'a> Iterator for TextSegments<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(child) = self.children.next() {
                let next_start = usize::try_from(child.span.start.offset).unwrap_or(usize::MAX);
                let segment = &self.raw[self.cursor..next_start];
                self.cursor = usize::try_from(child.span.end.offset).unwrap_or(usize::MAX);
                if !segment.is_empty() {
                    return Some(segment);
                }
                continue;
            }
            // No more children. Yield remaining tail text, if any. By
            // construction, `cursor < end` here means the slice is non-empty.
            if self.cursor < self.end {
                let segment = &self.raw[self.cursor..self.end];
                self.cursor = self.end;
                return Some(segment);
            }
            return None;
        }
    }
}
