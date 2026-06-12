# Reviewer A — Tokenizer

Model: Opus. Scope: `crates/marxml/src/tokenizer.rs` (state machine), with `docs/ARCHITECTURE.md`, `types.rs`, `escape.rs`, `parse.rs` cross-checked.

## tokenizer.rs

### Clean — UTF-8 safety

Walked every site where a byte offset is fed back into `&str` slicing.

- `read_tag_name` (`tokenizer.rs:265-272`) slices `input[name_start..*i]`. Both ends are advanced only while `is_name_char(bytes[*i])` (`escape.rs:149-151` — `[a-zA-Z0-9._-]`), so both ends are ASCII bytes → always at codepoint boundaries.
- Attribute key slice `input[name_start..i]` (`tokenizer.rs:421`) — same reasoning; `is_name_start` (`escape.rs:141-143`) and `is_name_char` are ASCII-only.
- Attribute value slice `&input[value_start..i]` (`tokenizer.rs:459`) — value scan stops only on `b'"'`, `b'\n'`, or EOF (`tokenizer.rs:442-447`). All three are ASCII bytes; UTF-8 continuation bytes (`0x80..=0xBF`) and lead bytes (`>= 0xC0`) cannot collide. Boundaries are valid.
- `next_char_at` (`tokenizer.rs:537-539`) uses `input.get(i..)` which is `None` on a non-boundary `i`, then `chars().next()`. Defensively correct even if a caller drifted off boundary; today every caller (`tokenizer.rs:411`) is on a boundary.
- `bytes[i] == b'\n'` line counting (`tokenizer.rs:121, 443, 525, 543`) is correct on raw bytes because `0x0A` cannot appear inside a UTF-8 multi-byte sequence.

### Clean — same-tag nesting

Walked `<task><task>inner</task></task>` (12 bytes of tags + 5 of "inner"):

1. `i=0`, `<` then `t` (name-start) → `parse_opening_tag` emits `Open{name:"task", body_start:6, span:[0,6)}`. `i=6`.
2. `i=6`, `<` then `t` → `Open{name:"task", body_start:12, span:[6,12)}`. `i=12`.
3. `i=12..17`, ASCII text "inner", skipped via the fall-through at `tokenizer.rs:121-124`.
4. `i=17`, `<` then `/` then `t` → `looks_like_tag_start` true → `parse_tag` dispatches to `parse_end_tag` (line 158) → `Close{name:"task", body_end:17, span:[17,24)}`. `i=24`.
5. `i=24`, `<` then `/` then `t` → `Close{name:"task", body_end:24, span:[24,31)}`. `i=31`. Done.

Same-tag nesting is the parser's responsibility (`parse.rs:98-187`); the tokenizer's flat token stream is correct. Name matching happens at `parse.rs:160-167`.

### Clean — EOF in every state

Cross-checked each state's EOF guard:

- Main loop: `while i < bytes.len()` (`tokenizer.rs:93`).
- `looks_like_tag_start`: explicit `i >= bytes.len()` and `i + 1 < bytes.len()` (`tokenizer.rs:133-141`).
- `try_skip_comment` / `try_skip_cdata`: `bytes[start..].starts_with(...)` returns false on short tails (`tokenizer.rs:478, 497`); `scan_to_terminator` short-circuits with `i + term_len <= bytes.len()` and returns `None` → mapped to `Unterminated{Comment,Cdata}` (`tokenizer.rs:481-486, 500-505, 521-530`).
- `parse_end_tag`: bounds-checked at `tokenizer.rs:181`.
- `parse_opening_tag` / `parse_attribute_list`: each loop iteration starts with `skip_ws` then `i >= bytes.len()` → `UnterminatedOpenTag` (`tokenizer.rs:297-303`).
- `parse_attribute`: each phase (name, `=`, opening `"`, value scan, closing `"`) checks `i >= bytes.len()` first (`tokenizer.rs:407, 423, 432, 448`).
- `scan_to_terminator` cannot integer-overflow on `i + term_len`: `parse.rs` enforces `input.len() <= u32::MAX` before the tokenizer runs (referenced at `tokenizer.rs:34-38, 79-86`), so on any `>= 32-bit` `usize` the addition stays in range.

No reachable panics inside the state machine for any input that parse::parse will accept.

### Clean — byte-offset bookkeeping

Spans are half-open `[start, end)` (`types.rs:31-38`) and consistently produced:

- `Open.span.end.offset = list.end` which is `i + 1` after the terminating `>` (`tokenizer.rs:310, 327`) — points one past `>`. ✓
- `Open.body_start = i` set to the same value after `i = list.end` (`tokenizer.rs:236, 250`). ✓ Inclusive start-of-body.
- `Close.body_end = start` where `start` is the `<` byte (`tokenizer.rs:199`). ✓ Exclusive end-of-body; matches `body_start..body_end` half-open contract used by the parser at `parse.rs:148-187`.
- `Close.span.end.offset = i` (just past `>`, `tokenizer.rs:188-191`). ✓
- `SelfClose.span.end.offset = i + 1` after `/>` (`tokenizer.rs:324-329, 242`). ✓
- CDATA trivia ranges:
  - `open_end = i + 9` covers `<![CDATA[`. ✓
  - `close_start = new_i - 3` covers `]]>`. `new_i` is always `>= i + 12` (the `starts_with(b"<![CDATA[")` precondition ensures 9 bytes, plus `]]>` is 3), so `new_i - 3 >= 9` and the subtraction cannot underflow.
  - Range pair is non-overlapping and abuts in the degenerate `<![CDATA[]]>` case (both ranges valid and accepted by the `TextSegments` filter at `types.rs:226-237`).

### WARN — `body_start` / `body_end` typed as raw `usize` while every neighbouring offset is `u32`

- Location: `tokenizer.rs:46-54`
- What: `Token::Open.body_start: usize` and `Token::Close.body_end: usize` are bare integers, while `SourcePosition.offset` (`types.rs:13-18`) is `u32` and `ElementData.content_range: Range<usize>` (`types.rs:46-60`) is also `usize`. Two byte-offset representations co-exist inside the same struct hierarchy and the parser converts between them implicitly.
- Why it matters: A reader (and any future contributor) has to remember which offsets are bounded `u32` and which are `usize` widenings, and the `offset_u32` narrowing helper (`tokenizer.rs:36-38`) carries an `expect` that will fire if the contract is ever broken. A small `ByteOffset(u32)` (or `ByteOffset(usize)`) newtype shared with `SourcePosition` would make the invariant compile-checked rather than `expect`-checked, and would remove the `usize`-to-`u32` shuttling at `tokenizer.rs:156, 190, 242`. Maps to API guideline C-NEWTYPE.
- Suggested fix: introduce `pub(crate) struct ByteOffset(u32)` with `into_usize(self) -> usize` and `try_from_usize(usize) -> Result<Self, ...>`. Use it for `body_start`, `body_end`, `content_range`, and as the inner type of `SourcePosition.offset`. Keep the public API unchanged.

### WARN — `parse_attribute_list` overstates `tag_start_line` for value-spanning attributes

- Location: `tokenizer.rs:289-346` (line carried in `start_line` argument), interaction with `parse_attribute` `tokenizer.rs:443-453`
- What: `parse_attribute_list` is called once with `start_line` set to the line of the tag's `<`. As `skip_ws` and attribute-value scans consume `\n` bytes, the local `line` variable advances correctly. But when an attribute value is unterminated at EOF, `parse_attribute` reports `line: start_line` (`tokenizer.rs:453`) — the line where the **attribute name** started, not the line where the unterminated `"` opened, and not the line where EOF was reached. For a 200-line attribute value that runs off the end, the error points at the attribute name far above.
- Why it matters: Diagnostic precision degrades for the exact pathological case where the user needs line numbers most. Not a correctness bug — the tokenizer still rejects the input.
- Suggested fix: thread a separate `value_open_line` captured at `tokenizer.rs:439` and use it (or `line` at the EOF site) for `UnterminatedValue`. The other `Malformed*` variants already use the right `line`.

### WARN — `record_seen_attr` rebuilds set with `.clone()` for every existing key

- Location: `tokenizer.rs:364-382`
- What: When the hash-set path is promoted (15+ existing attrs), `record_seen_attr` allocates a fresh `HashSet<String>` and `set.insert(k.clone())` for every existing key (`tokenizer.rs:377-379`). The promotion happens exactly once per tag, so this is O(n) clones on a single tag, but every existing key string was already owned in `attrs` and is re-cloned into the set for the duration of one tag's lifetime — doubling the memory for attribute names on tags with many attributes.
- Why it matters: TS-brain `.clone()` reflex; the same lookup behaviour can be achieved with `HashSet<&str>` keyed off `attrs` (borrowing). The threshold guarantees this only fires on pathological tags, so the impact is small, but the pattern is worth flagging because `record_seen_attr` is the file's most owned-`String`-heavy function.
- Suggested fix: use `HashSet<&str>` borrowing from `attrs`. The set's lifetime is bounded by `parse_attribute_list`'s stack frame, and `attrs.push(...)` happens *after* `record_seen_attr` returns, so the borrow is consistent for the duration of the membership check on the next iteration. (Alternative: keep `HashSet<String>` but build it once via `attrs.iter().map(|(k,_)| k.clone()).collect()` to drop one branch.)

### WARN — `tokenize` returns `Vec<Token>` with no capacity hint

- Location: `tokenizer.rs:88-89`
- What: `let mut tokens = Vec::new()` and `let mut trivia: Vec<...> = Vec::new()` start at capacity zero. Both grow via reallocation as the parser walks. Tag-dense input triggers `O(log n)` reallocations.
- Why it matters: Minor; criterion would tell whether it matters for the standard fixtures. Worth raising because the `O(input length / avg-tag-bytes)` upper bound is cheap to estimate.
- Suggested fix: `Vec::with_capacity(input.len() / 32)` (or similar). Benchmark-gated.

### INFO — Rust 2024 edition / RPIT capture

- Location: `Cargo.toml` (not read in this pass; flagged from the reference brief). The tokenizer file itself contains no public-API-visible iterators, so the edition-2024 RPIT capture changes don't affect it directly.
- Suggested fix: none — this is informational. When the workspace eventually moves from 2021 → 2024 (Rust 1.85+), no tokenizer changes should be required.

### INFO — `try_skip_comment` / `try_skip_cdata` could share a small helper

- Location: `tokenizer.rs:473-506`
- What: The two functions are structurally identical (`starts_with` → `scan_to_terminator` → `ok_or_else`). The only differences are the prefix bytes, the prefix length, the terminator bytes, and the error variant. A `fn try_skip_bracketed(bytes, start, line, prefix, terminator, missing_error) -> Result<Option<(usize, u32)>, ParseError>` would dedupe.
- Why it matters: Cosmetic. The current pair is readable and the duplication is contained. Mention but don't push.
- Suggested fix: defer until either a third bracketed trivia form lands (PI, doctype) or the two functions start drifting in behaviour.

### INFO — No regex temptation observed

- Location: entire file
- What: Walked every branch; the scanner does its own state work with byte indexing throughout. Attribute-value scanning stops on `"` only — no escape-quote handling that might tempt a regex-shaped refactor. Comment/CDATA scanning uses an explicit byte-wise `scan_to_terminator`. Numeric/entity decoding is delegated to `decode_entities` (`escape.rs:181`), which is also regex-free.
- Why it matters: Confirms compliance with the `CLAUDE.md` `Never` rule "no new regex for tokenization".
- Suggested fix: none.

## Checked

- `crates/marxml/src/tokenizer.rs:1-549` (full file, every state transition walked manually with a sample `<task><task>inner</task></task>` and edge inputs `<!doctype html>`, `<!--->`, `<!-->`, `<![CDATA[]]>`, `<![CDATA[]]`, lone `<` at EOF, `</` at EOF).
- `docs/ARCHITECTURE.md:1-147` (Tokenizer + Parser + Mutation sections cross-referenced against implementation).
- `crates/marxml/src/types.rs:1-272` (`SourcePosition`, `SourceSpan`, `ElementData`, `ElementRef`, `TextSegments` — to validate the byte-offset contract that the tokenizer emits into).
- `crates/marxml/src/escape.rs:135-258` (`is_name_start`, `is_name_char`, `is_valid_name`, `decode_entities`, `decode_one_entity`, `is_valid_xml_char` — to validate the ASCII-only name predicate that underpins UTF-8 safety, and the entity decoder called from `parse_attribute`).
- `crates/marxml/src/parse.rs:98-202` (skimmed; confirms the parser is the layer that handles same-tag nesting via stack + name match, so tokenizer's flat token stream is the right abstraction boundary).

**Conclusion:** 0 ERROR, 4 WARN, 3 INFO. Tokenizer is sound on UTF-8 (ASCII-only name predicate forces all string-slice boundaries onto codepoint boundaries) and same-tag nesting is correctly delegated to the parser.
