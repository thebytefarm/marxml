//! Core data types used across the crate.

use core::ops::Range;

/// A position in the source document.
///
/// `line` is 1-based to match what humans (and most editors) expect.
/// `offset` is the 0-based byte offset from the start of the document.
///
/// `PartialOrd`/`Ord` compare lexicographically by `(line, offset)` —
/// earlier-in-source first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourcePosition {
    /// 1-based line number.
    pub line: u32,
    /// 0-based byte offset from the start of the document.
    pub offset: u32,
}

impl SourcePosition {
    /// Byte offset as `usize`. The widening conversion is infallible on every
    /// target that supports `std` (pointer width ≥ 32 bits), and the source
    /// document is bounded to `u32::MAX` bytes at parse entry, so any value
    /// stored in `offset` fits.
    #[inline]
    pub(crate) fn offset_usize(self) -> usize {
        self.offset as usize
    }
}

/// A half-open span of source positions: `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
/// Cloning is free (it's a small handful of pointers). Methods return
/// references into the original document where possible.
#[derive(Debug, Clone, Copy)]
pub struct ElementRef<'a> {
    pub(crate) data: &'a ElementData,
    pub(crate) raw: &'a str,
    /// Trivia (comments + CDATA) byte ranges from the owning document, in
    /// ascending source order. Threaded onto every `ElementRef` so `text()`
    /// can exclude trivia bytes even without going back to the `Markdown`.
    pub(crate) trivia: &'a [core::ops::Range<usize>],
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

    /// Byte range of the inner content within the document's raw string.
    ///
    /// Crate-internal so mutation can splice content without recomputing
    /// the offsets from element-span arithmetic.
    pub(crate) fn content_range(&self) -> core::ops::Range<usize> {
        self.data.content_range.clone()
    }

    /// Iterate the element's direct children.
    pub fn children(&self) -> impl Iterator<Item = ElementRef<'a>> + 'a {
        let raw = self.raw;
        let trivia = self.trivia;
        self.data.children.iter().map(move |child| ElementRef {
            data: child,
            raw,
            trivia,
        })
    }

    /// Source span covering the full element (opening tag through closing tag,
    /// or the entire self-closing tag).
    ///
    /// Returned by value — [`SourceSpan`] is `Copy` and small.
    #[must_use]
    pub fn location(&self) -> SourceSpan {
        self.data.span
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
        crate::selector::select(&self.data.children, self.raw, self.trivia, sel).into_iter()
    }

    /// Inner text segments, in source order, with child element markup
    /// stripped. Comment / CDATA bytes are also excluded — `text()` reflects
    /// the user-authored content, not the document's raw bytes.
    ///
    /// For `<task>do <em>thing</em> now</task>`, this yields `"do "`,
    /// `" now"` (the text between child element open tags, plus the
    /// suffix after the last child).
    ///
    /// Returns an empty iterator for self-closing tags.
    pub fn text(&self) -> impl Iterator<Item = &'a str> + 'a {
        TextSegments::new_with_trivia(self.raw, self.data, self.trivia)
    }
}

/// Iterator over `ElementRef::text()` — the segments of raw text inside an
/// element, with child element markup omitted. Trivia byte ranges
/// (comments + CDATA produced by the parser) are also skipped so callers
/// never see comment markers as content.
///
/// The `trivia` slice is a sorted, non-overlapping list shared across the
/// whole document; the iterator advances a monotonic index into it so
/// iteration over text segments stays linear in `(children + trivia)`
/// rather than `children × trivia`.
#[derive(Debug, Clone)]
pub(crate) struct TextSegments<'a> {
    raw: &'a str,
    cursor: usize,
    end: usize,
    children: core::slice::Iter<'a, ElementData>,
    trivia: &'a [core::ops::Range<usize>],
    /// Index of the next trivia range that might still overlap `cursor..end`.
    /// Trivia is sorted ascending, so this is monotonically non-decreasing.
    trivia_idx: usize,
}

impl<'a> TextSegments<'a> {
    pub(crate) fn new_with_trivia(
        raw: &'a str,
        data: &'a ElementData,
        trivia: &'a [core::ops::Range<usize>],
    ) -> Self {
        // Skip past any trivia that ends before this element's body — they
        // can never overlap, so we don't want to look at them again.
        let start = data.content_range.start;
        let trivia_idx = trivia.partition_point(|r| r.end <= start);
        Self {
            raw,
            cursor: start,
            end: data.content_range.end,
            children: data.children.iter(),
            trivia,
            trivia_idx,
        }
    }
}

impl<'a> Iterator for TextSegments<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Next child span (peeked, not consumed). Cloning a slice
            // iterator is zero-cost, but `as_slice().first()` reads as
            // "peek without advancing" without leaning on that detail.
            let child_next = self.children.as_slice().first().map(|c| {
                let s = c.span.start.offset_usize();
                let e = c.span.end.offset_usize();
                s..e
            });
            // Next trivia range that might still overlap the remaining body.
            // The index is monotonically non-decreasing across calls, so the
            // total work is O(children + relevant trivia), not their product.
            while self.trivia_idx < self.trivia.len()
                && self.trivia[self.trivia_idx].end <= self.cursor
            {
                self.trivia_idx += 1;
            }
            let trivia_next = self.trivia.get(self.trivia_idx).and_then(|r| {
                if r.start >= self.end {
                    None
                } else {
                    Some(r.clone())
                }
            });

            let pick = match (child_next, trivia_next) {
                (Some(c), Some(t)) if c.start <= t.start => {
                    self.children.next();
                    Some(c)
                }
                (Some(c), None) => {
                    self.children.next();
                    Some(c)
                }
                (_, Some(t)) => {
                    self.trivia_idx += 1;
                    Some(t)
                }
                (None, None) => None,
            };

            let Some(span) = pick else {
                if self.cursor < self.end {
                    let segment = &self.raw[self.cursor..self.end];
                    self.cursor = self.end;
                    return Some(segment);
                }
                return None;
            };
            let seg_end = span.start.min(self.end).max(self.cursor);
            let segment = &self.raw[self.cursor..seg_end];
            self.cursor = span.end.max(self.cursor).min(self.end);
            if !segment.is_empty() {
                return Some(segment);
            }
        }
    }
}
