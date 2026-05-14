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

use crate::error::ParseError;
use crate::types::{SourcePosition, SourceSpan};

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

/// Tokenize the entire input, returning every tag in source order.
///
/// Returns `Err` on the first malformed tag encountered.
pub(crate) fn tokenize(input: &str) -> Result<Vec<Token>, ParseError> {
    let bytes = input.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    let mut line: u32 = 1;

    while i < bytes.len() {
        if bytes[i] == b'<' && looks_like_tag_start(bytes, i + 1) {
            let (token, new_i, new_line) = parse_tag(input, i, line)?;
            tokens.push(token);
            i = new_i;
            line = new_line;
        } else {
            if bytes[i] == b'\n' {
                line += 1;
            }
            i += 1;
        }
    }

    Ok(tokens)
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

#[inline]
fn is_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

#[inline]
fn is_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.'
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

    // Attributes.
    let mut attrs: Vec<(String, String)> = Vec::new();
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

    // Attribute name.
    let name_start = i;
    while i < bytes.len() && is_name_char(bytes[i]) {
        i += 1;
    }
    if i == name_start {
        return Err(ParseError::MalformedAttribute {
            tag: tag_name.to_string(),
            reason: format!("unexpected character {:?}", bytes[i] as char),
            line,
        });
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
            line += 1;
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
    let value = input[value_start..i].to_string();
    i += 1; // past closing '"'

    Ok((key, value, i, line))
}

fn skip_ws(bytes: &[u8], i: &mut usize, line: &mut u32) {
    while *i < bytes.len() && bytes[*i].is_ascii_whitespace() {
        if bytes[*i] == b'\n' {
            *line += 1;
        }
        *i += 1;
    }
}
