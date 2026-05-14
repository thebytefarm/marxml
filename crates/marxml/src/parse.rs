//! Token stream → element tree.
//!
//! Stack-based assembler. Same-tag nesting (`<a><a/></a>`) is supported —
//! every `Open` pushes onto the stack and every `Close` pops, with name-match
//! enforcement.

use std::collections::{HashMap, HashSet};

use crate::error::ParseError;
use crate::tokenizer::{tokenize, Token};
use crate::types::{ElementData, SourcePosition, SourceSpan};
use crate::Markdown;

/// Parse a full markdown+XML document.
///
/// # Errors
///
/// Returns [`ParseError`] on malformed tags, unmatched close tags, unclosed
/// tags, or duplicate sibling `id` attributes within the same tag name.
pub fn parse(input: &str) -> Result<Markdown, ParseError> {
    let tokens = tokenize(input)?;
    assemble(input, tokens)
}

/// Parse a fragment.
///
/// Identical to [`parse`] for now — kept as a separate entry point so future
/// API can differentiate (e.g. doctype rejection on `parse_fragment`).
///
/// # Errors
///
/// See [`parse`].
pub fn parse_fragment(input: &str) -> Result<Markdown, ParseError> {
    parse(input)
}

struct Frame {
    name: String,
    attrs: Vec<(String, String)>,
    body_start: usize,
    span_start: SourcePosition,
    children: Vec<ElementData>,
}

fn assemble(input: &str, tokens: Vec<Token>) -> Result<Markdown, ParseError> {
    let mut stack: Vec<Frame> = Vec::new();
    let mut roots: Vec<ElementData> = Vec::new();
    let mut seen_ids: HashMap<String, HashSet<String>> = HashMap::new();

    for token in tokens {
        match token {
            Token::Open {
                name,
                attrs,
                span,
                body_start,
            } => {
                stack.push(Frame {
                    name,
                    attrs,
                    body_start,
                    span_start: span.start,
                    children: Vec::new(),
                });
            }

            Token::SelfClose { name, attrs, span } => {
                check_duplicate_id(&name, &attrs, span.start.line, &mut seen_ids)?;
                let empty_pos = usize::try_from(span.end.offset).unwrap_or(usize::MAX);
                let elem = ElementData {
                    tag: name,
                    attrs,
                    content_range: empty_pos..empty_pos,
                    children: Vec::new(),
                    span,
                    self_closing: true,
                };
                push_element(elem, &mut stack, &mut roots);
            }

            Token::Close {
                name,
                span,
                body_end,
            } => {
                let frame = stack.pop().ok_or_else(|| ParseError::StrayClose {
                    tag: name.clone(),
                    line: span.start.line,
                })?;
                if frame.name != name {
                    return Err(ParseError::MismatchedClose {
                        found: name,
                        expected: frame.name,
                        line: span.start.line,
                    });
                }
                check_duplicate_id(
                    &frame.name,
                    &frame.attrs,
                    frame.span_start.line,
                    &mut seen_ids,
                )?;
                let full_span = SourceSpan {
                    start: frame.span_start,
                    end: span.end,
                };
                let elem = ElementData {
                    tag: frame.name,
                    attrs: frame.attrs,
                    content_range: frame.body_start..body_end,
                    children: frame.children,
                    span: full_span,
                    self_closing: false,
                };
                push_element(elem, &mut stack, &mut roots);
            }
        }
    }

    if let Some(unclosed) = stack.into_iter().next() {
        return Err(ParseError::UnclosedTag {
            tag: unclosed.name,
            line: unclosed.span_start.line,
        });
    }

    Ok(Markdown::from_parts(input.to_string(), roots))
}

fn push_element(elem: ElementData, stack: &mut [Frame], roots: &mut Vec<ElementData>) {
    if let Some(top) = stack.last_mut() {
        top.children.push(elem);
    } else {
        roots.push(elem);
    }
}

fn check_duplicate_id(
    tag: &str,
    attrs: &[(String, String)],
    line: u32,
    seen: &mut HashMap<String, HashSet<String>>,
) -> Result<(), ParseError> {
    let id = attrs
        .iter()
        .find(|(k, _)| k == "id")
        .map(|(_, v)| v.as_str());
    let Some(id) = id else {
        return Ok(());
    };
    let bucket = seen.entry(tag.to_string()).or_default();
    if !bucket.insert(id.to_string()) {
        return Err(ParseError::DuplicateId {
            tag: tag.to_string(),
            id: id.to_string(),
            line,
        });
    }
    Ok(())
}
