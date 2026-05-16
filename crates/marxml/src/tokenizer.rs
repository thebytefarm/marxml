//! Tag-level tokenizer.
//!
//! Scans the input once, emitting one [`Token`] per XML tag found. Non-tag
//! text between tags is skipped — we don't need it for the element tree, and
//! the [`crate::Markdown::raw`] string retains the original byte-for-byte.
//!
//! Recognized tag forms:
//! - `<name attr="value" …>` — open
//! - `<name attr="value" … />` — self-close
//! - `</name>` — close
//!
//! A `<` that isn't immediately followed by a tag-name-start (letter or `_`)
//! or `/`+name-start is treated as literal text and skipped, *not* an error.
//! This is what lets prose like "if x < 3" survive parsing untouched.
//!
//! XML name predicates ([`is_name_char`] etc.) live in [`crate::escape`] so
//! the parser, the mutator, and any other consumer all agree on what
//! constitutes a name byte.

use std::collections::HashSet;

use crate::error::ParseError;
use crate::escape::{decode_entities, is_name_char, is_name_start};
use crate::types::{SourcePosition, SourceSpan};

/// Threshold past which attribute-name duplicate detection switches from a
/// linear scan to a `HashSet` lookup. Real-world tags carry a handful of
/// attributes (linear scan is fastest); machine-generated tags can carry
/// thousands (quadratic dominates). The crossover is well-covered at 16.
const ATTR_DUP_SET_THRESHOLD: usize = 16;

/// Narrow a byte offset into the (bounded) input back to `u32` for storage in
/// a `SourcePosition`. `parse::parse` validates `input.len() <= u32::MAX`
/// before calling into the tokenizer, so every offset we see here fits.
#[inline]
fn offset_u32(n: usize) -> u32 {
    u32::try_from(n).expect("offset within MAX_INPUT_BYTES — checked at parse entry")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Token {
    Open {
        name: String,
        attrs: Vec<(String, String)>,
        span: SourceSpan,
        /// Byte index just past `>` — start of body for open tags.
        body_start: usize,
    },
    Close {
        name: String,
        span: SourceSpan,
        /// Byte index of the `<` — end of body for matching open tags.
        body_end: usize,
    },
    SelfClose {
        name: String,
        attrs: Vec<(String, String)>,
        span: SourceSpan,
    },
}

/// Result of tokenization: the recognized tag tokens plus the byte ranges
/// of XML trivia (comments and CDATA sections) that should be skipped when
/// extracting an element's text content. The ranges are in ascending,
/// non-overlapping order.
#[derive(Debug, Clone, Default)]
pub(crate) struct TokenStream {
    pub tokens: Vec<Token>,
    pub trivia: Vec<core::ops::Range<usize>>,
}

/// Tokenize the entire input, returning every tag in source order plus the
/// byte ranges of every comment / CDATA section.
///
/// Returns `Err` on the first malformed tag encountered.
///
/// # Panics
///
/// Debug builds assert `input.len() <= u32::MAX`. `parse::parse` enforces
/// this at its public entry, so this is a defense-in-depth check for the
/// `pub(crate)` boundary — release builds elide it.
pub(crate) fn tokenize(input: &str) -> Result<TokenStream, ParseError> {
    debug_assert!(
        u32::try_from(input.len()).is_ok(),
        "tokenize() requires input bounded by MAX_INPUT_BYTES"
    );
    let bytes = input.as_bytes();
    let mut tokens = Vec::new();
    let mut trivia: Vec<core::ops::Range<usize>> = Vec::new();
    let mut i = 0;
    let mut line: u32 = 1;

    while i < bytes.len() {
        if bytes[i] == b'<' {
            if let Some((new_i, new_line)) = try_skip_comment(bytes, i, line)? {
                trivia.push(i..new_i);
                i = new_i;
                line = new_line;
                continue;
            }
            if let Some((new_i, new_line)) = try_skip_cdata(bytes, i, line)? {
                // For `<![CDATA[CONTENT]]>` only the brackets become trivia;
                // CONTENT falls through as ordinary text so consumers see the
                // user-authored bytes.
                let open_end = i + 9; // `<![CDATA[`
                let close_start = new_i - 3; // `]]>`
                trivia.push(i..open_end);
                trivia.push(close_start..new_i);
                i = new_i;
                line = new_line;
                continue;
            }
            if looks_like_tag_start(bytes, i + 1) {
                let (token, new_i, new_line) = parse_tag(input, i, line)?;
                tokens.push(token);
                i = new_i;
                line = new_line;
                continue;
            }
        }
        if bytes[i] == b'\n' {
            line = line.saturating_add(1);
        }
        i += 1;
    }

    Ok(TokenStream { tokens, trivia })
}

/// `true` if the bytes starting at `i` look like the body of a tag — either a
/// tag-name-start byte directly, or `/` followed by a tag-name-start byte.
fn looks_like_tag_start(bytes: &[u8], i: usize) -> bool {
    if i >= bytes.len() {
        return false;
    }
    if bytes[i] == b'/' {
        i + 1 < bytes.len() && is_name_start(bytes[i + 1])
    } else {
        is_name_start(bytes[i])
    }
}

/// Parse one tag starting at `start` (which must point at `<`).
///
/// Dispatches to [`parse_end_tag`] for `</...>` and [`parse_opening_tag`] for
/// `<...>` / `<.../>`. Returns the parsed `Token`, the byte index just past
/// the tag, and the updated line counter.
fn parse_tag(
    input: &str,
    start: usize,
    start_line: u32,
) -> Result<(Token, usize, u32), ParseError> {
    let bytes = input.as_bytes();
    let span_start = SourcePosition {
        line: start_line,
        offset: offset_u32(start),
    };
    if bytes.get(start + 1) == Some(&b'/') {
        parse_end_tag(input, start, start_line, span_start)
    } else {
        parse_opening_tag(input, start, start_line, span_start)
    }
}

/// Parse `</name>` and produce a [`Token::Close`].
///
/// `start` points at `<`; `looks_like_tag_start` guarantees the bytes that
/// follow are `/` + name-start, so the name scan always advances at least
/// one byte.
fn parse_end_tag(
    input: &str,
    start: usize,
    start_line: u32,
    span_start: SourcePosition,
) -> Result<(Token, usize, u32), ParseError> {
    let bytes = input.as_bytes();
    let mut i = start + 2; // step past `</`
    let mut line = start_line;
    let name = read_tag_name(input, &mut i);
    skip_ws(bytes, &mut i, &mut line);
    if i >= bytes.len() || bytes[i] != b'>' {
        return Err(ParseError::MalformedTag {
            reason: format!("expected '>' to close </{name}>"),
            line: start_line,
        });
    }
    i += 1;
    let span_end = SourcePosition {
        line,
        offset: offset_u32(i),
    };
    Ok((
        Token::Close {
            name,
            span: SourceSpan {
                start: span_start,
                end: span_end,
            },
            body_end: start,
        },
        i,
        line,
    ))
}

/// Distinguishes the two forms an opening tag can take. Returned by
/// [`parse_attribute_list`] so the caller can build the right `Token`.
enum TagForm {
    /// `<name …>` — matching `</name>` follows somewhere later.
    Pair,
    /// `<name … />` — opens and closes in one tag.
    SelfClosing,
}

/// Result of parsing a tag's attribute list. The cursor (`end`, `line`) is
/// positioned just past the terminator (`>` or `/>`) so the caller can build
/// the appropriate `Token` without re-scanning.
struct AttributeList {
    attrs: Vec<(String, String)>,
    form: TagForm,
    end: usize,
    line: u32,
}

/// Parse `<name …>` or `<name … />` and produce the matching `Token`.
fn parse_opening_tag(
    input: &str,
    start: usize,
    start_line: u32,
    span_start: SourcePosition,
) -> Result<(Token, usize, u32), ParseError> {
    let mut i = start + 1; // step past `<`
    let mut line = start_line;
    let name = read_tag_name(input, &mut i);
    let list = parse_attribute_list(input, &name, start_line, i, line)?;
    i = list.end;
    line = list.line;
    let span = SourceSpan {
        start: span_start,
        end: SourcePosition {
            line,
            offset: offset_u32(i),
        },
    };
    let token = match list.form {
        TagForm::Pair => Token::Open {
            name,
            attrs: list.attrs,
            span,
            body_start: i,
        },
        TagForm::SelfClosing => Token::SelfClose {
            name,
            attrs: list.attrs,
            span,
        },
    };
    Ok((token, i, line))
}

/// Scan a tag name. Advances `*i` past the trailing name-char run and returns
/// the borrowed slice as an owned `String`. The caller is responsible for
/// having positioned `*i` at the first name-start byte (the `looks_like_tag_start`
/// precondition enforces this for the tokenizer's dispatch sites).
fn read_tag_name(input: &str, i: &mut usize) -> String {
    let bytes = input.as_bytes();
    let name_start = *i;
    while *i < bytes.len() && is_name_char(bytes[*i]) {
        *i += 1;
    }
    input[name_start..*i].to_string()
}

/// Parse the attribute list of an opening tag, returning the collected
/// attributes plus the [`TagForm`] of the terminator that ended the list
/// (`>` → `Pair`, `/>` → `SelfClosing`).
///
/// Duplicates are rejected so downstream consumers (selectors, serialization,
/// mutation) all agree on which value belongs to a name. Detection starts as
/// a linear scan (fast on tags with few attrs) and promotes to a `HashSet`
/// once the count crosses [`ATTR_DUP_SET_THRESHOLD`] so pathological tags
/// stay linear in attribute count.
fn parse_attribute_list(
    input: &str,
    tag_name: &str,
    tag_start_line: u32,
    start: usize,
    start_line: u32,
) -> Result<AttributeList, ParseError> {
    let bytes = input.as_bytes();
    let mut i = start;
    let mut line = start_line;
    let mut attrs: Vec<(String, String)> = Vec::new();
    let mut seen_set: Option<HashSet<String>> = None;
    loop {
        skip_ws(bytes, &mut i, &mut line);
        if i >= bytes.len() {
            return Err(ParseError::MalformedTag {
                reason: format!("<{tag_name}> not terminated"),
                line: tag_start_line,
            });
        }
        match bytes[i] {
            b'>' => {
                return Ok(AttributeList {
                    attrs,
                    form: TagForm::Pair,
                    end: i + 1,
                    line,
                });
            }
            b'/' => {
                i += 1;
                if i >= bytes.len() || bytes[i] != b'>' {
                    return Err(ParseError::MalformedTag {
                        reason: format!("expected '>' after '/' in <{tag_name}/>"),
                        line: tag_start_line,
                    });
                }
                return Ok(AttributeList {
                    attrs,
                    form: TagForm::SelfClosing,
                    end: i + 1,
                    line,
                });
            }
            _ => {
                let parsed = parse_attribute(input, tag_name, i, line)?;
                if seen_attribute(&attrs, seen_set.as_ref(), &parsed.key) {
                    return Err(ParseError::DuplicateAttr {
                        tag: tag_name.to_string(),
                        attr: parsed.key,
                        line,
                    });
                }
                record_seen_attr(&attrs, &parsed.key, &mut seen_set);
                attrs.push((parsed.key, parsed.value));
                i = parsed.end;
                line = parsed.line;
            }
        }
    }
}

/// `true` when `key` already appears in `attrs`. Uses the promoted hash set
/// when one exists; falls back to a linear scan over `attrs` for the common
/// "few attributes" case.
fn seen_attribute(attrs: &[(String, String)], seen: Option<&HashSet<String>>, key: &str) -> bool {
    if let Some(set) = seen {
        set.contains(key)
    } else {
        attrs.iter().any(|(k, _)| k == key)
    }
}

/// Record `next_key` as a seen attribute. If a hash set already exists, insert
/// into it; if `attrs` is about to cross [`ATTR_DUP_SET_THRESHOLD`], lazily
/// build the set first so subsequent insertions stay O(1). Otherwise do
/// nothing — the linear-scan path in [`seen_attribute`] handles small inputs.
fn record_seen_attr(
    attrs: &[(String, String)],
    next_key: &str,
    seen: &mut Option<HashSet<String>>,
) {
    if let Some(set) = seen.as_mut() {
        set.insert(next_key.to_string());
        return;
    }
    if attrs.len() + 1 < ATTR_DUP_SET_THRESHOLD {
        return;
    }
    let mut set: HashSet<String> = HashSet::with_capacity(attrs.len() + 1);
    for (k, _) in attrs {
        set.insert(k.clone());
    }
    set.insert(next_key.to_string());
    *seen = Some(set);
}

/// One parsed `key="value"` attribute. Cursor (`end`, `line`) is positioned
/// just past the closing `"` so the caller can resume scanning without
/// re-walking the value.
struct ParsedAttribute {
    key: String,
    value: String,
    end: usize,
    line: u32,
}

fn parse_attribute(
    input: &str,
    tag_name: &str,
    start: usize,
    start_line: u32,
) -> Result<ParsedAttribute, ParseError> {
    let bytes = input.as_bytes();
    let mut i = start;
    let mut line = start_line;

    // Attribute name. The first byte must be a name-start (letter/_); the
    // remaining bytes use the looser name-char predicate. Without this the
    // tokenizer would happily accept invalid names like `1id` or `.x`.
    if i >= bytes.len() || !is_name_start(bytes[i]) {
        return Err(ParseError::MalformedAttribute {
            tag: tag_name.to_string(),
            reason: format!(
                "unexpected character {:?} at start of attribute name",
                next_char_at(input, i).unwrap_or('\0')
            ),
            line,
        });
    }
    let name_start = i;
    i += 1;
    while i < bytes.len() && is_name_char(bytes[i]) {
        i += 1;
    }
    let key = input[name_start..i].to_string();

    if i >= bytes.len() || bytes[i] != b'=' {
        return Err(ParseError::MalformedAttribute {
            tag: tag_name.to_string(),
            reason: format!("expected '=' after attribute {key}"),
            line,
        });
    }
    i += 1; // past '='

    if i >= bytes.len() || bytes[i] != b'"' {
        return Err(ParseError::MalformedAttribute {
            tag: tag_name.to_string(),
            reason: format!("expected '\"' to open value of {key}"),
            line,
        });
    }
    i += 1; // past opening '"'

    let value_start = i;
    while i < bytes.len() && bytes[i] != b'"' {
        if bytes[i] == b'\n' {
            line = line.saturating_add(1);
        }
        i += 1;
    }
    if i >= bytes.len() {
        return Err(ParseError::MalformedAttribute {
            tag: tag_name.to_string(),
            reason: format!("unterminated value of {key}"),
            line: start_line,
        });
    }
    // Decode the five XML predefined entities + numeric character references
    // here, once, so downstream code sees the canonical Unicode form. The
    // alternative — storing raw bytes and re-decoding at every consumer —
    // produces double-escape bugs on `to_xml` / `update` round-trips.
    let value = decode_entities(&input[value_start..i]).into_owned();
    i += 1; // past closing '"'

    Ok(ParsedAttribute {
        key,
        value,
        end: i,
        line,
    })
}

/// If `bytes[start..]` opens with `<!--`, advance past the matching `-->`
/// and return the new index plus the updated line counter. Returns `Ok(None)`
/// when there is no comment at `start`.
fn try_skip_comment(
    bytes: &[u8],
    start: usize,
    start_line: u32,
) -> Result<Option<(usize, u32)>, ParseError> {
    if !bytes[start..].starts_with(b"<!--") {
        return Ok(None);
    }
    scan_to_terminator(bytes, start + 4, start_line, b"-->")
        .ok_or_else(|| ParseError::MalformedTag {
            reason: "unterminated <!-- comment".to_string(),
            line: start_line,
        })
        .map(Some)
}

/// If `bytes[start..]` opens with `<![CDATA[`, advance past the matching
/// `]]>` and return the new index plus the updated line counter. Returns
/// `Ok(None)` when there is no CDATA section at `start`.
fn try_skip_cdata(
    bytes: &[u8],
    start: usize,
    start_line: u32,
) -> Result<Option<(usize, u32)>, ParseError> {
    if !bytes[start..].starts_with(b"<![CDATA[") {
        return Ok(None);
    }
    scan_to_terminator(bytes, start + 9, start_line, b"]]>")
        .ok_or_else(|| ParseError::MalformedTag {
            reason: "unterminated <![CDATA[ section".to_string(),
            line: start_line,
        })
        .map(Some)
}

/// Scan `bytes[from..]` for the byte sequence `terminator`. On a match,
/// return the byte index just past the terminator plus the line counter
/// reflecting any `\n` bytes consumed. Returns `None` when the terminator is
/// never found before end-of-input.
fn scan_to_terminator(
    bytes: &[u8],
    from: usize,
    start_line: u32,
    terminator: &[u8],
) -> Option<(usize, u32)> {
    let term_len = terminator.len();
    let mut i = from;
    let mut line = start_line;
    while i + term_len <= bytes.len() {
        if &bytes[i..i + term_len] == terminator {
            return Some((i + term_len, line));
        }
        if bytes[i] == b'\n' {
            line = line.saturating_add(1);
        }
        i += 1;
    }
    None
}

/// Read the next Unicode character starting at byte offset `i`, if any.
///
/// Used for error diagnostics so a malformed-attribute message displays a
/// real character instead of garbling a UTF-8 continuation byte.
fn next_char_at(input: &str, i: usize) -> Option<char> {
    input.get(i..).and_then(|tail| tail.chars().next())
}

fn skip_ws(bytes: &[u8], i: &mut usize, line: &mut u32) {
    while *i < bytes.len() && bytes[*i].is_ascii_whitespace() {
        if bytes[*i] == b'\n' {
            *line = line.saturating_add(1);
        }
        *i += 1;
    }
}
