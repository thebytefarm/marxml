//! napi-rs bindings exposing the marxml crate to Node.
//!
//! The public JS API is a factory: `parse(source)` returns a `NativeMarkdown`
//! handle whose methods (`select`, `updateAttrs`, `toXml`, …) close over the
//! parsed document. The factory shape lives in the JS wrapper at
//! `marxml.mjs`; this file exposes the underlying napi class.
//!
//! Design notes:
//! - The document is parsed once into the `NativeMarkdown` handle. Subsequent
//!   queries and mutations reuse that handle — the document is never
//!   reparsed. Selector strings are still parsed per call (see follow-up
//!   work — exposing a compiled `Selector` class).
//! - All fallible crate calls are mapped to `napi::Error` via the `From`
//!   impls below, so call sites use `?` / `Into::into` rather than ad-hoc
//!   `.map_err(|e| Error::new(...))` closures.
//! - `Element` is still a flat `#[napi(object)]` POJO for the `elements`
//!   getter; materializing the whole tree as opaque handles is a separate
//!   refactor.
//!
//! Clippy allowances below are justified per-lint:
//! - `needless_pass_by_value`: napi-rs expands `#[napi]` methods into FFI
//!   signatures that take owned JS bridge values; switching to `&str` is
//!   not yet supported uniformly in v3 derive output.
//! - `missing_errors_doc`: error doc comments are intentionally on the
//!   `marxml::*Error` types in the core crate; the binding is a transparent
//!   pass-through and duplicating them rots.
//! - `missing_panics_doc`: napi-derive expansion contains FFI panic edges
//!   that are unreachable from caller-shaped input. Documenting "may panic
//!   if napi's FFI layer is broken" is noise.

#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

use std::collections::HashMap;

use napi::bindgen_prelude::Either;
use napi::{Error, Result, Status};
use napi_derive::napi;
use regex::{Regex, RegexBuilder};

// ─── Error mapping ────────────────────────────────────────────────────────

/// Map any crate-side `Result<T, E: std::error::Error>` into `napi::Result<T>`
/// with `Status::InvalidArg`. The orphan rule prevents a direct
/// `From<marxml::*Error> for napi::Error` impl in this crate, so this
/// extension trait stands in: call sites read `marxml::Selector::parse(s)
/// .into_napi()?` instead of repeating the `.map_err(|e| Error::new(...))`
/// closure on every fallible boundary.
///
/// Every marxml error variant is caller-input (malformed selector,
/// duplicate attribute, invalid XML name, regex compile failure), so a
/// single `InvalidArg` status fits all of them. If a future variant ever
/// represents a binding-internal failure, swap the call site to an explicit
/// `Error::new(Status::GenericFailure, ...)` and document why.
trait IntoNapi<T> {
    fn into_napi(self) -> Result<T>;
}

impl<T> IntoNapi<T> for std::result::Result<T, marxml::ParseError> {
    fn into_napi(self) -> Result<T> {
        self.map_err(|e| Error::new(Status::InvalidArg, e.to_string()))
    }
}

impl<T> IntoNapi<T> for std::result::Result<T, marxml::SelectorError> {
    fn into_napi(self) -> Result<T> {
        self.map_err(|e| Error::new(Status::InvalidArg, e.to_string()))
    }
}

impl<T> IntoNapi<T> for std::result::Result<T, marxml::MutateError> {
    fn into_napi(self) -> Result<T> {
        self.map_err(|e| Error::new(Status::InvalidArg, e.to_string()))
    }
}

impl<T> IntoNapi<T> for std::result::Result<T, marxml::SchemaError> {
    fn into_napi(self) -> Result<T> {
        self.map_err(|e| Error::new(Status::InvalidArg, e.to_string()))
    }
}

impl<T> IntoNapi<T> for std::result::Result<T, regex::Error> {
    fn into_napi(self) -> Result<T> {
        self.map_err(|e| Error::new(Status::InvalidArg, e.to_string()))
    }
}

// ─── Flat shape types crossing the FFI ────────────────────────────────────

/// One-based line + zero-based byte offset into the source document.
#[napi(object)]
pub struct SourcePosition {
    /// One-based line number.
    pub line: u32,
    /// Zero-based byte offset into the source.
    pub offset: u32,
}

/// Half-open `[start, end)` source range.
#[napi(object)]
pub struct SourceSpan {
    /// Inclusive start position.
    pub start: SourcePosition,
    /// Exclusive end position.
    pub end: SourcePosition,
}

/// A parsed XML element, materialized for JS consumption.
#[napi(object)]
pub struct Element {
    /// Tag name (e.g. `"task"`).
    pub tag: String,
    /// Attribute key/value pairs in source order.
    pub attrs: HashMap<String, String>,
    /// Inner content as a raw slice of the original source (entity
    /// references not decoded). Empty for self-closing tags.
    pub content: String,
    /// Direct child elements in source order.
    pub children: Vec<Element>,
    /// `true` for `<tag/>`, `false` for `<tag>…</tag>`.
    pub self_closing: bool,
    /// Source span covering the entire element including its open and close
    /// tags.
    pub loc: SourceSpan,
}

impl Element {
    fn from_ref(el: &marxml::ElementRef<'_>) -> Self {
        let attrs: HashMap<String, String> = el
            .attrs()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let children: Vec<Element> = el.children().map(|c| Self::from_ref(&c)).collect();
        let loc = el.location();
        Self {
            tag: el.tag().to_string(),
            attrs,
            content: el.content().to_string(),
            children,
            self_closing: el.is_self_closing(),
            loc: SourceSpan {
                start: SourcePosition {
                    line: loc.start.line,
                    offset: loc.start.offset,
                },
                end: SourcePosition {
                    line: loc.end.line,
                    offset: loc.end.offset,
                },
            },
        }
    }
}

/// One attribute update for [`NativeMarkdown::update_attrs`].
#[napi(object)]
pub struct AttrUpdate {
    /// Attribute name (must be a valid XML name).
    pub name: String,
    /// New attribute value.
    pub value: String,
}

/// Accepts a JS `RegExp` by destructuring its serializable fields. The
/// wrapper at `marxml.mjs` handles the conversion from a real `RegExp`
/// instance.
#[napi(object)]
pub struct RegExpShape {
    /// The pattern body, without the surrounding `/`.
    pub source: String,
    /// JS regex flags; `i`/`m`/`s`/`x` are forwarded to the Rust engine,
    /// `g`/`u`/`y`/`d` are ignored as they have no Rust-side equivalent.
    pub flags: Option<String>,
}

/// One validation failure surfaced by [`NativeMarkdown::validate`].
#[napi(object)]
pub struct ValidationError {
    /// Stable error kind: `"missing_attr"`, `"invalid_attr"`,
    /// `"missing_child"`, `"unexpected_child"`, `"empty_content"`, or
    /// `"unknown"` for variants newer than the binding.
    pub kind: String,
    /// Tag of the offending element.
    pub tag: String,
    /// One-based line number where the error was detected.
    pub line: u32,
    /// Human-readable rendering of the error.
    pub message: String,
}

/// Outcome of [`NativeMarkdown::validate`].
#[napi(object)]
pub struct ValidationReport {
    /// `true` iff `errors` is empty.
    pub valid: bool,
    /// Every failure detected, in document order.
    pub errors: Vec<ValidationError>,
}

/// One attribute constraint for [`TagSchemaShape`].
#[napi(object)]
pub struct AttrConstraintShape {
    /// `"string"`, `"enum"`, or `"regex"`.
    pub kind: String,
    /// Allowed values for `kind = "enum"`. Empty/absent otherwise.
    pub values: Option<Vec<String>>,
    /// Pattern body for `kind = "regex"`. Empty/absent otherwise.
    pub pattern: Option<String>,
    /// `true` to fail validation when the attribute is missing.
    pub required: Option<bool>,
}

/// Per-tag schema declaration for [`NativeMarkdown::validate`].
#[napi(object)]
pub struct TagSchemaShape {
    /// Attribute constraints keyed by attribute name.
    pub attrs: Option<HashMap<String, AttrConstraintShape>>,
    /// Child tag names that must appear at least once.
    pub children_required: Option<Vec<String>>,
    /// Additional child tag names allowed alongside the required ones.
    pub children_optional: Option<Vec<String>>,
    /// When `true`, any child tag not in `children_required` /
    /// `children_optional` is flagged as `unexpected_child`.
    pub children_exclusive: Option<bool>,
    /// When `true`, the element must contain at least one non-whitespace
    /// character of direct text content.
    pub content_required: Option<bool>,
}

/// Optional flags for [`NativeMarkdown::to_xml`].
#[napi(object)]
pub struct ToXmlOpts {
    /// When `true`, emit indented multi-line output. Default `false`.
    pub pretty: Option<bool>,
}

// ─── The native handle ────────────────────────────────────────────────────

/// Parsed-document handle. Construct via the top-level [`parse`] function.
///
/// This class is the napi-level handle; the JS factory at `marxml.mjs` wraps
/// it so end users never call `new NativeMarkdown(...)` directly.
#[napi]
pub struct NativeMarkdown {
    inner: marxml::Markdown,
}

#[napi]
impl NativeMarkdown {
    /// Original document source, byte-for-byte.
    #[napi(getter)]
    pub fn raw(&self) -> &str {
        self.inner.raw()
    }

    /// Materialized root elements of the document. Walks the parsed tree
    /// each time it's accessed — cache the result on the JS side if you'll
    /// re-read it.
    #[napi(getter)]
    pub fn elements(&self) -> Vec<Element> {
        self.inner
            .root_elements()
            .map(|e| Element::from_ref(&e))
            .collect()
    }

    /// Run a selector against the document and return every matching element
    /// in source order.
    #[napi]
    pub fn select(&self, selector: String) -> Result<Vec<Element>> {
        let sel = parse_selector(&selector)?;
        Ok(self
            .inner
            .select(&sel)
            .map(|e| Element::from_ref(&e))
            .collect())
    }

    /// Update or insert attributes on every element matching `selector`.
    /// Returns the rewritten document. The handle is unchanged.
    ///
    /// Routes through the crate's fallible `try_update`; invalid XML
    /// attribute names and duplicate keys surface as `napi::Error` with
    /// `InvalidArg`, never as a panic.
    #[napi]
    pub fn update_attrs(&self, selector: String, new_attrs: Vec<AttrUpdate>) -> Result<String> {
        let sel = parse_selector(&selector)?;
        let pairs: Vec<(&str, &str)> = new_attrs
            .iter()
            .map(|a| (a.name.as_str(), a.value.as_str()))
            .collect();
        self.inner
            .try_update(&sel, &pairs)
            .map(|report| report.output)
            .into_napi()
    }

    /// Replace inner content verbatim. `new_body` is spliced as raw bytes —
    /// `<` / `&` / `"` are NOT escaped. Use [`Self::replace_text`] for
    /// untrusted text.
    #[napi]
    pub fn replace_content(&self, selector: String, new_body: String) -> Result<String> {
        let sel = parse_selector(&selector)?;
        Ok(self.inner.replace_content(&sel, &new_body))
    }

    /// Replace inner content with `new_text`, escaping `<` / `&` / `"`
    /// before splicing. Safe for user-controlled strings.
    #[napi]
    pub fn replace_text(&self, selector: String, new_text: String) -> Result<String> {
        let sel = parse_selector(&selector)?;
        Ok(self.inner.replace_text(&sel, &new_text))
    }

    /// Run a regex `replace_all` over the inner content of matching
    /// elements. `pattern` accepts either a plain string or a `RegExpShape`
    /// (i.e. the destructured fields of a JS `RegExp`). JS regex flags
    /// `i`/`m`/`s`/`x` are honored via `RegexBuilder` setters
    /// (`case_insensitive`, `multi_line`, `dot_matches_new_line`,
    /// `ignore_whitespace`); `g`/`u`/`y`/`d` are accepted-and-ignored
    /// (`replace_all` is already global; `u` is implicit; `y`/`d` have no
    /// Rust equivalent). Any other flag returns `InvalidArg`.
    ///
    /// `replacement` is verbatim text — `$1` / `$name` are NOT interpreted as
    /// capture references.
    #[napi]
    pub fn replace_in_content(
        &self,
        selector: String,
        pattern: Either<String, RegExpShape>,
        replacement: String,
    ) -> Result<String> {
        let sel = parse_selector(&selector)?;
        let re = compile_regex(pattern)?;
        Ok(self.inner.replace_in(&sel, &re, &replacement))
    }

    /// Serialize the parsed XML elements back to a string. Surrounding
    /// markdown prose is dropped — this is just the structured payload.
    #[napi]
    pub fn to_xml(&self, opts: Option<ToXmlOpts>) -> Result<String> {
        let pretty = opts.and_then(|o| o.pretty).unwrap_or(false);
        let serialize_opts = if pretty {
            marxml::SerializeOpts::pretty()
        } else {
            marxml::SerializeOpts::default()
        };
        Ok(self.inner.to_xml(&serialize_opts))
    }

    /// Serialize the element tree as a JSON string. Top-level is an array of
    /// root elements; see the crate docs for the per-element schema.
    #[napi]
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(&self.inner.to_json())
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Validate the document against `schema` (per-tag declarations keyed by
    /// tag name). Returns a report with every violation.
    ///
    /// Schema construction routes through `try_build`; invalid regex
    /// patterns, non-XML tag/attr names, and duplicate declarations surface
    /// as `napi::Error` with `InvalidArg`.
    #[napi]
    #[allow(clippy::implicit_hasher)]
    pub fn validate(&self, schema: HashMap<String, TagSchemaShape>) -> Result<ValidationReport> {
        let schema = build_schema(schema)?;
        let report = marxml::validate(&self.inner, &schema);
        Ok(ValidationReport {
            valid: report.is_valid(),
            errors: report
                .errors()
                .iter()
                .map(|e| ValidationError {
                    kind: error_kind(e).to_string(),
                    tag: error_tag(e),
                    line: error_line(e),
                    message: e.to_string(),
                })
                .collect(),
        })
    }
}

// ─── Top-level entry points ───────────────────────────────────────────────

/// Parse a markdown + XML source string into a [`NativeMarkdown`] handle.
/// Throws on malformed input.
#[napi]
pub fn parse(source: String) -> Result<NativeMarkdown> {
    marxml::parse(&source)
        .map(|inner| NativeMarkdown { inner })
        .into_napi()
}

// ─── Internal helpers ─────────────────────────────────────────────────────

fn parse_selector(s: &str) -> Result<marxml::Selector> {
    marxml::Selector::parse(s).into_napi()
}

fn compile_regex(pattern: Either<String, RegExpShape>) -> Result<Regex> {
    let (source, flags) = match pattern {
        Either::A(s) => (s, String::new()),
        Either::B(rx) => (rx.source, rx.flags.unwrap_or_default()),
    };
    let mut builder = RegexBuilder::new(&source);
    for ch in flags.chars() {
        match ch {
            'i' => {
                builder.case_insensitive(true);
            }
            'm' => {
                builder.multi_line(true);
            }
            's' => {
                builder.dot_matches_new_line(true);
            }
            'x' => {
                builder.ignore_whitespace(true);
            }
            // JS-only flags that have no Rust equivalent or are implicit.
            'g' | 'u' | 'y' | 'd' => {}
            other => {
                return Err(Error::new(
                    Status::InvalidArg,
                    format!("unsupported regex flag: {other:?}"),
                ));
            }
        }
    }
    builder.build().into_napi()
}

fn build_schema(input: HashMap<String, TagSchemaShape>) -> Result<marxml::Schema> {
    let mut builder = marxml::Schema::builder();
    for (tag_name, shape) in input {
        builder = builder.tag(&tag_name, |mut tb| {
            if let Some(attrs) = shape.attrs {
                for (name, c) in attrs {
                    // Unknown kinds default to `String` to stay
                    // forward-compatible with future binding versions.
                    let kind = match c.kind.as_str() {
                        "enum" => marxml::schema::AttrKind::Enum(c.values.unwrap_or_default()),
                        "regex" => marxml::schema::AttrKind::Regex(c.pattern.unwrap_or_default()),
                        _ => marxml::schema::AttrKind::String,
                    };
                    let constraint = if c.required.unwrap_or(false) {
                        kind.required()
                    } else {
                        kind.optional()
                    };
                    tb = tb.attr(&name, constraint);
                }
            }
            for name in shape.children_required.unwrap_or_default() {
                tb = tb.child_required(&name);
            }
            for name in shape.children_optional.unwrap_or_default() {
                tb = tb.child_optional(&name);
            }
            if shape.children_exclusive.unwrap_or(false) {
                tb = tb.exclusive_children();
            }
            if shape.content_required.unwrap_or(false) {
                tb = tb.content_required();
            }
            tb
        });
    }
    builder.try_build().into_napi()
}

fn error_kind(e: &marxml::ValidationError) -> &'static str {
    match e {
        marxml::ValidationError::MissingAttr { .. } => "missing_attr",
        marxml::ValidationError::InvalidAttr { .. } => "invalid_attr",
        marxml::ValidationError::MissingChild { .. } => "missing_child",
        marxml::ValidationError::UnexpectedChild { .. } => "unexpected_child",
        marxml::ValidationError::EmptyContent { .. } => "empty_content",
        // `ValidationError` is `#[non_exhaustive]`; future variants surface
        // as `"unknown"` until the binding catches up.
        _ => "unknown",
    }
}

fn error_tag(e: &marxml::ValidationError) -> String {
    match e {
        marxml::ValidationError::MissingAttr { tag, .. }
        | marxml::ValidationError::InvalidAttr { tag, .. }
        | marxml::ValidationError::MissingChild { tag, .. }
        | marxml::ValidationError::UnexpectedChild { tag, .. }
        | marxml::ValidationError::EmptyContent { tag, .. } => tag.clone(),
        _ => String::new(),
    }
}

fn error_line(e: &marxml::ValidationError) -> u32 {
    match e {
        marxml::ValidationError::MissingAttr { line, .. }
        | marxml::ValidationError::InvalidAttr { line, .. }
        | marxml::ValidationError::MissingChild { line, .. }
        | marxml::ValidationError::UnexpectedChild { line, .. }
        | marxml::ValidationError::EmptyContent { line, .. } => *line,
        _ => 0,
    }
}
