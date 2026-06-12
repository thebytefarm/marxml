# 2026-06-04 audit

Multi-reviewer code audit of the marxml crate + Node binding against Rust 1.95 / Rust API Guidelines / napi-rs v3. The output is both a snapshot in time and a backlog for the deferred items below.

## What's in this folder

| File | What |
|---|---|
| [`SUMMARY.md`](./SUMMARY.md) | The consolidated finding list — every ERROR / WARN / INFO with a citation, severity, and suggested fix. Start here. |
| [`RUBRIC.md`](./RUBRIC.md) | The brief each reviewer was given before they started. Useful if you want to re-run the same audit shape against a different module or release. |
| [`reports/tokenizer.md`](./reports/tokenizer.md) | Per-reviewer report: tokenizer state machine. |
| [`reports/parser-selector.md`](./reports/parser-selector.md) | Per-reviewer report: parser + Document + selector subsystem. |
| [`reports/mutate-serialize.md`](./reports/mutate-serialize.md) | Per-reviewer report: mutate + serialize + escape. |
| [`reports/schema-validate.md`](./reports/schema-validate.md) | Per-reviewer report: schema + validate. |
| [`reports/api-surface.md`](./reports/api-surface.md) | Per-reviewer report: public API surface, error enum, types, Cargo. |
| [`reports/node-binding.md`](./reports/node-binding.md) | Per-reviewer report: napi-rs Node binding. |
| [`reports/codex-independent.md`](./reports/codex-independent.md) | Independent second opinion from Codex (GPT). |

## Methodology

- **6 parallel reviewers**, mixed-model: 3 Opus (tokenizer, parser+selectors, Node binding — most reasoning-heavy), 3 Sonnet (mutate+serialize, schema+validate, API surface). Each consumed `RUBRIC.md` before reading source.
- **1 Codex (GPT) independent run** via `codex exec --sandbox read-only` for cross-model second opinion.
- **Independent verification pass:** every ERROR-class finding was re-checked against the cited code at the cited line before being included in `SUMMARY.md`. One Codex finding (`parser.rs:297` integer-overflow context loss) was invalidated during verification and dropped — the parser actually emits the structured `SyntaxKind::IntegerOutOfRange` variant correctly.

## Status of findings

### Shipped (12 of ~32)

All five ERRORs landed via `ed50801` (E1, E3, E4, E5) + `773cb8a` (E2 partially — the IntoNapi trait covers the Node-side error mapping but the compile-once Selector class is still deferred). WARNs shipped: W2 (`IntoNapi` extension trait), W8 (regex-flags doc), W10 (typed `InvalidAttrKind`), W16 (drop dead `splice_regex` wrapper), W17 (`try_*` → `splice_*_report` rename), W18 (capacity hints on hot paths), W20 (`unreachable!` over `.expect()`), W22 (unterminated-value line bookkeeping), W23 (`record_seen_attr` `.collect()`), W25 (drop `once_cell` workspace dep).

### Deferred — Ask-First items

Each requires a design call before implementation per [AGENTS.md](../../../AGENTS.md). They're sorted roughly by impact.

#### Node binding surface

- **W4 + remainder of E2** — Compile-once `Selector` / `Schema` napi classes. The Rust crate's hot-path contract is "compile once, reuse many"; the Node binding currently re-parses on every call. Biggest single feature gap.
- **W1** — Async `parseAsync` / mutator variants. `MAX_INPUT_BYTES` ≈ 4 GiB; sync calls can stall the Node event loop on multi-MB inputs.
- **W3** — `toJson` should return `serde_json::Value` across the FFI instead of serializing to a string that the JS wrapper re-parses. Needs the napi `serde-json` feature.
- **W5** — Surface the `MutationReport.applied`/`.skipped` counts in `updateAttrs` (currently discarded). Mirrors the crate's `*_report` pattern.
- **W6** — Cache the `NativeMarkdown.elements` materialization. Doc is immutable so a `OnceCell` works.
- **W7** — Add `aarch64-unknown-linux-musl` napi target. Common gap (Alpine on Graviton). Changes `napi.targets` + release-workflow matrix.

#### Public Rust API

- **W11** — `ValidationReport::iter()` returns concrete `std::slice::Iter` (semver-leaky). Should be `impl Iterator<Item = &ValidationError>`.
- **W13** — `SelectorError::UnexpectedEnd` should carry `at: usize`. The parser knows the offset; the variant throws it away.
- **W14** — Selector parser accepts only `"`-quoted attribute values. CSS3 accepts both. One-line fix or doc the restriction.
- **W15** — Selector `\` escape silently parses corruptly inside attribute values. Reject with `SyntaxKind::EscapeNotSupported` or doc the absence.
- **W19** — `SerializeOpts::self_close_empty()` builder method shadows the public field of the same name. Rename to `with_self_close_empty` or similar.
- **W24** — `is_name_start` / `is_name_char` are `pub` in the private `escape` module but not re-exported. Pick one: re-export them or downgrade to `pub(crate)`.
- **W26** — `AttrConstraint` is a `pub` struct with `pub(crate)` fields and no accessors. Add `kind()` / `is_required()` getters.
- **W27** — `ElementRef<'a>` missing `PartialEq` / `Eq` derives. Decide pointer-identity vs value-equality semantics first.

#### Workspace

- **Edition 2024 + MSRV bump to 1.85.** Migration is non-breaking. Wins: RPIT lifetime capture would drop `+ 'a` noise from iterator return signatures; tighter `if let` temporary scope.

#### Skipped intentionally

- **W12** — `SchemaError::InvalidRegex` `reason: String` already became `#[source] regex::Error` in `c64fead`. Done before this audit started.
- **W21** — `ByteOffset(u32)` newtype refactor. Touches `Token`, `SourcePosition`, multiple modules — large lift relative to the win (a runtime `expect` becomes a compile-checked invariant). Defer until the type starts to bite.

## Adding a new audit

Convention:

```
docs/reviews/YYYY-MM-DD/
  README.md                  # index + roadmap for deferred items
  SUMMARY.md                 # the consolidated findings
  RUBRIC.md                  # the brief reviewers got
  reports/
    <module-or-area>.md      # one per reviewer slice
```

Re-using the rubric is fine — it's the durable contract. Update it whenever the lint policy, MSRV, or doctrine in [`contributing/rust-best-practices.md`](../../../contributing/rust-best-practices.md) changes.
