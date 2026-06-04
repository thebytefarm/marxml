//! Token stream → element tree.
//!
//! Stack-based assembler. Same-tag nesting (`<a><a/></a>`) is supported —
//! every `Open` pushes onto the stack and every `Close` pops, with name-match
//! enforcement.
//!
//! Duplicate `id` detection is scoped to siblings under the same parent.
//! `<a><task id="1"/></a><b><task id="1"/></b>` is accepted because the two
//! `task` elements live in different parents; only repeats within the same
//! parent's children are rejected.

use std::collections::{HashMap, HashSet};

use crate::error::ParseError;
use crate::tokenizer::{tokenize, Token, TokenStream};
use crate::types::{ElementData, SourcePosition, SourceSpan};
use crate::Markdown;

/// Maximum depth of element nesting accepted by the parser.
///
/// Adversarial documents with thousands of nested tags would otherwise let
/// recursive tree walks (validation, serialization, selector matching) blow
/// the stack. The limit is intentionally generous for hand-authored markdown
/// while staying well below the default Rust stack size.
pub const MAX_DEPTH: u32 = 1024;

/// Maximum byte length of input accepted by the parser.
///
/// Source positions are stored as `u32` for compact element storage; inputs
/// larger than this cannot have their offsets tracked accurately and are
/// rejected up front rather than silently producing wrong spans.
///
/// The bound is `u32::MAX - 1` (not `u32::MAX`) so the worst-case line
/// counter `1 + count('\n')` is always ≤ `u32::MAX`. With the tighter cap
/// the line bump can use `saturating_add` for safety without ever actually
/// saturating on a well-formed input.
pub const MAX_INPUT_BYTES: usize = u32::MAX as usize - 1;

/// Parse a full markdown+XML document.
///
/// # Errors
///
/// Returns [`ParseError`] on malformed tags, unmatched close tags, unclosed
/// tags, duplicate sibling `id` attributes within the same parent and tag
/// name, nesting deeper than [`MAX_DEPTH`], or inputs larger than
/// [`MAX_INPUT_BYTES`].
///
/// ```
/// let doc = marxml::parse("# heading\n\n<task id=\"1\">do thing</task>")?;
/// assert_eq!(doc.root_count(), 1);
/// assert!(doc.raw().contains("# heading"));
/// # Ok::<(), marxml::ParseError>(())
/// ```
pub fn parse(input: &str) -> Result<Markdown, ParseError> {
    parse_owned(input.to_string())
}

/// Parse a fragment.
///
/// Currently identical to [`parse`]. Kept as a separate entry point so a
/// future divergence (e.g. doctype rejection on `parse_fragment`) can land
/// without a breaking rename.
///
/// # Errors
///
/// See [`parse`].
pub fn parse_fragment(input: &str) -> Result<Markdown, ParseError> {
    parse(input)
}

/// Parse a document, moving in the owned source string instead of cloning
/// it. Use this when you already own a `String` and want to avoid the
/// allocation that [`parse`] would do internally.
///
/// # Errors
///
/// See [`parse`].
///
/// ```
/// let src = String::from(r#"<task id="1"/>"#);
/// let doc = marxml::parse_owned(src)?;
/// assert_eq!(doc.root_count(), 1);
/// # Ok::<(), marxml::ParseError>(())
/// ```
pub fn parse_owned(input: String) -> Result<Markdown, ParseError> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(ParseError::InputTooLarge {
            size: input.len() as u64,
            max: MAX_INPUT_BYTES as u64,
        });
    }
    let stream = tokenize(&input)?;
    assemble(input, stream)
}

/// Per-scope duplicate-id tracking: `tag -> set of ids seen at this scope`.
type IdScope = HashMap<String, HashSet<String>>;

struct Frame {
    name: String,
    attrs: Vec<(String, String)>,
    body_start: usize,
    span_start: SourcePosition,
    children: Vec<ElementData>,
    /// Sibling-scoped duplicate-id tracker for the children of this frame.
    /// Lazily allocated — only created when an id-bearing child appears.
    seen_ids: Option<IdScope>,
}

fn assemble(input: String, stream: TokenStream) -> Result<Markdown, ParseError> {
    let TokenStream { tokens, trivia } = stream;
    let mut stack: Vec<Frame> = Vec::new();
    let mut roots: Vec<ElementData> = Vec::new();
    let mut root_seen: Option<IdScope> = None;

    for token in tokens {
        match token {
            Token::Open {
                name,
                attrs,
                span,
                body_start,
            } => {
                if stack.len() >= MAX_DEPTH as usize {
                    return Err(ParseError::MaxDepthExceeded {
                        tag: name,
                        max: MAX_DEPTH,
                        line: span.start.line,
                    });
                }
                stack.push(Frame {
                    name,
                    attrs,
                    body_start,
                    span_start: span.start,
                    children: Vec::new(),
                    seen_ids: None,
                });
            }

            Token::SelfClose { name, attrs, span } => {
                // The self-close is a leaf, so total tree depth at this node
                // equals the open-frame stack + 1. Enforce that the tree
                // depth never exceeds MAX_DEPTH (so downstream recursive
                // walkers see the same bound as the parser advertises).
                if stack.len() >= MAX_DEPTH as usize {
                    return Err(ParseError::MaxDepthExceeded {
                        tag: name,
                        max: MAX_DEPTH,
                        line: span.start.line,
                    });
                }
                let scope = current_scope(&mut stack, &mut root_seen);
                check_duplicate_id(&name, &attrs, span.start.line, scope)?;
                let empty_pos = span.end.offset_usize();
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
                // let-else lets the error path move `name` directly instead
                // of cloning to satisfy the `ok_or_else` closure.
                let Some(frame) = stack.pop() else {
                    return Err(ParseError::StrayClose {
                        tag: name,
                        line: span.start.line,
                    });
                };
                if frame.name != name {
                    return Err(ParseError::MismatchedClose {
                        found: name,
                        expected: frame.name,
                        line: span.start.line,
                    });
                }
                let scope = current_scope(&mut stack, &mut root_seen);
                check_duplicate_id(&frame.name, &frame.attrs, frame.span_start.line, scope)?;
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

    if let Some(unclosed) = stack.pop() {
        return Err(ParseError::UnclosedTag {
            tag: unclosed.name,
            line: unclosed.span_start.line,
        });
    }

    Ok(Markdown::from_parts(input, roots, trivia))
}

fn push_element(elem: ElementData, stack: &mut [Frame], roots: &mut Vec<ElementData>) {
    if let Some(top) = stack.last_mut() {
        top.children.push(elem);
    } else {
        roots.push(elem);
    }
}

/// The duplicate-id scope for the element currently being finalized — the
/// children-list of the enclosing frame, or the root scope when there is none.
fn current_scope<'a>(
    stack: &'a mut [Frame],
    root: &'a mut Option<IdScope>,
) -> &'a mut Option<IdScope> {
    if let Some(top) = stack.last_mut() {
        &mut top.seen_ids
    } else {
        root
    }
}

fn check_duplicate_id(
    tag: &str,
    attrs: &[(String, String)],
    line: u32,
    seen: &mut Option<IdScope>,
) -> Result<(), ParseError> {
    let Some(id) = attrs
        .iter()
        .find(|(k, _)| k == "id")
        .map(|(_, v)| v.as_str())
    else {
        return Ok(());
    };
    let scope = seen.get_or_insert_with(HashMap::new);
    if !scope
        .entry(tag.to_string())
        .or_default()
        .insert(id.to_string())
    {
        return Err(ParseError::DuplicateId {
            tag: tag.to_string(),
            id: id.to_string(),
            line,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn check_duplicate_id_noop_without_id_attribute() {
        let mut scope: Option<IdScope> = None;
        let a = attrs(&[("class", "x")]);
        assert!(check_duplicate_id("task", &a, 1, &mut scope).is_ok());
        // Should remain unallocated — the scope is lazy.
        assert!(scope.is_none());
    }

    #[test]
    fn check_duplicate_id_allocates_scope_on_first_id() {
        let mut scope: Option<IdScope> = None;
        let a = attrs(&[("id", "t1")]);
        assert!(check_duplicate_id("task", &a, 1, &mut scope).is_ok());
        assert!(scope.is_some());
    }

    #[test]
    fn check_duplicate_id_flags_repeat_within_same_tag() {
        let mut scope: Option<IdScope> = None;
        let a = attrs(&[("id", "t1")]);
        check_duplicate_id("task", &a, 1, &mut scope).unwrap();
        let err = check_duplicate_id("task", &a, 2, &mut scope).unwrap_err();
        assert!(
            matches!(err, ParseError::DuplicateId { ref tag, ref id, line: 2 } if tag == "task" && id == "t1")
        );
    }

    #[test]
    fn check_duplicate_id_scoped_by_tag_name() {
        // Same id literal on two different tag names at the same scope is
        // fine — the scope key is (tag, id), not just id.
        let mut scope: Option<IdScope> = None;
        let a = attrs(&[("id", "x")]);
        check_duplicate_id("task", &a, 1, &mut scope).unwrap();
        assert!(check_duplicate_id("note", &a, 2, &mut scope).is_ok());
    }

    #[test]
    fn current_scope_routes_to_top_frame_then_root() {
        let mut stack: Vec<Frame> = Vec::new();
        let mut root_seen: Option<IdScope> = None;

        // Empty stack → root_seen.
        let scope = current_scope(&mut stack, &mut root_seen);
        assert!(scope.is_none());
        *scope = Some(HashMap::new());
        assert!(root_seen.is_some());

        // Non-empty stack → top frame's seen_ids, root_seen untouched.
        stack.push(Frame {
            name: "a".to_string(),
            attrs: Vec::new(),
            body_start: 0,
            span_start: SourcePosition { line: 1, offset: 0 },
            children: Vec::new(),
            seen_ids: None,
        });
        let scope = current_scope(&mut stack, &mut root_seen);
        assert!(scope.is_none()); // top frame starts fresh
        *scope = Some(HashMap::new());
        assert!(stack[0].seen_ids.is_some());
    }

    #[test]
    fn push_element_routes_to_top_or_roots() {
        let mut stack: Vec<Frame> = Vec::new();
        let mut roots: Vec<ElementData> = Vec::new();
        let elem = ElementData {
            tag: "a".into(),
            attrs: Vec::new(),
            content_range: 0..0,
            children: Vec::new(),
            span: SourceSpan {
                start: SourcePosition { line: 1, offset: 0 },
                end: SourcePosition { line: 1, offset: 4 },
            },
            self_closing: true,
        };

        // Empty stack → element becomes a root.
        push_element(elem.clone(), &mut stack, &mut roots);
        assert_eq!(roots.len(), 1);

        // Non-empty stack → element appended to top frame's children.
        stack.push(Frame {
            name: "outer".into(),
            attrs: Vec::new(),
            body_start: 0,
            span_start: SourcePosition { line: 1, offset: 0 },
            children: Vec::new(),
            seen_ids: None,
        });
        push_element(elem, &mut stack, &mut roots);
        assert_eq!(roots.len(), 1, "root count unchanged");
        assert_eq!(stack[0].children.len(), 1);
    }
}
