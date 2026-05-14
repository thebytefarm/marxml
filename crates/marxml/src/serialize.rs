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

use std::fmt::Write as _;

use serde_json::{json, Value};

use crate::types::{ElementData, ElementRef};
use crate::Markdown;

/// Options for [`Markdown::to_xml`].
#[derive(Debug, Clone, Default)]
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
}

pub(crate) fn to_xml(doc: &Markdown, opts: &SerializeOpts) -> String {
    let mut out = String::new();
    for (i, root) in doc.roots_internal().iter().enumerate() {
        if i > 0 && opts.indent.is_some() {
            out.push('\n');
        }
        emit_element(root, doc.raw(), opts, 0, &mut out);
    }
    out
}

pub(crate) fn to_json(doc: &Markdown) -> Value {
    Value::Array(
        doc.roots_internal()
            .iter()
            .map(|root| element_json(root, doc.raw()))
            .collect(),
    )
}

fn emit_element(el: &ElementData, raw: &str, opts: &SerializeOpts, depth: usize, out: &mut String) {
    indent_for(opts, depth, out);
    out.push('<');
    out.push_str(&el.tag);
    for (k, v) in &el.attrs {
        write!(out, r#" {k}="{v}""#).expect("write to String never fails");
    }
    let body = &raw[el.content_range.clone()];
    let is_empty = el.children.is_empty() && body.trim().is_empty();
    if is_empty && (el.self_closing || opts.self_close_empty) {
        out.push_str("/>");
        return;
    }
    out.push('>');
    if opts.indent.is_some() && !el.children.is_empty() {
        // Pretty mode: each child on its own line.
        for child in &el.children {
            out.push('\n');
            emit_element(child, raw, opts, depth + 1, out);
        }
        out.push('\n');
        indent_for(opts, depth, out);
    } else if el.children.is_empty() {
        // Pure text body — copy verbatim.
        out.push_str(body);
    } else {
        // Tight mode with children: emit body verbatim (which already contains
        // the children's source bytes).
        out.push_str(body);
    }
    out.push_str("</");
    out.push_str(&el.tag);
    out.push('>');
}

fn indent_for(opts: &SerializeOpts, depth: usize, out: &mut String) {
    if let Some(indent) = &opts.indent {
        for _ in 0..depth {
            out.push_str(indent);
        }
    }
}

fn element_json(el: &ElementData, raw: &str) -> Value {
    let attrs: serde_json::Map<String, Value> = el
        .attrs
        .iter()
        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
        .collect();
    let children: Vec<Value> = el.children.iter().map(|c| element_json(c, raw)).collect();
    json!({
        "tag": el.tag.clone(),
        "attrs": Value::Object(attrs),
        "content": raw[el.content_range.clone()].to_string(),
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
        let start = usize::try_from(span.start.offset).unwrap_or(usize::MAX);
        let end = usize::try_from(span.end.offset).unwrap_or(usize::MAX);
        f.write_str(&self.raw[start..end])
    }
}
