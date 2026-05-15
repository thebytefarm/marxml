//! Serialization of parsed elements back to text or structured forms.
//!
//! Two output formats:
//! - [`Markdown::to_xml`] — concatenate every root element as XML, ignoring
//!   the surrounding markdown text. Useful when the host markdown is
//!   uninteresting and you just want the structured payload.
//! - [`Markdown::to_json`] — emit the element tree as a `serde_json::Value`,
//!   suitable for crossing process / language boundaries.
//!
//! Plus `Display` impls on both [`Markdown`] (returns the original raw
//! source) and [`ElementRef`] (returns the element's outer XML, byte-for-byte
//! from the source).

use serde_json::{json, Map, Value};

use crate::escape::{decode_entities, push_escaped_attr, push_escaped_text};
use crate::types::{ElementData, ElementRef, TextSegments};
use crate::Markdown;

/// Walk the element body, producing text segments while skipping child
/// elements AND the document's trivia (comments + CDATA byte ranges).
fn text_with_trivia<'a>(
    raw: &'a str,
    el: &'a ElementData,
    trivia: &'a [core::ops::Range<usize>],
) -> TextSegments<'a> {
    TextSegments::new_with_trivia(raw, el, trivia)
}

/// Options for [`Markdown::to_xml`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct SerializeOpts {
    /// Indentation string. When set, child elements are nested on their own
    /// lines with this prefix per level. `None` yields tight, single-line output.
    pub indent: Option<String>,
    /// When true, empty elements are emitted as self-closing tags
    /// (`<tag/>`). When false, they stay as `<tag></tag>` unless the source
    /// already used self-close syntax.
    pub self_close_empty: bool,
}

impl SerializeOpts {
    /// Pretty-print defaults: 2-space indentation, self-close empty tags.
    #[must_use]
    pub fn pretty() -> Self {
        Self {
            indent: Some("  ".to_string()),
            self_close_empty: true,
        }
    }

    /// Set the indentation prefix; each nested child is prefixed with one
    /// copy of `indent` per level. Pair with [`Self::compact`] to switch
    /// back to single-line output.
    #[must_use]
    pub fn with_indent(mut self, indent: impl Into<String>) -> Self {
        self.indent = Some(indent.into());
        self
    }

    /// Disable indentation: emit everything on one line.
    #[must_use]
    pub fn compact(mut self) -> Self {
        self.indent = None;
        self
    }

    /// Collapse empty elements to `<tag/>`.
    #[must_use]
    pub fn self_close_empty(mut self) -> Self {
        self.self_close_empty = true;
        self
    }

    /// Keep empty elements as `<tag></tag>` (unless the source already used
    /// self-close form, which is always preserved).
    #[must_use]
    pub fn expand_empty(mut self) -> Self {
        self.self_close_empty = false;
        self
    }
}

pub(crate) fn to_xml(doc: &Markdown, opts: &SerializeOpts) -> String {
    let mut out = String::new();
    for (i, root) in doc.roots_internal().iter().enumerate() {
        if i > 0 && opts.indent.is_some() {
            out.push('\n');
        }
        emit_element(root, doc.raw(), doc.trivia(), opts, 0, &mut out);
    }
    out
}

pub(crate) fn to_json(doc: &Markdown) -> Value {
    Value::Array(
        doc.roots_internal()
            .iter()
            .map(|root| element_json(root, doc.raw(), doc.trivia()))
            .collect(),
    )
}

fn emit_element(
    el: &ElementData,
    raw: &str,
    trivia: &[core::ops::Range<usize>],
    opts: &SerializeOpts,
    depth: usize,
    out: &mut String,
) {
    indent_for(opts, depth, out);
    out.push('<');
    out.push_str(&el.tag);
    for (k, v) in &el.attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        push_escaped_attr(out, v);
        out.push('"');
    }
    let has_text = text_with_trivia(raw, el, trivia).any(|s| !s.trim().is_empty());
    let is_empty = el.children.is_empty() && !has_text;
    if is_empty && (el.self_closing || opts.self_close_empty) {
        out.push_str("/>");
        return;
    }
    out.push('>');
    if el.children.is_empty() {
        // Pure text body — escape so output is well-formed XML even when the
        // source slipped a literal `<` or `&` past the permissive tokenizer.
        // Goes through `push_escaped_text` so illegal control characters
        // are also dropped consistently with `crate::escape::escape_text`.
        for segment in text_with_trivia(raw, el, trivia) {
            push_escaped_text(out, segment);
        }
    } else if opts.indent.is_some() {
        emit_pretty_children(el, raw, trivia, opts, depth, out);
    } else {
        // Tight mode with children: re-emit children through this same
        // function (so their attrs/text escape consistently) interleaved
        // with the parent's direct text segments.
        emit_tight_children(el, raw, trivia, opts, depth, out);
    }
    out.push_str("</");
    out.push_str(&el.tag);
    out.push('>');
}

/// Emit tight-mode children: interleave escaped text segments with re-emitted
/// child elements, instead of copying the parent's raw body. Keeps round-trip
/// output well-formed even when the tokenizer accepted bytes XML doesn't.
///
/// Advances a monotonic `trivia_idx` over the (sorted) trivia slice so the
/// total cost is linear in `(children + trivia overlapping the body)`
/// rather than `children × trivia_total`.
fn emit_tight_children(
    el: &ElementData,
    raw: &str,
    trivia: &[core::ops::Range<usize>],
    opts: &SerializeOpts,
    depth: usize,
    out: &mut String,
) {
    let body_start = el.content_range.start;
    let body_end = el.content_range.end;
    let mut cursor = body_start;
    let mut trivia_idx = trivia.partition_point(|r| r.end <= body_start);
    for child in &el.children {
        let child_start = child.span.start.offset_usize();
        let segment_end = child_start.min(body_end);
        push_escaped_text_skipping_trivia(raw, cursor, segment_end, trivia, &mut trivia_idx, out);
        emit_element(child, raw, trivia, opts, depth + 1, out);
        cursor = child.span.end.offset_usize();
    }
    if cursor < body_end {
        push_escaped_text_skipping_trivia(raw, cursor, body_end, trivia, &mut trivia_idx, out);
    }
}

/// Append `raw[from..to]` to `out` with XML text escaping, but skip any
/// byte ranges in `trivia[trivia_idx..]` that overlap the segment. Advances
/// `trivia_idx` past any range fully consumed.
fn push_escaped_text_skipping_trivia(
    raw: &str,
    from: usize,
    to: usize,
    trivia: &[core::ops::Range<usize>],
    trivia_idx: &mut usize,
    out: &mut String,
) {
    let mut cursor = from;
    while cursor < to {
        while *trivia_idx < trivia.len() && trivia[*trivia_idx].end <= cursor {
            *trivia_idx += 1;
        }
        let tr = trivia.get(*trivia_idx);
        let plain_end = match tr {
            Some(r) if r.start < to => r.start.max(cursor),
            _ => to,
        };
        if cursor < plain_end {
            escape_into(&raw[cursor..plain_end], out);
        }
        match tr {
            Some(r) if r.start < to => {
                cursor = r.end.min(to).max(cursor);
                if r.end <= to {
                    *trivia_idx += 1;
                }
            }
            _ => break,
        }
    }
}

#[inline]
fn escape_into(slice: &str, out: &mut String) {
    push_escaped_text(out, slice);
}

/// Emit `<parent>`-wrapped children in pretty mode, interleaving the inner
/// text segments so mixed content (`<p>a <b>b</b> c</p>`) round-trips with
/// the `a `/` c` text preserved instead of dropped.
fn emit_pretty_children(
    el: &ElementData,
    raw: &str,
    trivia: &[core::ops::Range<usize>],
    opts: &SerializeOpts,
    depth: usize,
    out: &mut String,
) {
    let has_inline_text = text_with_trivia(raw, el, trivia).any(|s| !s.trim().is_empty());
    if has_inline_text {
        // Mixed content: emit text segments (escaped, trivia-skipped)
        // interleaved with re-emitted children. Disable indent for the
        // sub-tree so children don't have indentation injected into the
        // parent's text stream (`<p>a <b/> c</p>` must not become
        // `<p>a   <b/> c</p>`).
        let tight = SerializeOpts {
            indent: None,
            self_close_empty: opts.self_close_empty,
        };
        emit_tight_children(el, raw, trivia, &tight, depth, out);
        return;
    }
    // Pure-structure children: emit each on its own indented line.
    for child in &el.children {
        out.push('\n');
        emit_element(child, raw, trivia, opts, depth + 1, out);
    }
    out.push('\n');
    indent_for(opts, depth, out);
}

fn indent_for(opts: &SerializeOpts, depth: usize, out: &mut String) {
    if let Some(indent) = &opts.indent {
        for _ in 0..depth {
            out.push_str(indent);
        }
    }
}

fn element_json(el: &ElementData, raw: &str, trivia: &[core::ops::Range<usize>]) -> Value {
    let attrs: Map<String, Value> = el
        .attrs
        .iter()
        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
        .collect();
    let children: Vec<Value> = el
        .children
        .iter()
        .map(|c| element_json(c, raw, trivia))
        .collect();
    // `text` is the direct, child-stripped text of this element joined into a
    // single string. It does not recurse into descendants, so nesting depth
    // doesn't multiply allocations. Comment/CDATA byte ranges (trivia) are
    // also skipped so the consumer sees only user-authored text. Entity
    // references are decoded so the consumer sees literal characters; on
    // serialization back out they will be re-escaped.
    let mut text = String::new();
    for segment in text_with_trivia(raw, el, trivia) {
        text.push_str(&decode_entities(segment));
    }
    json!({
        "tag": el.tag.clone(),
        "attrs": Value::Object(attrs),
        "text": text,
        "children": Value::Array(children),
        "selfClosing": el.self_closing,
        "location": {
            "start": { "line": el.span.start.line, "offset": el.span.start.offset },
            "end":   { "line": el.span.end.line,   "offset": el.span.end.offset },
        },
    })
}

// ─── Display impls on the public types ───────────────────────────────────

impl std::fmt::Display for Markdown {
    /// Prints the original raw source byte-for-byte.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.raw())
    }
}

impl std::fmt::Display for ElementRef<'_> {
    /// Prints the element's full source span — its outer XML, exactly as it
    /// appeared in the source document.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let span = self.location();
        f.write_str(&self.raw[span.start.offset_usize()..span.end.offset_usize()])
    }
}
