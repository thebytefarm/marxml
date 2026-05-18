# Node reference

The `marxml` package on [npm](https://www.npmjs.com/package/marxml). The authoritative type spec lives in [`bindings/node/marxml.d.ts`](../../bindings/node/marxml.d.ts).

## At a glance

```ts
import { parse } from 'marxml'

const doc = parse(src)

for (const el of doc.select('task[status="todo"]')) {
  console.log(el.attrs.id)
}

const updated = doc.updateAttrs('task[status="todo"]', [
  { name: 'status', value: 'done' },
])
const xml  = doc.toXml({ pretty: true })
const json = doc.toJson()

const report = doc.validate({
  task: { attrs: { id: { kind: 'string', required: true } } },
})
```

## Install

```sh
pnpm add marxml          # or: npm i marxml / yarn add marxml / bun add marxml
```

## API

### parse

```ts
function parse(source: string): MarkdownDoc
```

Returns a `MarkdownDoc` handle. Throws on malformed input.

### MarkdownDoc

The factory return shape. No `new`, no class on the public surface — it's a plain object with bound methods.

**Reading**

```ts
readonly raw:      string         // original source, byte-for-byte
readonly elements: Element[]      // materialized root elements

select(selector: string): Element[]
```

**Mutating** — return the rewritten document as a new string:

```ts
updateAttrs(selector: string, newAttrs: { name: string; value: string }[]): string
replaceContent(selector: string, newBody: string): string            // RAW splice
replaceText(selector: string, newText: string): string               // escape-safe
replaceInContent(selector: string, pattern: string | RegExp, replacement: string): string
```

**Serializing**

```ts
toXml(opts?: { pretty?: boolean }): string
toJson(): unknown                 // returns parsed value, not a string
```

**Validating**

```ts
validate(schema: Record<string, TagSchemaShape>): ValidationReport
```

### Selectors

Passed as strings; compiled internally per call. There is no JS `Selector` class on the public surface — the compile cost is microseconds for typical patterns.

See [DSL · Selectors](../dsl/selectors.md) for the grammar.

### Schema

Authored as a plain JS object passed to `doc.validate(schema)`. No builder; compile happens internally.

See [DSL · Schema](../dsl/schema.md) for the validation semantics.

### Types

```ts
interface Element {
  tag:         string
  attrs:       Record<string, string>
  content:     string                // raw inner content
  children:    Element[]
  selfClosing: boolean
  loc:         SourceSpan
}

interface SourceSpan     { start: SourcePosition; end: SourcePosition }
interface SourcePosition { line: number; offset: number }     // line 1-based, offset 0-based byte

interface AttrUpdate     { name: string; value: string }
interface RegExpShape    { source: string; flags?: string }   // accepted by replaceInContent
interface ToXmlOpts      { pretty?: boolean }

interface ValidationReport { valid: boolean; errors: ValidationError[] }
interface ValidationError  { kind: string; tag: string; line: number; message: string }

interface TagSchemaShape {
  attrs?:             Record<string, AttrConstraintShape>
  childrenRequired?:  string[]
  childrenOptional?:  string[]
  childrenExclusive?: boolean
  contentRequired?:   boolean
}

interface AttrConstraintShape {
  kind:      'string' | 'enum' | 'regex'
  values?:   string[]      // when kind = 'enum'
  pattern?:  string        // when kind = 'regex'
  required?: boolean
}
```

## Behavior

- **Factory, not class.** `parse(src)` returns a plain object with methods. No `new`.
- **Parse once, reuse.** The handle holds the parsed document. `select` → `updateAttrs` → `toXml` on the same `doc` is one parse, not three.
- **Mutators return strings.** The handle itself is never modified. To chain mutations, `parse` the returned string.
- **`replaceContent` is a raw splice.** `<`, `&`, `"` are NOT escaped. Use `replaceText` for any string the user could control.
- **Regex flag mapping:**

  | JS flag         | Effect                                                       |
  | --------------- | ------------------------------------------------------------ |
  | `i`             | case-insensitive                                             |
  | `m`             | multiline (`^` / `$` match line boundaries)                  |
  | `s`             | dot matches newline                                          |
  | `x`             | ignore whitespace in the pattern                             |
  | `g`             | no-op (replace is always global)                             |
  | `u`, `y`, `d`   | no-op (no equivalent in the underlying engine; silently ignored) |

  `replacement` is **verbatim text** — `$1` / `$name` are NOT interpreted as capture references.

- **Errors throw.** Every fallible operation surfaces a structured `Error` with code `InvalidArg`. The host process is never panicked by caller-supplied input.
- **`toJson()` returns a parsed value,** not a string. No `JSON.parse` needed at the call site.

## Errors

Every method that can fail throws a standard `Error` with `code: 'InvalidArg'`. The `message` describes what went wrong:

| Method                                | Common error message                                                                  |
| ------------------------------------- | ------------------------------------------------------------------------------------- |
| `parse`                               | `unclosed tag at line N`, `mismatched close </X>`, `duplicate id "…" on <X>`, etc.    |
| `select` / any selector-taking method | `syntax error at offset N: …`, `unexpected end of selector`, `selector is empty`      |
| `updateAttrs`                         | `invalid XML attribute name "…"`, `duplicate attribute name "…" in update slice`      |
| `replaceInContent`                    | regex compile errors from the underlying engine                                       |
| `validate`                            | schema compile errors: `invalid regex for X.Y`, `duplicate tag …`, `invalid XML name in schema: …` |

`validate` itself does not throw on validation failures — it returns a `ValidationReport` whose `errors` array describes them. `ValidationError.kind` is one of `'missing_attr'`, `'invalid_attr'`, `'missing_child'`, `'unexpected_child'`, `'empty_content'`.

## Compatibility

- **Node:** ≥ 18.
- **Module system:** ESM only. Use `import`. No CJS entry.
- **Platforms** (prebuilt native binaries — npm picks the matching one automatically):

  | Platform                | Sub-package                       |
  | ----------------------- | --------------------------------- |
  | macOS arm64             | `marxml-darwin-arm64`             |
  | macOS x64               | `marxml-darwin-x64`               |
  | Linux x64 (glibc)       | `marxml-linux-x64-gnu`            |
  | Linux x64 (musl)        | `marxml-linux-x64-musl`           |
  | Linux arm64 (glibc)     | `marxml-linux-arm64-gnu`          |
  | Windows x64 (MSVC)      | `marxml-win32-x64-msvc`           |

  If the matching sub-package isn't picked up by npm (a known [npm optional-deps bug](https://github.com/npm/cli/issues/4828)): `rm -rf node_modules package-lock.json && npm i`.

## See also

- [DSL · Selectors](../dsl/selectors.md) · [DSL · Schema](../dsl/schema.md) · [DSL · Cookbook](../dsl/cookbook.md)
- [Rust reference](./rust.md)
- [Architecture](../ARCHITECTURE.md)
- [`bindings/node/marxml.d.ts`](../../bindings/node/marxml.d.ts) — authoritative type spec
