//! Validate a parsed [`Markdown`] against a [`Schema`].

use regex::Regex;
use thiserror::Error;

use crate::schema::{AttrKind, Schema, TagSchema};
use crate::types::ElementData;
use crate::Markdown;

/// One problem found during validation. Every variant carries a 1-based
/// `line` number and the offending `tag` for diagnostic output.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
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
    #[error("line {line}: <{tag}> attribute {attr} has invalid value {value:?} ({reason})")]
    InvalidAttr {
        /// Tag carrying the attribute.
        tag: String,
        /// Attribute name.
        attr: String,
        /// Offending value, as stored on the element.
        value: String,
        /// Short description of which constraint failed.
        reason: String,
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
    /// The element's inner content was empty but the schema marked it
    /// `content_required`.
    #[error("line {line}: <{tag}> requires non-empty content")]
    EmptyContent {
        /// Tag with the empty body.
        tag: String,
        /// 1-based source line.
        line: u32,
    },
}

/// Outcome of [`validate`].
#[derive(Debug, Clone)]
pub struct ValidationReport {
    valid: bool,
    errors: Vec<ValidationError>,
}

impl ValidationReport {
    /// `true` when the document conforms to the schema.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// All errors found, in source order.
    #[must_use]
    pub fn errors(&self) -> &[ValidationError] {
        &self.errors
    }
}

/// Validate every element in `doc` whose tag is named in `schema`.
///
/// Tags not present in the schema are not inspected. Validation continues
/// after the first error so callers see every issue at once.
#[must_use]
pub fn validate(doc: &Markdown, schema: &Schema) -> ValidationReport {
    let mut errors = Vec::new();
    for root in doc.roots_internal() {
        walk(root, doc.raw(), schema, &mut errors);
    }
    ValidationReport {
        valid: errors.is_empty(),
        errors,
    }
}

fn walk(node: &ElementData, raw: &str, schema: &Schema, errors: &mut Vec<ValidationError>) {
    if let Some(ts) = schema.tags.get(&node.tag) {
        check_element(node, raw, ts, errors);
    }
    for child in &node.children {
        walk(child, raw, schema, errors);
    }
}

fn check_element(node: &ElementData, raw: &str, ts: &TagSchema, errors: &mut Vec<ValidationError>) {
    let line = node.span.start.line;
    // Attribute checks.
    for (attr_name, constraint) in &ts.attrs {
        let value = node
            .attrs
            .iter()
            .find(|(k, _)| k == attr_name)
            .map(|(_, v)| v.as_str());
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
                if let Some(reason) = check_kind(&constraint.kind, v) {
                    errors.push(ValidationError::InvalidAttr {
                        tag: node.tag.clone(),
                        attr: attr_name.clone(),
                        value: v.to_string(),
                        reason,
                        line,
                    });
                }
            }
        }
    }

    // Children checks.
    for required in &ts.children_required {
        if !node.children.iter().any(|c| &c.tag == required) {
            errors.push(ValidationError::MissingChild {
                tag: node.tag.clone(),
                child: required.clone(),
                line,
            });
        }
    }
    if ts.children_exclusive {
        for child in &node.children {
            let allowed = ts.children_required.iter().any(|n| n == &child.tag)
                || ts.children_optional.iter().any(|n| n == &child.tag);
            if !allowed {
                errors.push(ValidationError::UnexpectedChild {
                    tag: node.tag.clone(),
                    child: child.tag.clone(),
                    line: child.span.start.line,
                });
            }
        }
    }

    // Content check.
    if ts.content_required {
        let content = &raw[node.content_range.clone()];
        if content.trim().is_empty() && node.children.is_empty() {
            errors.push(ValidationError::EmptyContent {
                tag: node.tag.clone(),
                line,
            });
        }
    }
}

fn check_kind(kind: &AttrKind, value: &str) -> Option<String> {
    match kind {
        AttrKind::String => None,
        AttrKind::Enum(allowed) => {
            if allowed.iter().any(|v| v == value) {
                None
            } else {
                Some(format!(
                    "expected one of [{}]",
                    allowed
                        .iter()
                        .map(|s| format!("{s:?}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            }
        }
        AttrKind::Regex(pat) => {
            // Schema::build pre-compiles to surface bad patterns early.
            let re = Regex::new(pat).expect("schema built with invalid regex");
            if re.is_match(value) {
                None
            } else {
                Some(format!("did not match regex /{pat}/"))
            }
        }
    }
}
