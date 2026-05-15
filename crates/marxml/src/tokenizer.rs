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
pub(crate) fn tokenize(input: &str) -> Result<TokenStream, ParseError> {
    let bytes = input.as_bytes();
    let mut tokens = Vec::new();
    let mut trivia: Vec<core::ops::Range<usize>> = Vec::new();
    let mut i = 0;
    let mut line: u32 = 1;

    while i < bytes.len() {
        if bytes[i] == b'<' {
            if let Some((new_i, new_line, kind)) = skip_comment_or_cdata(bytes, i, line)? {
                match kind {
                    TriviaKind::Comment => trivia.push(i..new_i),
                    TriviaKind::Cdata => {
                        // For `<![CDATA[CONTENT]]>` we surface only the open
                        // and close brackets as trivia; CONTENT falls through
                        // as ordinary text so consumers see the user-authored
                        // bytes.
                        let open_end = i + 9; // `<![CDATA[`
                        let close_start = new_i - 3; // `]]>`
                        trivia.push(i..open_end);
                        trivia.push(close_start..new_i);
                    }
                }
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
/// Returns the parsed `Token`, the byte index just past the tag, and the
/// updated line counter.
#[allow(clippy::too_many_lines)]
fn parse_tag(
    input: &str,
    start: usize,
    start_line: u32,
) -> Result<(Token, usize, u32), ParseError> {
    let bytes = input.as_bytes();
    let span_start = SourcePosition {
        line: start_line,
        offset: u32::try_from(start).unwrap_or(u32::MAX),
    };

    let mut i = start + 1; // step past `<`
    let mut line = start_line;

    let is_close = bytes[i] == b'/';
    if is_close {
        i += 1;
    }

    // Tag name. `looks_like_tag_start` is the precondition; it guarantees the
    // next byte after `<` (or `</`) is a name-start byte, so the first
    // iteration of this loop always consumes at least one byte.
    let name_start = i;
    while i < bytes.len() && is_name_char(bytes[i]) {
        i += 1;
    }
    let name = input[name_start..i].to_string();

    if is_close {
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
            offset: u32::try_from(i).unwrap_or(u32::MAX),
        };
        return Ok((
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
        ));
    }

    // Attributes. Duplicates are rejected so downstream consumers (selectors,
    // serialization, mutation) all agree on which value belongs to a name.
    // Detection starts as a linear scan (fast on tags with few attrs) and
    // promotes to a `HashSet` once attribute count crosses the threshold so
    // pathological tags stay linear in attribute count.
    let mut attrs: Vec<(String, String)> = Vec::new();
    let mut seen_set: Option<HashSet<String>> = None;
    loop {
        skip_ws(bytes, &mut i, &mut line);
        if i >= bytes.len() {
            return Err(ParseError::MalformedTag {
                reason: format!("<{name}> not terminated"),
                line: start_line,
            });
        }
        match bytes[i] {
            b'>' => {
                i += 1;
                let span_end = SourcePosition {
                    line,
                    offset: u32::try_from(i).unwrap_or(u32::MAX),
                };
                return Ok((
                    Token::Open {
                        name,
                        attrs,
                        span: SourceSpan {
                            start: span_start,
                            end: span_end,
                        },
                        body_start: i,
                    },
                    i,
                    line,
                ));
            }
            b'/' => {
                i += 1;
                if i >= bytes.len() || bytes[i] != b'>' {
                    return Err(ParseError::MalformedTag {
                        reason: format!("expected '>' after '/' in <{name}/>"),
                        line: start_line,
                    });
                }
                i += 1;
                let span_end = SourcePosition {
                    line,
                    offset: u32::try_from(i).unwrap_or(u32::MAX),
                };
                return Ok((
                    Token::SelfClose {
                        name,
                        attrs,
                        span: SourceSpan {
                            start: span_start,
                            end: span_end,
                        },
                    },
                    i,
                    line,
                ));
            }
            _ => {
                let (key, value, new_i, new_line) = parse_attribute(input, &name, i, line)?;
                let is_dup = if let Some(set) = seen_set.as_mut() {
                    !set.insert(key.clone())
                } else {
                    attrs.iter().any(|(k, _)| k == &key)
                };
                if is_dup {
                    return Err(ParseError::DuplicateAttr {
                        tag: name,
                        attr: key,
                        line,
                    });
                }
                if seen_set.is_none() && attrs.len() + 1 >= ATTR_DUP_SET_THRESHOLD {
                    let mut set: HashSet<String> = HashSet::with_capacity(attrs.len() + 1);
                    for (k, _) in &attrs {
                        set.insert(k.clone());
                    }
                    set.insert(key.clone());
                    seen_set = Some(set);
                }
                attrs.push((key, value));
                i = new_i;
                line = new_line;
            }
        }
    }
}

fn parse_attribute(
    input: &str,
    tag_name: &str,
    start: usize,
    start_line: u32,
) -> Result<(String, String, usize, u32), ParseError> {
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

    Ok((key, value, i, line))
}

/// Distinguishes the two kinds of trivia the tokenizer recognizes. Comments
/// disappear entirely; CDATA keeps its inner content as text (only the
/// markers become trivia).
enum TriviaKind {
    Comment,
    Cdata,
}

/// If `bytes[i..]` starts with `<!--` or `<![CDATA[`, advance past the
/// matching `-->` / `]]>` and return the new index, line counter, and the
/// trivia kind. Returns `Ok(None)` when no such construct is present at `i`.
fn skip_comment_or_cdata(
    bytes: &[u8],
    start: usize,
    start_line: u32,
) -> Result<Option<(usize, u32, TriviaKind)>, ParseError> {
    if bytes[start..].starts_with(b"<!--") {
        let mut i = start + 4;
        let mut line = start_line;
        while i + 2 < bytes.len() {
            if &bytes[i..i + 3] == b"-->" {
                return Ok(Some((i + 3, line, TriviaKind::Comment)));
            }
            if bytes[i] == b'\n' {
                line = line.saturating_add(1);
            }
            i += 1;
        }
        return Err(ParseError::MalformedTag {
            reason: "unterminated <!-- comment".to_string(),
            line: start_line,
        });
    }
    if bytes[start..].starts_with(b"<![CDATA[") {
        let mut i = start + 9;
        let mut line = start_line;
        while i + 2 < bytes.len() {
            if &bytes[i..i + 3] == b"]]>" {
                return Ok(Some((i + 3, line, TriviaKind::Cdata)));
            }
            if bytes[i] == b'\n' {
                line = line.saturating_add(1);
            }
            i += 1;
        }
        return Err(ParseError::MalformedTag {
            reason: "unterminated <![CDATA[ section".to_string(),
            line: start_line,
        });
    }
    Ok(None)
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
