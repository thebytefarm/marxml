# Selector grammar

`marxml`'s selectors are a small CSS3 subset, hand-parsed in `crates/marxml/src/selector/parser.rs`. The grammar is identical in both the Rust API (`Selector::parse(s)`) and the Node binding (`select(src, "selector")`).

## Formal grammar

```
selector   := compound ( WS* "," WS* compound )*
compound   := simple ( combinator simple )*
combinator := WS+                       # descendant
            | WS* ">" WS*               # direct child
simple     := ( tag | "*" )? predicate*
predicate  := "[" attr [ op quoted ] "]"
            | ":" pseudo
op         := "=" | "^=" | "$=" | "*="
pseudo     := "first-child"
            | "nth-child(" digits ")"
            | "not(" simple ")"
tag        := name-start name-char*
attr       := name-start name-char*
name-start := [A-Za-z_]
name-char  := [A-Za-z0-9_\-.]
quoted     := '"' /[^"]*/ '"'
digits     := [0-9]+
```

Whitespace is significant only between consecutive simples (where it forms the descendant combinator). Whitespace around `>`, around `,`, inside `[ ]`, and inside `:not(...)` is allowed.

## Examples

### Tags and universal

| Selector | Matches |
| --- | --- |
| `task` | every `<task>` |
| `phase` | every `<phase>` |
| `*` | every element |

### Attribute predicates

| Selector | Matches |
| --- | --- |
| `task[id]` | `<task>` elements with any `id` attribute |
| `task[id="4.1"]` | `<task id="4.1">` |
| `task[id^="4."]` | `<task>` whose `id` starts with `4.` |
| `task[id$=".final"]` | `<task>` whose `id` ends with `.final` |
| `task[id*="."]` | `<task>` whose `id` contains `.` |
| `[id]` | any element with an `id` attribute |
| `[status="todo"]` | any element with `status="todo"` |

Multiple predicates on the same simple are AND-combined:

```
task[id="1.1"][status="todo"]
```

matches only `<task>` elements with *both* `id="1.1"` and `status="todo"`.

### Combinators

| Selector | Matches |
| --- | --- |
| `phase task` | any `<task>` that is a descendant of a `<phase>` at any depth |
| `phase > task` | any `<task>` that is a *direct* child of a `<phase>` |
| `a b > c d` | any `<d>` descendant of a `<c>` that is a direct child of a `<b>` that is a descendant of an `<a>` |

### Union

A comma separates compounds; matches are deduplicated by element identity, so a target satisfying multiple compounds appears once:

```
task, phase
*, task              # equivalent to `*` — every element, no dupes
task:not([id]), phase[id]
```

### Pseudo-classes

| Selector | Matches |
| --- | --- |
| `task:first-child` | a `<task>` that is the first child of its parent |
| `task:nth-child(2)` | the 2nd child of its parent (1-indexed) |
| `task:not(:first-child)` | every `<task>` except the first |
| `*:not([id])` | every element without an `id` attribute |
| `task:not([status])` | every `<task>` without a `status` attribute |

`:nth-child(0)` matches nothing (1-indexed).

`:not(...)` takes a single simple selector (tag, universal, or one or more predicates). Nested `:not(:not(...))` and `:has(...)` are not supported.

### Tag-less selectors

A simple selector can omit the tag and start with a predicate directly. The tag part defaults to `*`:

```
[id="4.1"]           # any element with id="4.1"
[status]             # any element with a status attr
:first-child         # the first child of any parent (i.e. every first child anywhere)
```

This is also what makes `task:not([id])` work — inside `:not(...)`, the inner simple has no tag, so it matches any element that has an `id` attribute.

## Not yet supported

- Sibling combinators: `~` (general sibling), `+` (adjacent sibling).
- `:has(...)` — would require a second traversal pass.
- `:contains(...)` — content-text matching.
- Case-insensitive flags: `[attr="val" i]`.
- Multiple attribute operators inside `:not(...)` are fine, but nested `:not(:not(...))` isn't.
- `:nth-of-type`, `:nth-last-child`, etc. — only `:nth-child(n)` for now.

These are tracked as post-0.1.0 work. If you need them, open an issue.

## Error reporting

`Selector::parse(s)` returns `Result<Selector, SelectorError>`:

| Variant | When |
| --- | --- |
| `Empty` | input is empty or only whitespace |
| `UnexpectedEnd` | input ends inside an unclosed `[...]` or `"..."` |
| `Syntax { reason, at }` | other parse problem; `at` is the 0-based byte offset |

Selectors are intentionally parsed up-front and reused — `Selector::parse` returns an opaque `Selector` you pass to every `select()` call, so bad selectors fail once at parse time and never at query time.
