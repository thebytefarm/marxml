# Selectors

CSS-subset selectors for finding elements in a parsed `marxml` document. Identical grammar in Rust (`Selector::parse(s)`) and Node (`doc.select(s)`).

## TL;DR

```
task                           every <task>
*                              every element
[id]                           any element with an `id` attr
[status="todo"]                any element with status="todo"
task[id]                       <task> with an id (any value)
task[id="4.1"]                 <task id="4.1">
task[id^="4."]                 <task> whose id starts with "4."
task[id$=".final"]             <task> whose id ends with ".final"
task[id*="."]                  <task> whose id contains "."
task[id="1.1"][status="todo"]  <task> with BOTH id and status (AND)
phase task                     any <task> descendant of a <phase>
phase > task                   <task> direct-child of <phase>
task, phase                    union (deduped by element identity)
task:first-child               <task> that is the first child of its parent
task:nth-child(2)              <task> that is the 2nd child of its parent (1-indexed)
task:not(:first-child)         every <task> except the first
task:not([status])             every <task> without a `status` attribute
*:not([id])                    every element without an `id` attribute
```

## Concept

- **Compile once, reuse many.** `Selector::parse(s)` is the hot-path entry — pay the parse cost up front, hand the `Selector` to every `select`/`update`/`replace_*` call. On the Node side the compile happens per call but stays microseconds.
- **CSS3 subset.** Familiar syntax (tags, attribute predicates, `>`, descendants, `,`, `:first-child`, `:nth-child(n)`, `:not(...)`). No sibling combinators, no `:has()`, no pseudo-elements.
- **Tag-less simples allowed.** `[id]` and `:first-child` work on their own (implicit universal target). This is also what makes `task:not([id])` work — the inner simple of `:not()` has no leading tag.
- **Matches are deduplicated** by element identity, so `task, *` doesn't yield duplicates.

## Reference

### Simple selectors

A simple selector is `(tag | *)? predicate*`. Either the tag, the universal `*`, or at least one predicate must be present.

| Form             | Meaning                                                |
| ---------------- | ------------------------------------------------------ |
| `tag`            | Elements whose tag name equals `tag` (case-sensitive). |
| `*`              | Any element.                                           |
| `tag pred…`      | `tag` AND every predicate.                             |
| `pred…`          | Any element matching every predicate (implicit `*`).   |

### Attribute predicates

| Form              | Matches when                                            |
| ----------------- | ------------------------------------------------------- |
| `[attr]`          | the attribute is present (any value).                   |
| `[attr="value"]`  | the attribute equals `value` exactly.                   |
| `[attr^="value"]` | the attribute value starts with `value`.                |
| `[attr$="value"]` | the attribute value ends with `value`.                  |
| `[attr*="value"]` | the attribute value contains `value` as a substring.    |

Multiple predicates on the same simple are AND-combined:

```
task[id="1.1"][status="todo"]   matches <task> with BOTH attrs
```

Quoted values are required (`[id="t1"]` works; `[id=t1]` errors with `expected '"'`). Inside the quotes, XML entity references decode (`&amp;` → `&`) so a selector value matches what the tokenizer stored on the element.

### Combinators

| Form     | Meaning                                                          |
| -------- | ---------------------------------------------------------------- |
| `a b`    | **Descendant** — `b` anywhere below `a` (any depth ≥ 1).         |
| `a > b`  | **Direct child** — `b` is a direct child of `a`.                 |
| `a, b`   | **Union** — match `a` OR `b`. Matches deduped by element identity. |

Chain them: `a b > c d` matches any `<d>` descendant of a `<c>` that is a direct child of a `<b>` that is a descendant of an `<a>`.

### Pseudo-classes

| Form               | Matches when                                                                       |
| ------------------ | ---------------------------------------------------------------------------------- |
| `:first-child`     | element is the first child of its parent.                                          |
| `:nth-child(n)`    | element is the `n`-th child (1-indexed). `n` is an integer ≥ 1.                    |
| `:not(simple)`     | element does NOT match the inner simple selector.                                  |

`:nth-child(0)` is rejected at parse time — siblings are 1-indexed. The inner argument of `:not(...)` is a single simple selector; nested `:not()` is permitted but capped at 64 levels of nesting before the parser rejects it as `NotNestingTooDeep` (see Limits table).

### Limits

Selector parsing rejects inputs that exceed any of the following. Exceeding a limit is a syntax error.

| Limit                                                 | Max |
| ----------------------------------------------------- | --- |
| Comma-separated compounds in one selector             | 64  |
| Space-separated simples in one compound chain         | 64  |
| Predicates (`[…]` / `:…`) on one simple selector      | 32  |
| Nesting depth of `:not(...)`                          | 64  |
| Digits in a `:nth-child(n)` argument                  | 11  |

## Examples

```
task                            # every <task>
*[id]                           # every element with an id
[id="4.1"]                      # any element with id="4.1" (tag-less)
task[status^="in-"]             # <task> with status starting "in-"
phase > task                    # direct-child tasks
phase task                      # descendant tasks (any depth)
task, phase                     # union
task:first-child                # leading task in any parent
task:nth-child(3)               # third child if it's a task
task:not([archived])            # tasks without `archived` attr
*:not(:first-child)             # everything except first children
task[id^="4."]:not([status])    # 4.x tasks missing a status
```

## Not supported

Each omission is deliberate; ordered by what we get asked about most often.

| Form                                | Why we don't ship it (yet)                                                                                                                  |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `a + b`, `a ~ b` (sibling)          | Would require a sibling scan at every match point; rarely needed for tree-shaped task/phase docs.                                            |
| `:has(...)`                         | Requires a second traversal pass — non-trivial implementation, post-0.1.0.                                                                  |
| `:contains(...)`                    | Selector grammar targets structure, not text content. Use `replaceInContent` with a regex for text matching.                                |
| `[attr="val" i]` (case-insensitive) | Schema regex constraints can express `(?i)` patterns today; the selector predicate flag is post-0.1.0.                                      |
| `:not(:not(...))`                   | Spec ambiguity at nesting; trivially derivable (`:not(...)` of the inner). Inner `:not()` argument is a single simple, not a full selector. |
| `:nth-of-type`, `:nth-last-child`   | Only `:nth-child(n)` is implemented. The others were noise on real docs.                                                                    |
| Pseudo-elements (`::before` etc.)   | DOM-specific; meaningless for a parsed markdown+XML tree.                                                                                   |

If any of these matter for your use case, [open an issue](https://github.com/thebytefarm/marxml/issues).

## Errors

`Selector::parse(s) -> Result<Selector, SelectorError>`:

| Variant                           | When                                                                                                         |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `SelectorError::Empty`            | Input is empty or only whitespace.                                                                           |
| `SelectorError::UnexpectedEnd`    | Input ends inside an unclosed `[...]`, `"..."`, or after a trailing `,`.                                     |
| `SelectorError::Syntax { reason, at }` | Anything else. `reason` is a short description; `at` is the 0-based byte offset where parsing failed.   |

Bad selectors fail once at parse time and never at query time — `Selector::parse` validates everything up front, including the size caps above. On the Node side the same errors surface as `Error` with code `InvalidArg`.

## See also

- [Schema](./schema.md) — declarative validation that uses the same element model.
- [Cookbook](./cookbook.md) — selector + mutator recipes side-by-side in Rust and TS.
- [Grammar](./grammar.md) — formal EBNF for tooling authors.
