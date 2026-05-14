//! CSS-subset selector parser.
//!
//! Grammar supported (0.1.0):
//!
//! ```text
//! selector       := compound ( WS* "," WS* compound )*
//! compound       := simple ( combinator simple )*
//! combinator     := WS* ">" WS*       // direct child
//!                 | WS+               // descendant
//! simple         := ( "*" | name ) predicate*
//! predicate      := "[" name ( ( "=" | "^=" | "$=" | "*=" ) quoted_str )? "]"
//!                 | ":" pseudo
//! pseudo         := "first-child"
//!                 | "nth-child(" digits ")"
//!                 | "not(" simple ")"
//! quoted_str     := '"' chars '"'
//! ```

use super::ast::{Combinator, CompiledSelector, Compound, Predicate, Simple};
use super::error::SelectorError;

pub(super) fn parse(input: &str) -> Result<CompiledSelector, SelectorError> {
    if input.trim().is_empty() {
        return Err(SelectorError::Empty);
    }
    let mut p = Parser::new(input);
    // `parse_compound` returns with the cursor at either end-of-input or
    // immediately before a `,`. Any other position would mean a bug in the
    // compound parser, so we don't recheck — we just consume commas.
    let mut compounds = vec![p.parse_compound()?];
    while !p.at_end() {
        p.advance(1); // consume the `,`
        p.skip_ws();
        compounds.push(p.parse_compound()?);
    }
    Ok(CompiledSelector { compounds })
}

struct Parser<'a> {
    bytes: &'a [u8],
    src: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            bytes: src.as_bytes(),
            src,
            pos: 0,
        }
    }

    fn parse_compound(&mut self) -> Result<Compound, SelectorError> {
        // Skip any leading whitespace (e.g. after `,`).
        self.skip_ws();
        // Build the chain left-to-right as parallel lists of simples and the
        // combinators between them. `simples.len() == links.len() + 1`.
        let mut simples: Vec<Simple> = vec![self.parse_simple()?];
        let mut links: Vec<Combinator> = Vec::new();
        loop {
            let saved = self.pos;
            let had_ws = self.skip_ws_returning_seen();
            if self.at_end() || self.peek() == Some(b',') {
                self.pos = saved;
                self.skip_ws();
                break;
            }
            let combinator = if self.peek() == Some(b'>') {
                self.advance(1);
                self.skip_ws();
                Combinator::Child
            } else if had_ws {
                Combinator::Descendant
            } else {
                return Err(self.syntax_error("expected combinator or ','"));
            };
            simples.push(self.parse_simple()?);
            links.push(combinator);
        }
        let subject = simples.pop().expect("at least one simple");
        let mut prefix: Vec<(Combinator, Simple)> = Vec::with_capacity(simples.len());
        while let (Some(simple), Some(link)) = (simples.pop(), links.pop()) {
            prefix.push((link, simple));
        }
        Ok(Compound { subject, prefix })
    }

    fn parse_simple(&mut self) -> Result<Simple, SelectorError> {
        // A simple selector is `tag | *` optionally followed by predicates,
        // OR (when tag/* is missing) one or more predicates with an implicit
        // universal target. The latter form lets users write `[id]` to mean
        // "any element with an `id` attribute", and is also what makes
        // `:not([id])` work — the inner simple has no leading tag.
        let (tag, had_marker) = if self.peek() == Some(b'*') {
            self.advance(1);
            (None, true)
        } else if self.peek().is_some_and(is_name_start) {
            (Some(self.read_name()), true)
        } else {
            (None, false)
        };
        let mut predicates = Vec::new();
        loop {
            match self.peek() {
                Some(b'[') => predicates.push(self.parse_attribute_predicate()?),
                Some(b':') => predicates.push(self.parse_pseudo()?),
                _ => break,
            }
        }
        if !had_marker && predicates.is_empty() {
            return Err(self.syntax_error("expected tag name, '*', or predicate"));
        }
        Ok(Simple { tag, predicates })
    }

    fn parse_attribute_predicate(&mut self) -> Result<Predicate, SelectorError> {
        self.advance(1); // '['
        if !self.peek().is_some_and(is_name_start) {
            return Err(self.syntax_error("expected attribute name after '['"));
        }
        let name = self.read_name();
        match self.peek() {
            Some(b']') => {
                self.advance(1);
                Ok(Predicate::HasAttr(name))
            }
            Some(b'=') => {
                self.advance(1);
                let value = self.read_quoted_string()?;
                self.expect(b']', "']'")?;
                Ok(Predicate::AttrEquals(name, value))
            }
            Some(b'^') => self.parse_two_char_op(name, b'=', Predicate::AttrStartsWith),
            Some(b'$') => self.parse_two_char_op(name, b'=', Predicate::AttrEndsWith),
            Some(b'*') => self.parse_two_char_op(name, b'=', Predicate::AttrContains),
            _ => Err(self.syntax_error("expected attribute operator or ']'")),
        }
    }

    fn parse_two_char_op(
        &mut self,
        name: String,
        expected_second: u8,
        ctor: fn(String, String) -> Predicate,
    ) -> Result<Predicate, SelectorError> {
        self.advance(1);
        self.expect(expected_second, "'='")?;
        let value = self.read_quoted_string()?;
        self.expect(b']', "']'")?;
        Ok(ctor(name, value))
    }

    fn parse_pseudo(&mut self) -> Result<Predicate, SelectorError> {
        self.advance(1); // ':'
        if !self.peek().is_some_and(is_name_start) {
            return Err(self.syntax_error("expected pseudo-class name after ':'"));
        }
        let name = self.read_pseudo_name();
        match name.as_str() {
            "first-child" => Ok(Predicate::FirstChild),
            "nth-child" => {
                self.expect(b'(', "'(' after :nth-child")?;
                let n = self.read_unsigned_int()?;
                self.expect(b')', "')' after nth-child argument")?;
                Ok(Predicate::NthChild(n))
            }
            "not" => {
                self.expect(b'(', "'(' after :not")?;
                let inner = self.parse_simple()?;
                self.expect(b')', "')' after :not argument")?;
                Ok(Predicate::Not(Box::new(inner)))
            }
            other => Err(SelectorError::Syntax {
                reason: format!("unsupported pseudo-class :{other}"),
                at: self.pos,
            }),
        }
    }

    fn read_name(&mut self) -> String {
        let start = self.pos;
        while self.peek().is_some_and(is_name_char) {
            self.pos += 1;
        }
        self.src[start..self.pos].to_string()
    }

    fn read_pseudo_name(&mut self) -> String {
        // Pseudo names also allow `-` (e.g. `first-child`).
        let start = self.pos;
        while self.peek().is_some_and(|b| is_name_char(b) || b == b'-') {
            self.pos += 1;
        }
        self.src[start..self.pos].to_string()
    }

    fn read_quoted_string(&mut self) -> Result<String, SelectorError> {
        self.expect(b'"', "'\"'")?;
        let start = self.pos;
        while !self.at_end() && self.bytes[self.pos] != b'"' {
            self.pos += 1;
        }
        if self.at_end() {
            return Err(SelectorError::UnexpectedEnd);
        }
        let value = self.src[start..self.pos].to_string();
        self.pos += 1; // closing '"'
        Ok(value)
    }

    fn read_unsigned_int(&mut self) -> Result<u32, SelectorError> {
        let start = self.pos;
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.pos += 1;
        }
        if start == self.pos {
            return Err(self.syntax_error("expected digit"));
        }
        self.src[start..self.pos]
            .parse::<u32>()
            .map_err(|_| self.syntax_error("integer out of range"))
    }

    fn expect(&mut self, byte: u8, label: &str) -> Result<(), SelectorError> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(SelectorError::Syntax {
                reason: format!("expected {label}"),
                at: self.pos,
            })
        }
    }

    fn syntax_error(&self, reason: &str) -> SelectorError {
        SelectorError::Syntax {
            reason: reason.to_string(),
            at: self.pos,
        }
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(|b| b.is_ascii_whitespace()) {
            self.pos += 1;
        }
    }

    fn skip_ws_returning_seen(&mut self) -> bool {
        let start = self.pos;
        self.skip_ws();
        self.pos > start
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn at_end(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn advance(&mut self, n: usize) {
        self.pos += n;
    }
}

fn is_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.'
}
