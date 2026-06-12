# Reviewer B — Parser + Document + Selector

Model: Opus. Scope: `crates/marxml/src/parse.rs`, `document.rs`, `selector/{mod,ast,error,parser,matcher}.rs`, and `docs/dsl/selectors.md` as the grammar source-of-truth.

## parse.rs (stack-based tree assembler)

### Clean — stack assembly, byte spans, ID dedup

- Stack underflow on `Close` is handled deterministically via `let Some(frame) = stack.pop() else { return Err(StrayClose) }` (parse.rs:160). No `unwrap`/`expect` in the hot path.
- Mismatched close tag returns `MismatchedClose { found, expected, line }` with the line carried verbatim from the tokenizer's span (parse.rs:166-172).
- Unclosed tag at EOF caught by the post-loop `if let Some(unclosed) = stack.pop()` (parse.rs:192-197).
- Byte spans flow from tokenizer to `ElementData` unmodified: `Token::SelfClose.span` becomes the elem span directly (parse.rs:147); `Token::Close.body_end` joins `frame.body_start` to form `content_range` (parse.rs:182); `frame.span_start` + `span.end` becomes the full element span (parse.rs:175-178). No arithmetic — pure assembly.
- `MAX_DEPTH` is enforced on both `Open` (parse.rs:110) *and* `SelfClose` (parse.rs:132), so a self-closing tag at the limit still receives the same depth bound the matcher will later assume. Good defensive symmetry.
- Sibling-scoped ID dedup correctly resets per parent via the lazily-allocated `Frame::seen_ids` (parse.rs:93) and the `current_scope` helper (parse.rs:212-221). The doc comment example matches behavior.

### WARN — `parse()` does an avoidable `String::to_string()` clone

- Location: `parse.rs:47-49`
- What: `parse(input: &str)` calls `input.to_string()` and forwards to `parse_owned`. `parse_owned` then borrows the `String` to tokenize and finally moves it into `Markdown::from_parts`. The clone is one alloc + one memcpy of the whole document.
- Why it matters: Rust idiom for `parse: &str -> T` is to do the allocation only when needed; here the allocation is mandatory (the Markdown stores the owned source), so the clone is unavoidable in this design. The WARN is that the *flag* `parse` vs `parse_owned` reads as a perf knob, but the only saving is the second alloc when a caller already had a `String`. Fine as is — flagging because the comment on `parse_owned` ("avoid the allocation that `parse` would do internally") understates that `parse` does a single allocation, not multiple.
- Suggested fix: tighten the doc comment to "lets the caller skip the `to_string()` `parse` performs to obtain an owned source". No code change.

### INFO — `check_duplicate_id` clones `tag` + `id` on every id-bearing element

- Location: `parse.rs:236-247`
- What: `scope.entry(tag.to_string()).or_default().insert(id.to_string())` allocates two `String`s per id-bearing element even on the success path.
- Why it matters: Hot path on docs with many tasks. The id is from a `Vec<(String, String)>` already-owned; cloning is the price of the HashMap.
- Suggested fix: Not worth restructuring unless a benchmark shows it. Mention only.

## document.rs (typed tree + `Document::select`)

### Clean — public surface, compile-once contract

- `Markdown` derives `Debug, Clone, PartialEq, Eq` (document.rs:16). Matches C-COMMON-TRAITS.
- `impl FromStr for Markdown` (document.rs:23) and `impl TryFrom<String> for Markdown` (document.rs:33) cover the standard conversion vocab without ad-hoc `new_from_X` constructors.
- `Document::select(&self, sel: &Selector)` (document.rs:85) takes a compiled selector — no string overload, no per-call re-parse. The doc example (document.rs:78-84) demonstrates the compile-once flow.
- All five mutator entry points (`update`, `replace_content`, `replace_in`, `replace_text`, `replace_text_in`) and the three `*_report` variants take `&Selector` — no escape hatch that re-parses per call.
- All five accept `&Regex`, not `&str`, so the regex is also compile-once on the caller's side.
- `#[must_use]` on every non-`try_` mutator (document.rs:110, 117, 127, 135, 142, 167, 175, 189, 206) — these are pure returns and silently ignoring them would be a bug.

### INFO — `select` collects eagerly then returns `into_iter()`

- Location: `document.rs:85-87`, backed by `selector/matcher.rs:34-60` returning `Vec<ElementRef<'a>>`.
- What: `Document::select` advertises `impl Iterator<Item = ElementRef<'_>>` but the underlying matcher walks the entire tree before any element is yielded. A caller doing `doc.select(&sel).next()` still pays the full walk cost.
- Why it matters: Doc users may write `.take(N)` expecting lazy early-termination. They'd get the right answer, but not the perf shape they expect.
- Suggested fix: Either (a) doc this on `select` ("walks the whole tree eagerly; returned iterator is over a pre-collected Vec"), or (b) restructure the matcher into a `Matches<'a>` iterator carrying its own ancestor stack. (b) is non-trivial because the depth-first walk currently uses the call stack. Defer until a real workload demands lazy.

## selector/mod.rs

### Clean — minimal public re-exports, compile-once enforced

- Only `Selector`, `SelectorError`, `SyntaxKind` are public (mod.rs:10). `Compound`, `Simple`, `Predicate`, `Combinator` stay `pub(super)` in `ast.rs` — the internal grammar is not part of the public contract, which matches a "compiled handle" pattern.
- `Selector::parse(&str)` is the only constructor (mod.rs:44). The `FromStr` impl (mod.rs:51-58) delegates to `parse` — no second parse path.
- `Selector` derives `Debug, Clone, PartialEq, Eq` (mod.rs:32). Fits C-COMMON-TRAITS. `Send`/`Sync` are auto-implemented (all fields are owned `String`/`Vec`/`Box`).

## selector/ast.rs

### Clean — typed compound representation

- `Compound { subject, prefix: Vec<(Combinator, Simple)> }` (ast.rs:15-22) stores the subject + a *reversed* prefix walking right-to-left. The matcher consumes this exactly as documented.
- All variants derive `Debug, Clone, PartialEq, Eq` (small enums also `Copy`). Internal types — `pub(super)` keeps them out of the public API.

### INFO — `Box<Simple>` inside `Predicate::Not`

- Location: `ast.rs:51`
- What: `Predicate::Not(Box<Simple>)` boxes a `Simple` (≈ 64 bytes — String + Vec). Box is required to break the recursion (`Predicate` lives inside `Simple`); cannot be replaced with generics.
- Why it matters: Not actionable. Box is the right call here. Mentioned to head off a "could this be `impl Trait`?" review note.

## selector/error.rs

### Clean — `thiserror`, `non_exhaustive`, structured

- `#[non_exhaustive]` on both `SelectorError` (error.rs:7) and `SyntaxKind` (error.rs:36). Future variants are additive without breaking downstream `match` exhaustiveness.
- `SyntaxKind::Expected { what: &'static str }` (error.rs:81-86) — `&'static str` not `String`, so the variant is `Copy`-adjacent. The comment correctly notes `what` is crate-controlled, not user input.
- `at` is a `usize` byte offset (error.rs:24). Fine — no need for a newtype here; the public docs name it explicitly.

### WARN — `SelectorError::UnexpectedEnd` carries no position

- Location: `error.rs:14-15`
- What: `UnexpectedEnd` has no `at: usize`. Selectors like `[id="foo` (unterminated string) hit `UnexpectedEnd` (parser.rs:277) with no byte offset, even though the parser knows exactly where it was when input ran out.
- Why it matters: A friendly downstream error message wants "selector ended unexpectedly at offset 8". Today, the consumer has to either recompute or just say "somewhere".
- Suggested fix: `UnexpectedEnd { at: usize }`. Since the enum is `#[non_exhaustive]`, adding a field is still a breaking change for callers that construct or pattern-match the variant — would need a minor bump and a changeset.

### INFO — `SelectorError::Empty` and `UnexpectedEnd` could fold into `Syntax`

- Location: `error.rs:8-25`
- What: Three top-level variants where one structured `Syntax { kind, at }` could cover all of them (`SyntaxKind::Empty`, `SyntaxKind::UnexpectedEnd`). The split is a stylistic choice, not a bug.
- Suggested fix: None unless a major-version pass revisits the error shape.

## selector/parser.rs

### Self-corrected ERROR — descendant detection across newlines

(Re-reading: `skip_ws_returning_seen` calls `skip_ws` which matches `b.is_ascii_whitespace()`, covering `\t`, `\n`, `\r`, `\x0C`. Selectors split across lines do parse as descendant. No finding.)

### WARN — `read_quoted_string` only accepts `"`, not `'`

- Location: `parser.rs:270-286`
- What: The first `self.expect(b'"', "'\"'")?` rejects single-quoted attribute values. Selector `[id='t1']` errors at offset 4 with `expected '\"'`. The grammar doc (docs/dsl/selectors.md:64) explicitly says quoted values are required, but it does not call out *which* quote.
- Why it matters: CSS3 accepts `[id='t1']` and `[id="t1"]`. Users coming from `querySelectorAll` will hit this on documents copied verbatim from web tutorials. Today the error is "expected '\"'" — clear but pedantic.
- Suggested fix: Either (a) accept both quote styles and continue reading until the matching close (one-line change), or (b) update the grammar table in `docs/dsl/selectors.md` to spell out "double-quoted only". (a) is the user-friendly call and the matcher doesn't care which quote produced the value.

### WARN — no `\"` escape inside attribute values; `&quot;` is the only escape

- Location: `parser.rs:270-286`
- What: The string-body loop terminates on the *first* `"` byte. There is no `\"` / `\\` escape sequence. The escape mechanism is XML entity references via `decode_entities(...)` (parser.rs:283), so `[id="a&quot;b"]` is how a quote inside a value is expressed.
- Why it matters: This is consistent with XML attribute literal semantics (you escape `"` as `&quot;`), and the grammar doc covers entity decoding (docs/dsl/selectors.md:64). But a CSS-trained user expects `\"`; failing silently — `[id="a\"b"]` doesn't error, it parses as `AttrEquals(id, "a\\")` then leaves `b"]` to error on the next compound — is the worst kind of footgun.
- Suggested fix: When the byte at `self.pos` is `\\`, reject with a dedicated `SyntaxKind::EscapeNotSupported` so the user gets "use `&quot;` instead of `\\\"`" rather than a downstream cascade. Or, document the absence in the grammar with an explicit "no `\\` escape" line.

### WARN — `read_quoted_string` always allocates via `.into_owned()`

- Location: `parser.rs:283`
- What: `decode_entities(&self.src[start..self.pos]).into_owned()` forces a `String` even when `decode_entities` returns `Cow::Borrowed`. The `Predicate` stores `String`, so an allocation is required *eventually* — but for the borrowed case this is `value.to_string()` masquerading as a Cow conversion.
- Why it matters: Selectors with many literal values pay one alloc per value. Probably fine; selectors are short-lived inputs. Mentioned for completeness.
- Suggested fix: No-op unless a benchmark shows it.

### INFO — `use` statement at end of file

- Location: `parser.rs:362`
- What: `use crate::escape::{is_name_char, is_name_start};` sits at the bottom of the file, below all impls.
- Why it matters: `cargo fmt` accepts it but Rust idiom is imports grouped at the top of the file. A reader scanning the imports will miss this dependency.
- Suggested fix: Move to the existing `use` block at the top (parser.rs:19-21).

### INFO — `parse_compound` does extra work assembling `prefix` from parallel `Vec`s

- Location: `parser.rs:96-131`
- What: Builds `simples: Vec<Simple>` and `links: Vec<Combinator>` in lockstep, then pops both and reassembles a reversed `prefix: Vec<(Combinator, Simple)>`. The invariant `simples.len() == links.len() + 1` is enforced by construction but only commented (parser.rs:96).
- Why it matters: Three allocations (simples, links, prefix) where one would do — push `(Combinator, Simple)` pairs directly. The current shape is fine; the `subject` extraction is what motivated the split. Not a bug.
- Suggested fix: Could refactor to build `prefix` forward and reverse at the end. Worth doing only if `parse` ever becomes hot, which it shouldn't because of compile-once.

## selector/matcher.rs

### Clean — descendant/child semantics, dedup

- `compound_matches` (matcher.rs:106-142): the subject is checked first against the current node (matcher.rs:107), then `depth = ancestors.len()` walks the ancestor stack right-to-left. `Child` decrements by 1 and checks the parent; `Descendant` scans toward the root. Semantics match CSS3.
- Dedup uses `std::ptr::from_ref(node) as usize` (matcher.rs:77) — element identity is stable for the lifetime of the `Markdown` owning the `ElementData`. Only allocated when the selector is a real union (>1 compound) — single compounds skip the HashSet entirely (matcher.rs:41-45). Good optimization.
- `walk` (matcher.rs:62-104) carries `ancestors: &mut Vec<NodeCtx<'a>>` push/pop'd around the recursion — the stack is reused across siblings, so allocation is bounded by the document depth, not by tree size.
- `sibling_index(usize) -> u32` (matcher.rs:22-25) is justified by the `MAX_INPUT_BYTES < u32::MAX` document bound. The `expect` here is sound — children count cannot exceed input bytes — but documented inline.
- Attribute predicates (matcher.rs:153-172) use the appropriate string ops (`starts_with`, `ends_with`, `contains`). No regex on this path — regex would over-shoot the documented `^=` / `$=` / `*=` semantics.

### INFO — `Predicate::NthChild(n).saturating_add(1) == *n`

- Location: `matcher.rs:169`
- What: `node.index.saturating_add(1) == *n`. If `node.index == u32::MAX` and `n == u32::MAX`, saturating returns `u32::MAX` and the predicate spuriously matches.
- Why it matters: `node.index` is constrained by `sibling_index` (≤ `u32::MAX` and in practice ≤ input bytes), so an actual `index == u32::MAX` is unreachable. The saturating add is defensive against the parser's `u32` choice meeting an arithmetic overflow, but the false-match window is purely theoretical.
- Suggested fix: None — saturating is the conservative choice. If we ever want strict semantics, `node.index.checked_add(1) == Some(*n)` is the precise form.

### INFO — `walk` is recursive; passes documents up to `MAX_DEPTH = 1024` levels deep

- Location: `matcher.rs:62-104`
- What: The walker uses real recursion. With `MAX_DEPTH = 1024` (parse.rs:25) and each stack frame holding a Vec ref + a small struct, this is well within the default 8 MiB thread stack on every supported platform.
- Why it matters: A future MSRV bump or LTO change could shift frame size. Not actionable today.
- Suggested fix: None. The choice to do `&mut ancestors` plus recursion (matcher.rs:67) is the right Rust idiom for tree walks of bounded depth.

## docs/dsl/selectors.md vs implementation

### WARN — grammar doc lists `:nth-child(0)` rejection but glosses the digit cap

- Location: `docs/dsl/selectors.md:96` ("Digits in a `:nth-child(n)` argument | 11")
- What: The "Limits" table caps digits at 11 but doesn't say what happens at 12 (`IntegerOutOfRange`). The error variant docstring on `error.rs:73` is the only place the cap is justified ("u32::MAX is 10 digits; 11 distinguishes leading zeros from out-of-range").
- Why it matters: A consumer hitting the cap will see `integer out of range` and have to read the source to understand why a 12-digit number errored. Not a bug — flagging because docs are the bridge between rule and reason.
- Suggested fix: Add a one-liner under "Limits": "Selectors with more than 11 digits are rejected as `IntegerOutOfRange`; the cap is just above `u32::MAX`'s 10-digit width."

### INFO — `:not(:not(...))` rejection is enforced by the depth cap, not a dedicated error

- Location: `docs/dsl/selectors.md:84` ("`:not(:not(...))` is rejected"), enforced in `parser.rs:227-243`
- What: The cap is `MAX_NOT_DEPTH = 64` (parser.rs:55), not a flat rejection of any `:not(:not(...))`. So `:not(:not(task))` actually *parses* (depth 2 ≤ 64) and matches "anything matching task". The doc is wrong here — or more precisely, it's describing intended behavior (single-simple inner) that the parser doesn't enforce: `parse_simple` does accept a `:not()` predicate inside another `:not()` because `parse_pseudo` recurses.
- Why it matters: Either the doc is misleading or the code under-enforces. The grammar quote in `parser.rs:5-16` shows `pseudo := ... | "not(" simple ")"` and `simple := (* | name) predicate*` and `predicate := ... | ":" pseudo` — so yes, nested `:not` parses to the `MAX_NOT_DEPTH = 64` cap.
- Suggested fix: Either (a) the docs are correct in intent → reject nested `:not` at parser level with a dedicated `SyntaxKind::NotInsideNot`, or (b) the code is correct → soften the docs to "deeply nested `:not()` is capped at 64 levels". The repo rule "Schema DSL changes: ask first" suggests raising this rather than picking one.

### Clean — every grammar feature listed has a corresponding parser branch + matcher arm

Verified the docs' TL;DR table (selectors.md:7-26) against parser predicates: `[attr]` (`HasAttr`), `[attr="v"]` (`AttrEquals`), `^=` (`AttrStartsWith`), `$=` (`AttrEndsWith`), `*=` (`AttrContains`), `>` (`Combinator::Child`), descendant whitespace (`Combinator::Descendant`), `,` (compound union), `:first-child`, `:nth-child(n)`, `:not(simple)`, `*` (universal tag), tag-less simples. Every one round-trips parser → matcher.

"Not supported" table (selectors.md:117-129) — sibling combinators, `:has`, `:contains`, case-insensitive flags, `:nth-of-type`/`:nth-last-child`, pseudo-elements — none appear in the parser or matcher. Matches the doc.

## Cross-cutting

### WARN — `#[allow(clippy::too_many_arguments)]` on `walk` lacks justification comment

- Location: `selector/matcher.rs:62`
- What: `#[allow(clippy::too_many_arguments)]` with no comment. The repo rule (CLAUDE.md) forbids "clippy warnings papered over with `#[allow(...)]` without an explanatory comment."
- Why it matters: CI runs `clippy -D warnings`; the suppression is required to compile but undocumented. Future maintainers won't know whether it's load-bearing.
- Suggested fix: Add a one-line comment above the attribute explaining the trade.

### INFO — Rust 2024 edition migration

- Edition is `2021` (Cargo.toml). 2024 edition deltas relevant to this subsystem:
  - RPIT lifetime capture (`Markdown::select`, `ElementRef::select`, `attrs`, `children`, `text`) — fewer explicit lifetime bounds needed. None of the current bounds would actually go away without rework, so the migration is cosmetic here.
  - `if let` temporary scope tightening — affects `if let Some(top) = stack.last_mut() { ... }` patterns in `parse.rs` and `current_scope`. No correctness impact at current code shape, but worth a `cargo fix --edition` pass when MSRV gets to 1.85.

## Checked

- `crates/marxml/src/parse.rs` — lines 1-249 (full file).
- `crates/marxml/src/document.rs` — lines 1-220 (full file).
- `crates/marxml/src/selector/mod.rs` — lines 1-68 (full file).
- `crates/marxml/src/selector/ast.rs` — lines 1-52 (full file).
- `crates/marxml/src/selector/error.rs` — lines 1-87 (full file).
- `crates/marxml/src/selector/parser.rs` — lines 1-362 (full file).
- `crates/marxml/src/selector/matcher.rs` — lines 1-179 (full file).
- `docs/dsl/selectors.md` — lines 1-147 (full file).
- `crates/marxml/src/types.rs` — lines 1-271 (full file, for `ElementData`/`ElementRef` cross-references).
- `crates/marxml/src/escape.rs` — lines 1-307 (full file, for `decode_entities` / `is_name_*` cross-references).
- `crates/marxml/src/tokenizer.rs` — lines 35-119 (Token enum + entry point; verified `Token`/`TokenStream` shape and `decode_entities` call on attribute values).

**Conclusion:** Parser and matcher structurally sound (no stack underflow, deterministic error paths, byte spans flow through unmodified, compile-once selector contract enforced in the public API). Actionable findings concentrate in the selector parser's grammar surface area.
