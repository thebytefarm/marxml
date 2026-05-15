//! String-returning mutation helpers.
//!
//! All three functions take a selector, find matching elements, splice new
//! bytes into the document's raw string, and return the resulting owned
//! `String`. The original [`Markdown`] is never modified.
//!
//! Splices are applied in a single forward pass over `raw`. When a selector
//! matches both a parent and one of its descendants, the parent splice
//! encloses the child's range; the outer splice wins and the inner splice is
//! discarded so the document doesn't end up with mutually-inconsistent edits
//! at overlapping byte ranges. The fallible variants ([`crate::Markdown::try_update`],
//! [`try_replace_content`], [`try_replace_in`]) surface the discarded count
//! in the returned [`MutationReport`] so callers can distinguish "no match"
//! from "match shadowed by an outer match".
//!
//! ## Raw vs. text semantics
//!
//! `replace_content` and `replace_in` substitute **raw bytes** into the source.
//! `new_body` / the regex `replacement` are written verbatim — special
//! characters (`<`, `&`, `"`) are **not** escaped. This is deliberate: the
//! intended use is splicing well-formed XML, prose, or other markup. For
//! text that should be safe by default, use [`replace_text`] /
//! [`replace_text_in`] which route the input through `escape_text` before
//! splicing.
//!
//! `update`, by contrast, owns the surrounding attribute syntax, so it
//! validates attribute names against [`crate::is_valid_name`] and
//! XML-escapes attribute values before writing them. The infallible
//! [`update`] panics on programmer error (invalid name / duplicate key) so
//! that bugs surface loudly; [`crate::Markdown::try_update`] returns the
//! offending input as a [`MutateError`] for runtime-sourced inputs.

use core::ops::Range;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use regex::{NoExpand, Regex};
use thiserror::Error;

use crate::escape::{escape_text, is_valid_name, push_escaped_attr};
use crate::selector::Selector;
use crate::types::ElementRef;
use crate::Markdown;

/// Errors returned by the fallible mutation variants.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum MutateError {
    /// An attribute name passed to [`crate::Markdown::try_update`] is not a valid XML name.
    #[error("invalid XML attribute name {name:?}")]
    InvalidAttrName {
        /// The offending name.
        name: String,
    },
    /// The attribute slice passed to [`crate::Markdown::try_update`] repeats the same name.
    #[error("duplicate attribute name {name:?} in update slice")]
    DuplicateAttrName {
        /// The repeated name.
        name: String,
    },
}

/// Outcome of a successful mutation. Reports both the rewritten document
/// and any accounting useful for diagnostics.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct MutationReport {
    /// Rewritten document. The original [`Markdown`] is unchanged.
    pub output: String,
    /// Number of splices applied to the output.
    pub applied: usize,
    /// Number of splices skipped because they overlapped a previously-
    /// emitted splice (the "outermost wins" rule).
    pub skipped_overlaps: usize,
    /// Number of selected elements that were self-closing and therefore had
    /// no content range to splice into. Relevant for `replace_content` /
    /// `replace_in` only — `update` is happy to rewrite self-closing tags.
    pub skipped_self_closing: usize,
}

// ─── infallible entry points (public re-exports via Markdown) ─────────────

pub(crate) fn update(doc: &Markdown, sel: &Selector, new_attrs: &[(&str, &str)]) -> String {
    // Programmer error (invalid attribute name or duplicate key in
    // `new_attrs`) panics consistently in both debug and release builds.
    // Callers that need to recover from bad input should use
    // [`Markdown::try_update`] instead, which returns the offending value as
    // a [`MutateError`].
    try_update(doc, sel, new_attrs)
        .unwrap_or_else(|e| panic!("update() called with invalid attrs: {e}"))
        .output
}

pub(crate) fn replace_content(doc: &Markdown, sel: &Selector, new_body: &str) -> String {
    splice_content(doc, sel, new_body).output
}

pub(crate) fn replace_in(
    doc: &Markdown,
    sel: &Selector,
    pattern: &Regex,
    replacement: &str,
) -> String {
    splice_regex(doc, sel, pattern, replacement).output
}

pub(crate) fn replace_text(doc: &Markdown, sel: &Selector, new_body: &str) -> String {
    let escaped = escape_text(new_body).into_owned();
    splice_content(doc, sel, &escaped).output
}

pub(crate) fn replace_text_in(
    doc: &Markdown,
    sel: &Selector,
    pattern: &Regex,
    replacement: &str,
) -> String {
    let escaped = escape_text(replacement).into_owned();
    splice_regex_with(doc, sel, pattern, &escaped).output
}

// ─── fallible variants ────────────────────────────────────────────────────

pub(crate) fn try_update(
    doc: &Markdown,
    sel: &Selector,
    new_attrs: &[(&str, &str)],
) -> Result<MutationReport, MutateError> {
    check_new_attrs(new_attrs)?;
    let raw = doc.raw();
    // `Cow::Owned` is appropriate here: every rewritten open tag is unique.
    let mut splices: Vec<(Range<usize>, Cow<'_, str>)> = Vec::new();
    for el in doc.select(sel) {
        let open_tag = open_tag_span(&el);
        let self_close = el.is_self_closing();
        let rewritten = rewrite_open_tag(&el, new_attrs, self_close);
        splices.push((open_tag, Cow::Owned(rewritten)));
    }
    Ok(apply_splices(raw, splices))
}

pub(crate) fn try_replace_content(
    doc: &Markdown,
    sel: &Selector,
    new_body: &str,
) -> MutationReport {
    splice_content(doc, sel, new_body)
}

pub(crate) fn try_replace_in(
    doc: &Markdown,
    sel: &Selector,
    pattern: &Regex,
    replacement: &str,
) -> MutationReport {
    splice_regex(doc, sel, pattern, replacement)
}

// ─── helpers ──────────────────────────────────────────────────────────────

fn splice_content<'a>(doc: &'a Markdown, sel: &Selector, new_body: &'a str) -> MutationReport {
    let raw = doc.raw();
    let mut self_closing_skipped = 0usize;
    let mut splices: Vec<(Range<usize>, Cow<'_, str>)> = Vec::new();
    for el in doc.select(sel) {
        if el.is_self_closing() {
            self_closing_skipped += 1;
            continue;
        }
        splices.push((el.content_range(), Cow::Borrowed(new_body)));
    }
    let mut report = apply_splices(raw, splices);
    report.skipped_self_closing = self_closing_skipped;
    report
}

fn splice_regex(
    doc: &Markdown,
    sel: &Selector,
    pattern: &Regex,
    replacement: &str,
) -> MutationReport {
    splice_regex_with(doc, sel, pattern, replacement)
}

fn splice_regex_with(
    doc: &Markdown,
    sel: &Selector,
    pattern: &Regex,
    replacement: &str,
) -> MutationReport {
    let raw = doc.raw();
    let mut self_closing_skipped = 0usize;
    let mut splices: Vec<(Range<usize>, Cow<'_, str>)> = Vec::new();
    for el in doc.select(sel) {
        if el.is_self_closing() {
            self_closing_skipped += 1;
            continue;
        }
        let range = el.content_range();
        let body = &raw[range.clone()];
        // `regex::NoExpand` disables `$1`/`$name` expansion so the module's
        // "verbatim" contract holds. When `replace_all` returns `Cow::Borrowed`
        // (no matches in this body), keep the borrow — `apply_splices` will
        // re-emit the original bytes without an extra allocation.
        let replaced = pattern.replace_all(body, NoExpand(replacement));
        let payload: Cow<'_, str> = match replaced {
            std::borrow::Cow::Borrowed(_) => Cow::Borrowed(body),
            std::borrow::Cow::Owned(s) => Cow::Owned(s),
        };
        splices.push((range, payload));
    }
    let mut report = apply_splices(raw, splices);
    report.skipped_self_closing = self_closing_skipped;
    report
}

fn check_new_attrs(new_attrs: &[(&str, &str)]) -> Result<(), MutateError> {
    let mut seen: HashSet<&str> = HashSet::with_capacity(new_attrs.len());
    for (k, _) in new_attrs {
        if !is_valid_name(k) {
            return Err(MutateError::InvalidAttrName {
                name: (*k).to_string(),
            });
        }
        if !seen.insert(*k) {
            return Err(MutateError::DuplicateAttrName {
                name: (*k).to_string(),
            });
        }
    }
    Ok(())
}

/// Byte range of an element's opening tag (`<name ...>` or `<name ... />`).
fn open_tag_span(el: &ElementRef<'_>) -> Range<usize> {
    let span = el.location();
    let start = span.start.offset_usize();
    if el.is_self_closing() {
        start..span.end.offset_usize()
    } else {
        start..el.content_range().start
    }
}

/// Build the replacement opening tag for `el` with `new_attrs` merged.
///
/// `new_attrs` is indexed by name once at the entry of `try_update` (caller
/// passes the same slice for every match), but the index is rebuilt here
/// per call — the slice is tiny in practice (<10 entries), so a linear
/// scan beats hash construction below a threshold.
fn rewrite_open_tag(el: &ElementRef<'_>, new_attrs: &[(&str, &str)], self_close: bool) -> String {
    let use_map = new_attrs.len() >= ATTR_INDEX_THRESHOLD;
    let index: Option<HashMap<&str, usize>> = if use_map {
        let mut m = HashMap::with_capacity(new_attrs.len());
        for (i, (k, _)) in new_attrs.iter().enumerate() {
            m.insert(*k, i);
        }
        Some(m)
    } else {
        None
    };
    let mut applied = vec![false; new_attrs.len()];
    let mut out = String::new();
    out.push('<');
    out.push_str(el.tag());
    for (name, existing) in el.attrs() {
        out.push(' ');
        let lookup = index
            .as_ref()
            .and_then(|m| m.get(name).copied())
            .or_else(|| new_attrs.iter().position(|(k, _)| *k == name));
        if let Some(i) = lookup {
            write_attr(&mut out, name, new_attrs[i].1);
            applied[i] = true;
        } else {
            // The existing value already passed the tokenizer's validation,
            // but re-escape on output so a permissively-tokenized value can't
            // reappear as invalid XML on this rewrite.
            write_attr(&mut out, name, existing);
        }
    }
    for (i, (k, v)) in new_attrs.iter().enumerate() {
        if !applied[i] {
            out.push(' ');
            write_attr(&mut out, k, v);
        }
    }
    if self_close {
        out.push_str("/>");
    } else {
        out.push('>');
    }
    out
}

/// Threshold past which `rewrite_open_tag` builds a `HashMap` for
/// `new_attrs` lookup. Below this, a linear scan is faster (no hash setup
/// cost).
const ATTR_INDEX_THRESHOLD: usize = 8;

/// Write `name="<escaped-value>"` into `out`. The name is assumed to have
/// passed the [`is_valid_name`] check at the entry to `try_update`.
fn write_attr(out: &mut String, name: &str, value: &str) {
    out.push_str(name);
    out.push_str("=\"");
    push_escaped_attr(out, value);
    out.push('"');
}

/// Apply a list of byte-range replacements to `raw` in a single forward pass.
///
/// Splices are sorted ascending by start (with longer ranges preferred when
/// starts tie, so an outer enclosing range wins over an inner match). Any
/// splice whose range overlaps with a previously-emitted one is counted in
/// the report's `skipped_overlaps` and discarded.
///
/// Owns the replacement via `Cow<'_, str>` so callers can borrow a shared
/// replacement body across many matches without per-match allocation.
fn apply_splices(raw: &str, mut splices: Vec<(Range<usize>, Cow<'_, str>)>) -> MutationReport {
    if splices.is_empty() {
        return MutationReport {
            output: raw.to_string(),
            applied: 0,
            skipped_overlaps: 0,
            skipped_self_closing: 0,
        };
    }
    splices.sort_by(|a, b| {
        a.0.start
            .cmp(&b.0.start)
            .then_with(|| b.0.end.cmp(&a.0.end))
    });
    let mut out = String::with_capacity(raw.len());
    let mut cursor: usize = 0;
    let mut applied = 0usize;
    let mut skipped = 0usize;
    for (range, replacement) in splices {
        if range.start < cursor {
            skipped += 1;
            continue;
        }
        if range.start > raw.len() || range.end > raw.len() || range.start > range.end {
            skipped += 1;
            continue;
        }
        out.push_str(&raw[cursor..range.start]);
        out.push_str(&replacement);
        cursor = range.end;
        applied += 1;
    }
    if cursor < raw.len() {
        out.push_str(&raw[cursor..]);
    }
    MutationReport {
        output: out,
        applied,
        skipped_overlaps: skipped,
        skipped_self_closing: 0,
    }
}
