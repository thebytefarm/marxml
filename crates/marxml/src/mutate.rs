//! String-returning mutation helpers.
//!
//! All three functions take a selector, find matching elements, splice new
//! bytes into the document's raw string, and return the resulting owned
//! `String`. The original [`Markdown`] is never modified.
//!
//! Multiple matches are spliced in a single pass, applied from later byte
//! offsets to earlier so earlier splices don't shift the positions of
//! later ones.

use core::ops::Range;
use std::fmt::Write as _;

use regex::Regex;

use crate::selector::Selector;
use crate::types::ElementRef;
use crate::Markdown;

/// Update or insert attributes on every element matching `sel`.
///
/// For each match:
/// - If the attribute name is already present, its value is replaced.
/// - Otherwise the attribute is appended at the end of the opening tag's
///   attribute list, preserving source-order of existing attributes.
///
/// Returns the new raw document. The caller decides what to do with it
/// (commonly: write to disk).
pub(crate) fn update(doc: &Markdown, sel: &Selector, new_attrs: &[(&str, &str)]) -> String {
    let raw = doc.raw();
    let mut splices: Vec<(Range<usize>, String)> = Vec::new();
    for el in doc.select(sel) {
        let open_tag = open_tag_span(&el);
        let original = &raw[open_tag.clone()];
        let rewritten = rewrite_open_tag(original, new_attrs);
        splices.push((open_tag, rewritten));
    }
    apply_splices(raw, splices)
}

/// Replace the inner content of every element matching `sel` with `new_body`.
///
/// Self-closing elements have no body; they are skipped.
pub(crate) fn replace_content(doc: &Markdown, sel: &Selector, new_body: &str) -> String {
    let raw = doc.raw();
    let splices: Vec<(Range<usize>, String)> = doc
        .select(sel)
        .filter(|el| !el.is_self_closing())
        .map(|el| (content_range(&el), new_body.to_string()))
        .collect();
    apply_splices(raw, splices)
}

/// Run a regex `replace_all` over the inner content of every element
/// matching `sel`.
pub(crate) fn replace_in(
    doc: &Markdown,
    sel: &Selector,
    pattern: &Regex,
    replacement: &str,
) -> String {
    let raw = doc.raw();
    let splices: Vec<(Range<usize>, String)> = doc
        .select(sel)
        .filter(|el| !el.is_self_closing())
        .map(|el| {
            let range = content_range(&el);
            let body = &raw[range.clone()];
            let replaced = pattern.replace_all(body, replacement).into_owned();
            (range, replaced)
        })
        .collect();
    apply_splices(raw, splices)
}

fn open_tag_span(el: &ElementRef<'_>) -> Range<usize> {
    let span = el.location();
    let start = usize::try_from(span.start.offset).unwrap_or(usize::MAX);
    if el.is_self_closing() {
        let end = usize::try_from(span.end.offset).unwrap_or(usize::MAX);
        start..end
    } else {
        // For open/close elements, the opening tag ends where content begins.
        // We don't store content_range on ElementRef directly, but content()
        // borrows it; recover the end by length.
        let content_start = start + open_tag_length(el);
        start..content_start
    }
}

/// Byte range of the inner content of a (non-self-closing) element.
///
/// Callers must filter out self-closing elements before invoking this — they
/// have no inner body to address.
fn content_range(el: &ElementRef<'_>) -> Range<usize> {
    debug_assert!(!el.is_self_closing());
    let span = el.location();
    let start = usize::try_from(span.start.offset).unwrap_or(usize::MAX);
    let end = usize::try_from(span.end.offset).unwrap_or(usize::MAX);
    let open_len = open_tag_length(el);
    let close_len = el.tag().len() + 3; // </name>
    (start + open_len)..(end - close_len)
}

/// Length of an open/close element's opening tag (`<name ...>`).
///
/// Computed from the difference between the element's total span and the
/// known inner content length and closing tag length. Callers must filter
/// out self-closing elements first.
fn open_tag_length(el: &ElementRef<'_>) -> usize {
    debug_assert!(!el.is_self_closing());
    let span = el.location();
    let total = usize::try_from(span.end.offset - span.start.offset).unwrap_or(usize::MAX);
    let content_len = el.content().len();
    let close_len = el.tag().len() + 3;
    total - content_len - close_len
}

/// Build a replacement open-tag with `new_attrs` merged in.
///
/// Strategy: keep the original tag (everything up to the first whitespace
/// after the tag name, or end of name) and the trailing `>` (or `/>`); only
/// rewrite the middle attribute section.
fn rewrite_open_tag(original: &str, new_attrs: &[(&str, &str)]) -> String {
    let bytes = original.as_bytes();
    debug_assert!(bytes.first() == Some(&b'<'));
    // Locate the tag name end.
    let mut i = 1;
    while i < bytes.len() && is_name_char(bytes[i]) {
        i += 1;
    }
    let name_end = i;
    let self_close_tail = original.ends_with("/>");
    let close_tail_len = if self_close_tail { 2 } else { 1 };
    let attrs_section = &original[name_end..original.len() - close_tail_len];
    let merged = merge_attrs(attrs_section, new_attrs);
    let close_tail = if self_close_tail { "/>" } else { ">" };
    format!("{}{merged}{close_tail}", &original[..name_end])
}

/// Merge `new_attrs` into an existing attribute section.
///
/// The section is everything between the tag name and the closing `>` (or
/// `/>`), typically including a leading space. Existing attributes are kept
/// in source order; new attributes overwrite values, and attributes not
/// already present are appended at the end.
fn merge_attrs(section: &str, new_attrs: &[(&str, &str)]) -> String {
    let mut out = String::new();
    let mut bytes = section.as_bytes();
    let mut applied: Vec<bool> = vec![false; new_attrs.len()];
    // Step through whitespace + name="value" pairs.
    while !bytes.is_empty() {
        // Copy whitespace verbatim.
        let ws_end = bytes
            .iter()
            .position(|b| !b.is_ascii_whitespace())
            .unwrap_or(bytes.len());
        out.push_str(std::str::from_utf8(&bytes[..ws_end]).unwrap_or(""));
        bytes = &bytes[ws_end..];
        if bytes.is_empty() {
            break;
        }
        // Read attribute name.
        let name_end = bytes
            .iter()
            .position(|b| !is_name_char(*b))
            .unwrap_or(bytes.len());
        let name_slice = &bytes[..name_end];
        let name = std::str::from_utf8(name_slice).unwrap_or("");
        bytes = &bytes[name_end..];
        // We expect `="value"`.
        debug_assert_eq!(bytes.first(), Some(&b'='));
        bytes = &bytes[1..]; // '='
        debug_assert_eq!(bytes.first(), Some(&b'"'));
        bytes = &bytes[1..]; // '"'
        let value_end = bytes.iter().position(|&b| b == b'"').unwrap_or(bytes.len());
        let value = std::str::from_utf8(&bytes[..value_end]).unwrap_or("");
        bytes = &bytes[value_end..];
        debug_assert_eq!(bytes.first(), Some(&b'"'));
        bytes = &bytes[1..];

        // Does the new_attrs list want to overwrite this?
        let override_value = new_attrs
            .iter()
            .enumerate()
            .find(|(_, (k, _))| *k == name)
            .map(|(i, (_, v))| (i, *v));

        if let Some((i, v)) = override_value {
            write!(out, r#"{name}="{v}""#).expect("writing to String never fails");
            applied[i] = true;
        } else {
            write!(out, r#"{name}="{value}""#).expect("writing to String never fails");
        }
    }
    // Append any new attrs not consumed.
    let mut trailing = String::new();
    for (i, (k, v)) in new_attrs.iter().enumerate() {
        if !applied[i] {
            write!(trailing, r#" {k}="{v}""#).expect("writing to String never fails");
        }
    }
    if !trailing.is_empty() {
        // If the existing section already ends with whitespace, the leading
        // space inside `trailing` would double it; trim instead.
        if out.ends_with(char::is_whitespace) {
            out.push_str(trailing.trim_start());
        } else {
            out.push_str(&trailing);
        }
    }
    out
}

#[inline]
fn is_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.'
}

/// Apply a list of byte-range replacements to `raw`, returning the new string.
///
/// Splices are sorted by descending start offset so the earlier replacements
/// don't shift later ones. Overlapping ranges (which would be a bug at the
/// caller level) take the first.
fn apply_splices(raw: &str, mut splices: Vec<(Range<usize>, String)>) -> String {
    if splices.is_empty() {
        return raw.to_string();
    }
    splices.sort_by_key(|(range, _)| core::cmp::Reverse(range.start));
    let mut result = raw.to_string();
    for (range, replacement) in splices {
        result.replace_range(range, &replacement);
    }
    result
}
