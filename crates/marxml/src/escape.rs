//! XML escaping helpers + XML-name predicates.
//!
//! The escaping helpers cover two contexts:
//! - [`escape_text`] — XML text content (`&`, `<`, `>`).
//! - [`escape_attr`] — XML attribute literal (`&`, `<`, `>`, `"`).
//!
//! Both helpers borrow when no escape is needed (returning [`Cow::Borrowed`])
//! to avoid an allocation on clean inputs, and own (`Cow::Owned`) otherwise.
//!
//! The name predicates ([`is_name_start`], [`is_name_char`], [`is_valid_name`])
//! are the single source of truth used by the tokenizer, the mutation
//! re-writer, and any other consumer that needs to recognize an XML name.
//! Lifting them here lets a future grammar tweak (namespace colons, Unicode
//! names, …) land in one place.

use std::borrow::Cow;

/// Escape `value` for use as XML text content.
///
/// Replaces `&`, `<`, `>` with their entity references. Quotes are left
/// alone — they have no meaning between tags. Control characters that XML
/// 1.0 forbids in content (NUL and most other C0 controls) are dropped on
/// the way through, so the output is always well-formed XML even when the
/// source slipped past upstream validation.
#[must_use]
pub fn escape_text(value: &str) -> Cow<'_, str> {
    if !value.bytes().any(needs_text_rewrite) {
        return Cow::Borrowed(value);
    }
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other if is_xml_illegal_control(other) => {} // drop
            other => out.push(other),
        }
    }
    Cow::Owned(out)
}

/// Escape `value` for use inside a double-quoted XML attribute literal.
///
/// Replaces `&`, `<`, `>`, and `"` with their entity references; drops
/// XML-illegal C0 control characters.
#[must_use]
pub fn escape_attr(value: &str) -> Cow<'_, str> {
    if !value.bytes().any(needs_attr_rewrite) {
        return Cow::Borrowed(value);
    }
    let mut out = String::with_capacity(value.len());
    push_escaped_attr(&mut out, value);
    Cow::Owned(out)
}

/// Append `value` into `out`, XML-escaping the characters that have meaning
/// inside a double-quoted attribute literal and dropping XML-illegal
/// control characters. The single in-place primitive every other module
/// (serialize, mutate) routes through.
pub(crate) fn push_escaped_attr(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            other if is_xml_illegal_control(other) => {} // drop
            other => out.push(other),
        }
    }
}

/// Append `value` into `out` with XML text escaping (handles `&`, `<`, `>`)
/// and drops XML-illegal control characters. Mirrors [`push_escaped_attr`]
/// but for text content where `"` carries no syntactic meaning.
pub(crate) fn push_escaped_text(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other if is_xml_illegal_control(other) => {} // drop
            other => out.push(other),
        }
    }
}

#[inline]
fn needs_text_rewrite(b: u8) -> bool {
    matches!(b, b'&' | b'<' | b'>') || maybe_illegal_control_byte(b)
}

#[inline]
fn needs_attr_rewrite(b: u8) -> bool {
    matches!(b, b'&' | b'<' | b'>' | b'"') || maybe_illegal_control_byte(b)
}

/// Byte-level prefilter for the escape fast path.
///
/// Returns `true` when the byte is either a direct illegal control (C0
/// plus DEL) OR the first byte of a UTF-8 sequence that *could* encode a
/// C1 control (`0xC2`). The per-char slow path applies the full
/// `is_xml_illegal_control` predicate to decide whether to actually drop.
/// Without the `0xC2` check, C1 controls (`\u{0080}..\u{009F}`, encoded as
/// `0xC2 0x80..0x9F`) would slip through the borrowed fast path.
#[inline]
fn maybe_illegal_control_byte(b: u8) -> bool {
    matches!(b, 0x00..=0x08 | 0x0B | 0x0C | 0x0E..=0x1F | 0x7F | 0xC2)
}

#[inline]
fn is_xml_illegal_control(ch: char) -> bool {
    let c = ch as u32;
    // C0 (excluding TAB/LF/CR), DEL, and the C1 control range.
    (c < 0x20 && c != 0x09 && c != 0x0A && c != 0x0D) || c == 0x7F || (0x80..=0x9F).contains(&c)
}

// ─── XML-whitespace predicate ────────────────────────────────────────────

/// `true` when every byte of `s` is one of the four XML whitespace bytes
/// (`' '`, `'\t'`, `'\r'`, `'\n'`).
///
/// Distinct from [`str::trim`], which uses Unicode whitespace semantics —
/// XML's whitespace grammar is ASCII-only, so structural-emptiness checks
/// inside an XML serializer/validator should honor that.
#[inline]
#[must_use]
pub(crate) fn is_xml_whitespace_only(s: &str) -> bool {
    s.bytes().all(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
}

// ─── XML-name predicates ─────────────────────────────────────────────────

/// `true` when `b` could be the first byte of an XML name (letter or `_`).
///
/// Intentionally ASCII-only: marxml accepts only ASCII XML names, which keeps
/// the byte-level tokenizer correct without UTF-8 decoding.
#[inline]
#[must_use]
pub fn is_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

/// `true` when `b` can appear in an XML name body (alpha-numeric, `-`, `_`,
/// `.`).
#[inline]
#[must_use]
pub fn is_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.'
}

/// `true` when `name` is a syntactically valid XML name as recognized by the
/// rest of the crate (non-empty, starts with a name-start byte, every
/// subsequent byte is a name-char).
#[must_use]
pub fn is_valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    let Some((first, rest)) = bytes.split_first() else {
        return false;
    };
    if !is_name_start(*first) {
        return false;
    }
    rest.iter().all(|b| is_name_char(*b))
}

// ─── Entity reference decoding ───────────────────────────────────────────

/// Decode the five XML predefined entity references (`&amp;`, `&lt;`, `&gt;`,
/// `&apos;`, `&quot;`) plus numeric character references (`&#NNN;` and
/// `&#xHH;`) in `value`.
///
/// Returns [`Cow::Borrowed`] when `value` contains no entity references at
/// all (the common case for hand-written documents). Multi-byte UTF-8 in
/// `value` is preserved unchanged.
///
/// Unknown entity references and malformed numeric references are passed
/// through verbatim — the tokenizer is permissive about content it doesn't
/// understand, mirroring the rest of the parser.
pub(crate) fn decode_entities(value: &str) -> Cow<'_, str> {
    if !value.bytes().any(|b| b == b'&') {
        return Cow::Borrowed(value);
    }
    let bytes = value.as_bytes();
    let mut out = String::with_capacity(value.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'&' {
            // XML entity references are short — capping the search window at
            // 16 bytes covers every legal reference (`&#x10FFFF;` is 10)
            // without scanning unbounded text on a stray `&`.
            let limit = (i + 16).min(bytes.len());
            if let Some(rel) = bytes[i + 1..limit].iter().position(|&b| b == b';') {
                let end = i + 1 + rel;
                let body = &value[i + 1..end];
                if let Some(decoded) = decode_one_entity(body) {
                    out.push(decoded);
                    i = end + 1;
                    continue;
                }
            }
            // Unknown / malformed — copy `&` verbatim and resume from the
            // next byte so `&unknown;` and `Tom & Jerry` both survive.
            out.push('&');
            i += 1;
            continue;
        }
        // Copy one whole UTF-8 scalar so multi-byte sequences stay intact.
        let ch = value[i..].chars().next().expect("non-empty tail");
        out.push(ch);
        i += ch.len_utf8();
    }
    Cow::Owned(out)
}

fn decode_one_entity(body: &str) -> Option<char> {
    match body {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "apos" => Some('\''),
        "quot" => Some('"'),
        _ => {
            let digits = body.strip_prefix('#')?;
            let code = if let Some(hex) = digits.strip_prefix(['x', 'X']) {
                u32::from_str_radix(hex, 16).ok()?
            } else {
                digits.parse::<u32>().ok()?
            };
            let ch = char::from_u32(code)?;
            // XML 1.0 §2.2 — only TAB, LF, CR, and the printable Unicode
            // ranges are valid characters. NUL and the other C0 controls
            // would round-trip into invalid XML; refuse the reference so
            // `&#0;` decodes to a literal `&#0;` rather than corrupting
            // output downstream.
            if is_valid_xml_char(ch) {
                Some(ch)
            } else {
                None
            }
        }
    }
}

#[inline]
fn is_valid_xml_char(ch: char) -> bool {
    // XML 1.0 §2.2 character classes, intersected with XML 1.1 restricted
    // ranges (DEL + C1 controls) so a decoded numeric reference can never
    // smuggle bytes that strict downstream parsers would refuse.
    matches!(ch,
        '\u{0009}' | '\u{000A}' | '\u{000D}'
        | '\u{0020}'..='\u{007E}'
        | '\u{00A0}'..='\u{D7FF}'
        | '\u{E000}'..='\u{FFFD}'
        | '\u{10000}'..='\u{10FFFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_escapes_lt_gt_amp() {
        assert_eq!(escape_text("a < b & c > d"), "a &lt; b &amp; c &gt; d");
    }

    #[test]
    fn text_leaves_quotes_alone() {
        assert_eq!(escape_text("he said \"hi\""), "he said \"hi\"");
    }

    #[test]
    fn attr_escapes_quotes_too() {
        assert_eq!(escape_attr("\" onclick=evil"), "&quot; onclick=evil");
    }

    #[test]
    fn passthrough_for_safe_input() {
        // Clean inputs should borrow rather than allocate.
        let text = escape_text("hello world");
        assert!(matches!(text, Cow::Borrowed(_)));
        assert_eq!(text, "hello world");
        let attr = escape_attr("hello world");
        assert!(matches!(attr, Cow::Borrowed(_)));
        assert_eq!(attr, "hello world");
    }

    #[test]
    fn handles_multibyte_utf8() {
        assert_eq!(escape_text("café — 日本"), "café — 日本");
        assert_eq!(escape_attr("café — 日本"), "café — 日本");
    }

    #[test]
    fn names_predicates() {
        assert!(is_valid_name("task"));
        assert!(is_valid_name("_data"));
        assert!(is_valid_name("a-b_c.d"));
        assert!(!is_valid_name(""));
        assert!(!is_valid_name("1abc"));
        assert!(!is_valid_name("a b"));
        assert!(!is_valid_name("a\"b"));
        assert!(!is_valid_name("a=b"));
    }
}
