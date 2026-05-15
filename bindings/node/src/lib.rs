//! napi-rs bindings exposing the marxml crate to Node.
//!
//! Designed to mirror the Rust shape with JS-flavored adaptations:
//! - Selectors are passed as plain strings (internally compiled per call).
//! - `Markdown` and `Element` are returned as plain `#[napi(object)]` shapes
//!   — flat data, no class instances. Material­ized once; query methods on
//!   the JS side that need to walk back to the Rust tree are exposed as
//!   top-level functions taking a parsed handle (or accepting the raw
//!   source).

#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

use std::collections::HashMap;

use napi::bindgen_prelude::Either;
use napi::Result;
use napi_derive::napi;
use regex::Regex;

// ─── Shape types crossing the FFI ─────────────────────────────────────────

#[napi(object)]
pub struct SourcePosition {
    pub line: u32,
    pub offset: u32,
}

#[napi(object)]
pub struct SourceSpan {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

#[napi(object)]
pub struct Element {
    pub tag: String,
    pub attrs: HashMap<String, String>,
    pub content: String,
    pub children: Vec<Element>,
    pub self_closing: bool,
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

#[napi(object)]
pub struct Markdown {
    pub raw: String,
    pub elements: Vec<Element>,
}

impl Markdown {
    fn from_doc(doc: &marxml::Markdown) -> Self {
        let elements: Vec<Element> = doc.root_elements().map(|e| Element::from_ref(&e)).collect();
        Self {
            raw: doc.raw().to_string(),
            elements,
        }
    }
}

#[napi(object)]
pub struct AttrUpdate {
    pub name: String,
    pub value: String,
}

#[napi(object)]
pub struct ValidationError {
    pub kind: String,
    pub tag: String,
    pub line: u32,
    pub message: String,
}

#[napi(object)]
pub struct ValidationReport {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
}

#[napi(object)]
pub struct AttrConstraintShape {
    /// `"string"`, `"enum"`, or `"regex"`.
    pub kind: String,
    /// Values for `kind = "enum"`. Empty otherwise.
    pub values: Option<Vec<String>>,
    /// Pattern for `kind = "regex"`. Empty otherwise.
    pub pattern: Option<String>,
    pub required: Option<bool>,
}

#[napi(object)]
pub struct TagSchemaShape {
    pub attrs: Option<HashMap<String, AttrConstraintShape>>,
    pub children_required: Option<Vec<String>>,
    pub children_optional: Option<Vec<String>>,
    pub children_exclusive: Option<bool>,
    pub content_required: Option<bool>,
}

// ─── Public API ───────────────────────────────────────────────────────────

#[napi]
pub fn parse(source: String) -> Result<Markdown> {
    marxml::parse(&source)
        .map(|doc| Markdown::from_doc(&doc))
        .map_err(|e| napi::Error::new(napi::Status::InvalidArg, e.to_string()))
}

#[napi]
pub fn select(source: String, selector: String) -> Result<Vec<Element>> {
    let doc = marxml::parse(&source)
        .map_err(|e| napi::Error::new(napi::Status::InvalidArg, e.to_string()))?;
    let sel = marxml::Selector::parse(&selector)
        .map_err(|e| napi::Error::new(napi::Status::InvalidArg, e.to_string()))?;
    Ok(doc.select(&sel).map(|e| Element::from_ref(&e)).collect())
}

#[napi]
pub fn update_attrs(
    source: String,
    selector: String,
    new_attrs: Vec<AttrUpdate>,
) -> Result<String> {
    let doc = marxml::parse(&source)
        .map_err(|e| napi::Error::new(napi::Status::InvalidArg, e.to_string()))?;
    let sel = marxml::Selector::parse(&selector)
        .map_err(|e| napi::Error::new(napi::Status::InvalidArg, e.to_string()))?;
    let pairs: Vec<(&str, &str)> = new_attrs
        .iter()
        .map(|a| (a.name.as_str(), a.value.as_str()))
        .collect();
    Ok(doc.update(&sel, &pairs))
}

#[napi]
pub fn replace_content(source: String, selector: String, new_body: String) -> Result<String> {
    let doc = marxml::parse(&source)
        .map_err(|e| napi::Error::new(napi::Status::InvalidArg, e.to_string()))?;
    let sel = marxml::Selector::parse(&selector)
        .map_err(|e| napi::Error::new(napi::Status::InvalidArg, e.to_string()))?;
    Ok(doc.replace_content(&sel, &new_body))
}

#[napi]
pub fn replace_in_content(
    source: String,
    selector: String,
    pattern: Either<String, RegExpShape>,
    replacement: String,
) -> Result<String> {
    let doc = marxml::parse(&source)
        .map_err(|e| napi::Error::new(napi::Status::InvalidArg, e.to_string()))?;
    let sel = marxml::Selector::parse(&selector)
        .map_err(|e| napi::Error::new(napi::Status::InvalidArg, e.to_string()))?;
    let pattern_str = match pattern {
        Either::A(s) => s,
        Either::B(rx) => rx.source,
    };
    let re = Regex::new(&pattern_str)
        .map_err(|e| napi::Error::new(napi::Status::InvalidArg, e.to_string()))?;
    Ok(doc.replace_in(&sel, &re, &replacement))
}

/// Accepts a JS `RegExp` by destructuring its serializable fields.
#[napi(object)]
pub struct RegExpShape {
    pub source: String,
    pub flags: Option<String>,
}

#[napi]
pub fn to_xml(source: String, pretty: Option<bool>) -> Result<String> {
    let doc = marxml::parse(&source)
        .map_err(|e| napi::Error::new(napi::Status::InvalidArg, e.to_string()))?;
    let opts = if pretty.unwrap_or(false) {
        marxml::SerializeOpts::pretty()
    } else {
        marxml::SerializeOpts::default()
    };
    Ok(doc.to_xml(&opts))
}

#[napi]
pub fn to_json(source: String) -> Result<String> {
    let doc = marxml::parse(&source)
        .map_err(|e| napi::Error::new(napi::Status::InvalidArg, e.to_string()))?;
    serde_json::to_string(&doc.to_json())
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, e.to_string()))
}

#[napi]
#[allow(clippy::implicit_hasher)] // napi-rs requires concrete HashMap
pub fn validate_schema(
    source: String,
    schema: HashMap<String, TagSchemaShape>,
) -> Result<ValidationReport> {
    let doc = marxml::parse(&source)
        .map_err(|e| napi::Error::new(napi::Status::InvalidArg, e.to_string()))?;
    let schema = build_schema(schema);
    let report = marxml::validate(&doc, &schema);
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

fn build_schema(input: HashMap<String, TagSchemaShape>) -> marxml::Schema {
    let mut builder = marxml::Schema::builder();
    for (tag_name, shape) in input {
        builder = builder.tag(&tag_name, |mut tb| {
            if let Some(attrs) = shape.attrs {
                for (name, c) in attrs {
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
    builder.build()
}

fn error_kind(e: &marxml::ValidationError) -> &'static str {
    match e {
        marxml::ValidationError::MissingAttr { .. } => "missing_attr",
        marxml::ValidationError::InvalidAttr { .. } => "invalid_attr",
        marxml::ValidationError::MissingChild { .. } => "missing_child",
        marxml::ValidationError::UnexpectedChild { .. } => "unexpected_child",
        marxml::ValidationError::EmptyContent { .. } => "empty_content",
        // `ValidationError` is `#[non_exhaustive]`; future variants surface
        // with a generic kind until the bindings catch up.
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
