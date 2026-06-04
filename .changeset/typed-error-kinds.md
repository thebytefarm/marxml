---
default: minor
---

#### Refactor: replace stringly-typed `reason: String` error fields with typed sub-enums

The public error types in `marxml` previously buried the actual failure mode inside a `reason: String` interpolated at the call site. Three error variants used this pattern, which forced consumers to string-match on diagnostic prose to discriminate failure modes:

- `ParseError::MalformedTag { reason: String, line: u32 }`
- `ParseError::MalformedAttribute { tag: String, reason: String, line: u32 }`
- `SelectorError::Syntax { reason: String, at: usize }`

Plus one foreign-error wrap that stringified the underlying cause instead of preserving the source chain:

- `SchemaError::InvalidRegex { tag: String, attr: String, reason: String }`

This release replaces `reason: String` with typed sub-enums that callers can `match` on:

- `MalformedTag.kind: MalformedTagKind` — `ExpectedCloseAngle`, `UnterminatedOpenTag`, `ExpectedCloseSlashAngle`, `UnterminatedComment`, `UnterminatedCdata`.
- `MalformedAttribute.kind: MalformedAttrKind` — `UnexpectedNameStart{found:char}`, `ExpectedEquals{attr}`, `ExpectedOpenQuote{attr}`, `UnterminatedValue{attr}`.
- `SelectorError::Syntax.kind: SyntaxKind` — `UnionTooLarge{max}`, `CompoundTooLong{max}`, `TooManyPredicates{max}`, `NotNestingTooDeep{max}`, `UnsupportedPseudoClass{name}`, `NthChildMustBeOneOrGreater`, `IntegerOutOfRange`, `ExpectedDigit`, plus a catch-all `Expected{what: &'static str}` for the structural "parser expected token X" cases.

`SchemaError::InvalidRegex` now carries `#[source] source: regex::Error` instead of `reason: String`, so `Error::source()` walks the cause chain. `SchemaError` drops its `Eq` derive in the process (`regex::Error` is `PartialEq` but not `Eq`); `PartialEq` is retained.

`Display` output is unchanged across every variant — the new `#[error("…")]` strings reproduce the original prose verbatim, so log lines, snapshot tests, and human-facing error messages are identical.

Breaking change for any consumer that constructed these errors directly or matched on the `reason` field. Consumers that only pattern-matched on the variant tag (`matches!(e, ParseError::MalformedTag { .. })`) or read `Display` output are unaffected.
