# Selector grammar (formal)

EBNF for the selector grammar. For day-to-day usage, [selectors.md](./selectors.md) is the friendlier reference; this page is the formal spec for tooling authors (syntax highlighters, alternative parsers).

The validation schema has no formal grammar — it's a typed object shape, not a parsed language. Its spec is the TS interface ([`bindings/node/marxml.d.ts`](../../bindings/node/marxml.d.ts)) and the Rust builder types ([`crates/marxml/src/schema.rs`](../../crates/marxml/src/schema.rs)).

## EBNF

Whitespace handling: `WS+` between two simples is the descendant combinator; `WS*` is allowed (and ignored) around `>`, around `,`, immediately inside `[ ]`, and immediately inside `:not(...)`.

```ebnf
selector       = compound, { WS*, ",", WS*, compound } ;
compound       = simple, { combinator, simple } ;
combinator     = WS+                                          (* descendant *)
               | WS*, ">", WS* ;                              (* direct child *)
simple         = ( tag | "*" ), { predicate }
               | predicate, { predicate } ;                   (* tag-less; implicit "*" *)
predicate      = "[", name, [ op, quoted ], "]"
               | ":", pseudo ;
op             = "="
               | "^="
               | "$="
               | "*=" ;
pseudo         = "first-child"
               | "nth-child(", digits, ")"
               | "not(", simple, ")" ;
tag            = name ;
name           = name-start, { name-char } ;
name-start     = letter | "_" ;
name-char      = letter | digit | "_" | "-" | "." ;
letter         = "A".."Z" | "a".."z" ;
digit          = "0".."9" ;
quoted         = '"', { quoted-char }, '"' ;
quoted-char    = (* any byte except '"'; entity refs decoded *) ;
digits         = digit, { digit } ;                            (* `n` for :nth-child(n), >= 1 *)
WS             = " " | "\t" | "\r" | "\n" ;
```

## Hard limits

Inputs that exceed any of these are rejected at parse time.

| Limit                                                 | Max |
| ----------------------------------------------------- | --- |
| Comma-separated compounds in one selector             | 64  |
| Space-separated simples in one compound chain         | 64  |
| Predicates (`[…]` / `:…`) on one simple selector      | 32  |
| Nesting depth of `:not(...)`                          | 64  |
| Digits in a `:nth-child(n)` argument                  | 11  |

## Deliberate departures from CSS3

| CSS3 convention                           | marxml behavior                                                                                                          |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Attribute values can be unquoted          | Required to be quoted (`[id="t1"]`, never `[id=t1]`). Simpler to parse, matches XML attribute syntax.                    |
| Sibling combinators `+` and `~`           | Not supported.                                                                                                            |
| `:has()`, `:contains()`, `:nth-of-type()` | Not supported (post-0.1 work).                                                                                            |
| Case-insensitive attr flag `[a="b" i]`    | Not supported; use a `Regex` schema constraint with `(?i)…` if you need it.                                                |
| Pseudo-elements (`::before` etc.)         | Not supported; meaningless for parsed markdown+XML.                                                                       |
| Entity references in quoted values        | Decoded (`&amp;` → `&`) so selector strings match what the tokenizer stored.                                              |

## See also

- [DSL · Selectors](./selectors.md) — day-to-day reference.
- [DSL · Schema](./schema.md) — schema shape (defined by TS interface, not a grammar).
- [Selector parser source](../../crates/marxml/src/selector/parser.rs) — authoritative implementation.
