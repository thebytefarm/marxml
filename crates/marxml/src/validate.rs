//! Validate a parsed [`Markdown`] against a [`Schema`].

use std::collections::{BTreeSet, HashMap};
use std::fmt;

use thiserror::Error;

use crate::escape::is_xml_whitespace_only;
use crate::schema::{CompiledAttrKind, CompiledTagSchema, Schema};
use crate::types::{ElementData, TextSegments};
use crate::Markdown;

/// One problem found during validation. Every variant carries a 1-based
/// `line` number and the offending `tag` for diagnostic output.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum ValidationError {
    /// A schema-required attribute was missing on a matched element.
    #[error("line {line}: <{tag}> missing required attribute {attr}")]
    MissingAttr {
        /// Tag where the attr should have been.
        tag: String,
        /// Attribute name.
        attr: String,
        /// 1-based source line.
        line: u32,
    },
    /// An attribute was present but its value didn't satisfy the schema
    /// (failed an enum/regex check).
    #[error("line {line}: <{tag}> attribute {attr} has invalid value {value:?} ({kind})")]
    InvalidAttr {
        /// Tag carrying the attribute.
        tag: String,
        /// Attribute name.
        attr: String,
        /// Offending value, as stored on the element.
        value: String,
        /// Specific constraint that failed, with the machine-readable
        /// context the schema would otherwise bury in a string.
        kind: InvalidAttrKind,
        /// 1-based source line.
        line: u32,
    },
    /// A required child element was missing.
    #[error("line {line}: <{tag}> missing required child <{child}>")]
    MissingChild {
        /// Parent tag.
        tag: String,
        /// Required child tag that was absent.
        child: String,
        /// 1-based source line of the parent.
        line: u32,
    },
    /// A child element was present that the schema's exclusive-children list
    /// did not allow.
    #[error("line {line}: <{tag}> has unexpected child <{child}>")]
    UnexpectedChild {
        /// Parent tag.
        tag: String,
        /// Child tag that was not in the allowlist.
        child: String,
        /// 1-based source line of the child.
        line: u32,
    },
    /// The element's inner text content was empty but the schema marked it
    /// `content_required`. Child-element markup does not satisfy this rule.
    #[error("line {line}: <{tag}> requires non-empty content")]
    EmptyContent {
        /// Tag with the empty body.
        tag: String,
        /// 1-based source line.
        line: u32,
    },
}

/// Which constraint inside a [`ValidationError::InvalidAttr`] failed.
///
/// The `Display` impl reproduces the legacy `"expected one of [...]"` /
/// `"did not match regex /…/"` rendering, so log lines and snapshot tests
/// see byte-identical output. Programmatic consumers (LSP servers, UI
/// surfacing) can `match` on the variant to recover the allowed set or
/// the pattern without re-parsing the message.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InvalidAttrKind {
    /// The value was not in the schema's enum allowlist.
    NotInEnum {
        /// Values the schema permits, in stable lexical order.
        allowed: Vec<String>,
    },
    /// The value failed the schema's regex constraint.
    NoRegexMatch {
        /// The pattern source, as supplied to [`crate::AttrKind::Regex`]
        /// (without the implicit `\A(?:…)\z` anchors).
        pattern: String,
    },
}

impl fmt::Display for InvalidAttrKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInEnum { allowed } => {
                f.write_str("expected one of [")?;
                let mut first = true;
                for v in allowed {
                    if !first {
                        f.write_str(", ")?;
                    }
                    first = false;
                    write!(f, "{v:?}")?;
                }
                f.write_str("]")
            }
            Self::NoRegexMatch { pattern } => write!(f, "did not match regex /{pattern}/"),
        }
    }
}

/// Outcome of [`validate`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ValidationReport {
    errors: Vec<ValidationError>,
}

impl ValidationReport {
    /// `true` when the document conforms to the schema.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// All errors found, in source order.
    #[must_use]
    pub fn errors(&self) -> &[ValidationError] {
        &self.errors
    }

    /// Number of errors collected. `0` iff the document is valid.
    #[must_use]
    pub fn len(&self) -> usize {
        self.errors.len()
    }

    /// `true` when no errors were collected. Equivalent to [`Self::is_valid`].
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Iterate the errors by reference.
    pub fn iter(&self) -> std::slice::Iter<'_, ValidationError> {
        self.errors.iter()
    }
}

impl<'a> IntoIterator for &'a ValidationReport {
    type Item = &'a ValidationError;
    type IntoIter = std::slice::Iter<'a, ValidationError>;

    fn into_iter(self) -> Self::IntoIter {
        self.errors.iter()
    }
}

impl IntoIterator for ValidationReport {
    type Item = ValidationError;
    type IntoIter = std::vec::IntoIter<ValidationError>;

    fn into_iter(self) -> Self::IntoIter {
        self.errors.into_iter()
    }
}

/// Threshold past which `check_element` builds a `HashMap` over the
/// element's attributes for O(1) lookup. Below this, a linear scan over
/// `node.attrs` is faster (no hash setup cost). The same value mirrors the
/// tokenizer's duplicate-attr threshold.
const ATTR_MAP_THRESHOLD: usize = 16;

/// Validate every element in `doc` whose tag is named in `schema`.
///
/// Tags not present in the schema are not inspected. Validation continues
/// after the first error so callers see every issue at once.
///
/// ```
/// use marxml::{parse, schema::AttrKind, validate, Schema};
///
/// let schema = Schema::builder()
///     .tag("task", |t| t.attr("id", AttrKind::String.required()))
///     .build();
/// let doc = parse("<task/>")?;
/// let report = validate(&doc, &schema);
/// assert!(!report.errors().is_empty()); // missing required `id`
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn validate(doc: &Markdown, schema: &Schema) -> ValidationReport {
    let mut errors = Vec::new();
    for root in doc.roots_internal() {
        walk(root, doc.raw(), doc.trivia(), schema, &mut errors);
    }
    ValidationReport { errors }
}

fn walk(
    node: &ElementData,
    raw: &str,
    trivia: &[core::ops::Range<usize>],
    schema: &Schema,
    errors: &mut Vec<ValidationError>,
) {
    if let Some(ts) = schema.tags.get(&node.tag) {
        check_element(node, raw, trivia, ts, errors);
    }
    for child in &node.children {
        walk(child, raw, trivia, schema, errors);
    }
}

fn check_element(
    node: &ElementData,
    raw: &str,
    trivia: &[core::ops::Range<usize>],
    ts: &CompiledTagSchema,
    errors: &mut Vec<ValidationError>,
) {
    let line = node.span.start.line;
    // Attribute checks. For typical elements (<16 attrs each side) a linear
    // scan is faster than building a hashmap; past the threshold we
    // promote so machine-generated schemas/elements stay near-linear.
    let attr_map: Option<HashMap<&str, &str>> =
        if node.attrs.len() >= ATTR_MAP_THRESHOLD || ts.attrs.len() >= ATTR_MAP_THRESHOLD {
            Some(
                node.attrs
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect(),
            )
        } else {
            None
        };
    for (attr_name, constraint) in &ts.attrs {
        let value = if let Some(map) = &attr_map {
            map.get(attr_name.as_str()).copied()
        } else {
            node.attrs
                .iter()
                .find(|(k, _)| k == attr_name)
                .map(|(_, v)| v.as_str())
        };
        match value {
            None => {
                if constraint.required {
                    errors.push(ValidationError::MissingAttr {
                        tag: node.tag.clone(),
                        attr: attr_name.clone(),
                        line,
                    });
                }
            }
            Some(v) => {
                if let Some(kind) = check_kind(&constraint.kind, v) {
                    errors.push(ValidationError::InvalidAttr {
                        tag: node.tag.clone(),
                        attr: attr_name.clone(),
                        value: v.to_string(),
                        kind,
                        line,
                    });
                }
            }
        }
    }

    // Children checks. Build a tag set over `node.children` once so both the
    // required-name lookup and the exclusive-allow walk are O(c + r + |allow|)
    // total rather than O(c * (r + |allow|)).
    let child_tags: BTreeSet<&str> = node.children.iter().map(|c| c.tag.as_str()).collect();
    for required in &ts.children_required {
        if !child_tags.contains(required.as_str()) {
            errors.push(ValidationError::MissingChild {
                tag: node.tag.clone(),
                child: required.clone(),
                line,
            });
        }
    }
    if ts.children_exclusive {
        for child in &node.children {
            if !ts.children_allowed.contains(&child.tag) {
                errors.push(ValidationError::UnexpectedChild {
                    tag: node.tag.clone(),
                    child: child.tag.clone(),
                    line: child.span.start.line,
                });
            }
        }
    }

    // Content check: text-only. Child-element markup and comment/CDATA
    // trivia do not count toward satisfying `content_required`.
    if ts.content_required {
        let has_text =
            TextSegments::new_with_trivia(raw, node, trivia).any(|s| !is_xml_whitespace_only(s));
        if !has_text {
            errors.push(ValidationError::EmptyContent {
                tag: node.tag.clone(),
                line,
            });
        }
    }
}

fn check_kind(kind: &CompiledAttrKind, value: &str) -> Option<InvalidAttrKind> {
    match kind {
        CompiledAttrKind::String => None,
        CompiledAttrKind::Enum(allowed) => {
            if allowed.contains(value) {
                None
            } else {
                Some(InvalidAttrKind::NotInEnum {
                    allowed: allowed.iter().cloned().collect(),
                })
            }
        }
        CompiledAttrKind::Regex(re) => {
            if re.is_match(value) {
                None
            } else {
                Some(InvalidAttrKind::NoRegexMatch {
                    pattern: re.as_str().to_string(),
                })
            }
        }
    }
}
