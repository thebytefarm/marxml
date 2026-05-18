# Cookbook

Common tasks, side-by-side Rust + TypeScript. Each recipe is self-contained — copy and adapt.

## Find every matching element

```rust
// Rust
let doc = marxml::parse(src)?;
let sel = marxml::Selector::parse(r#"task[status="todo"]"#)?;

for task in doc.select(&sel) {
    println!("{}", task.attr("id").unwrap_or(""));
}
```

```ts
// TypeScript
import { parse } from 'marxml'

const doc = parse(src)
for (const task of doc.select('task[status="todo"]')) {
  console.log(task.attrs.id)
}
```

## Update an attribute on every match

```rust
// Rust
let sel = marxml::Selector::parse(r#"task[status="todo"]"#)?;
let updated = doc.update(&sel, &[("status", "in_progress")]);
std::fs::write("doc.md", updated)?;
```

```ts
// TypeScript
const updated = doc.updateAttrs('task[status="todo"]', [
  { name: 'status', value: 'in_progress' },
])
await fs.writeFile('doc.md', updated)
```

`update` / `updateAttrs` insert the attribute if it's missing, replace its value if it's present. Other attributes on the same element are untouched.

## Replace inner content safely (user-controlled string)

```rust
// Rust — escape-safe
let updated = doc.replace_text(&sel, user_input);
```

```ts
// TypeScript — escape-safe
const updated = doc.replaceText('task', userInput)
```

Use `replace_text` / `replaceText` for any string the user could control. `<`, `&`, `"` are escaped before splicing, so `</task><evil/>` becomes `&lt;/task&gt;&lt;evil/&gt;` — it can't break out of the element.

`replace_content` / `replaceContent` is the **raw-splice** variant; use only when you're splicing trusted XML markup.

## Find direct children only

```rust
// Rust
let sel = marxml::Selector::parse("phase > task")?;
for task in doc.select(&sel) { /* … */ }
```

```ts
// TypeScript
const tasks = doc.select('phase > task')
```

The `>` combinator restricts the match to direct children. Without it, `phase task` would also match `<task>` nested deeper inside the `<phase>`.

## Validate every task has required attrs

```rust
// Rust
use marxml::{Schema, schema::AttrKind};

let schema = Schema::builder()
    .tag("task", |t| {
        t.attr("id",     AttrKind::String.required())
         .attr("status", AttrKind::one_of(["todo", "done"]).required())
    })
    .try_build()?;

let report = marxml::validate(&doc, &schema);
if !report.is_valid() {
    for err in report.errors() {
        eprintln!("{err}");
    }
    std::process::exit(1);
}
```

```ts
// TypeScript
const report = doc.validate({
  task: {
    attrs: {
      id:     { kind: 'string', required: true },
      status: { kind: 'enum', values: ['todo', 'done'], required: true },
    },
  },
})

if (!report.valid) {
  for (const err of report.errors) {
    console.error(`[${err.kind}] ${err.tag} line ${err.line}: ${err.message}`)
  }
  process.exit(1)
}
```

## Build a schema from a config file

```rust
// Rust — schema declarations in TOML, deserialized via serde
#[derive(serde::Deserialize)]
struct TagDecl {
    attrs: std::collections::BTreeMap<String, AttrDecl>,
    #[serde(default)] required_children: Vec<String>,
}

#[derive(serde::Deserialize)]
struct AttrDecl { kind: String, #[serde(default)] values: Vec<String>, #[serde(default)] required: bool }

let decls: std::collections::BTreeMap<String, TagDecl> =
    toml::from_str(&std::fs::read_to_string("schema.toml")?)?;

let mut builder = marxml::Schema::builder();
for (tag, decl) in decls {
    builder = builder.tag(&tag, |mut t| {
        for (name, attr) in decl.attrs {
            let kind = match attr.kind.as_str() {
                "enum"  => AttrKind::Enum(attr.values),
                "regex" => AttrKind::Regex(attr.values.into_iter().next().unwrap_or_default()),
                _       => AttrKind::String,
            };
            let constraint = if attr.required { kind.required() } else { kind.optional() };
            t = t.attr(&name, constraint);
        }
        for child in decl.required_children { t = t.child_required(child); }
        t
    });
}
let schema = builder.try_build()?;
```

```ts
// TypeScript — schema JSON loaded at startup, passed straight in
import schemaJson from './schema.json' assert { type: 'json' }
const report = doc.validate(schemaJson)
```

The Node binding accepts the schema as a plain object every call, so loading from JSON is `import + pass`. The Rust side compiles once via `Schema::builder()` — useful when the same schema runs against many documents.

## Replace text inside elements with a regex

```rust
// Rust — safe (escapes the replacement) and raw variants
let re = regex::Regex::new(r"draft").unwrap();

// Escape-safe: replacement is escaped before splicing
let safe   = doc.replace_text_in(&sel, &re, "final");

// Raw: replacement is verbatim — only for trusted markup
let raw    = doc.replace_in(&sel, &re, "<final/>");
```

```ts
// TypeScript — pass either a RegExp or a {source, flags} shape
const out  = doc.replaceInContent('task', /draft/gi, 'final')

// Inline-flag form when you have a string pattern + flags string:
const out2 = doc.replaceInContent('task', { source: 'draft', flags: 'i' }, 'final')
```

JS regex flags `i`, `m`, `s`, `x` are honored; `g` is a no-op (replace_all is always global); `u`, `y`, `d` have no Rust-side equivalent and are silently ignored.

`replacement` is verbatim text — `$1` / `$name` are NOT interpreted as capture references.

## Allowlist children with `:not`-style validation

```rust
// Rust
let schema = marxml::Schema::builder()
    .tag("phase", |t| {
        t.child_required("task")
         .child_optional("note")
         .exclusive_children()
    })
    .try_build()?;
```

```ts
// TypeScript
const report = doc.validate({
  phase: {
    childrenRequired:  ['task'],
    childrenOptional:  ['note'],
    childrenExclusive: true,
  },
})
```

Any child that isn't `<task>` or `<note>` under a `<phase>` triggers `unexpected_child` / `ValidationError::UnexpectedChild`.

## Read source positions for diagnostic output

```rust
// Rust
for el in doc.select(&sel) {
    let span = el.location();
    println!("{} at {}:{}–{}:{}", el.tag(),
        span.start.line, span.start.offset,
        span.end.line,   span.end.offset);
}
```

```ts
// TypeScript
for (const el of doc.select(sel)) {
  const { start, end } = el.loc
  console.log(`${el.tag} at ${start.line}:${start.offset}–${end.line}:${end.offset}`)
}
```

Locations are present on every `Element` / `ElementRef`. Line numbers are 1-based, byte offsets are 0-based.

## Walk nested same-tag elements

```rust
// Rust — find every nested <task> inside another <task>
let sel = marxml::Selector::parse("task task")?;
for inner in doc.select(&sel) { /* … */ }
```

```ts
// TypeScript
for (const inner of doc.select('task task')) { /* … */ }
```

marxml allows same-tag nesting (`<task><task/></task>`), unlike some XML/HTML stacks. Descendant selectors compose normally across nesting.

## See also

- [Selectors](./selectors.md) — full selector grammar.
- [Schema](./schema.md) — full validation reference.
- [Node reference](../reference/node.md) — `MarkdownDoc` surface.
- [Rust reference](../reference/rust.md) — `Markdown` surface.
