// Public types for the marxml Node binding. The factory at `marxml.mjs`
// returns `MarkdownDoc` — there is no class on the public surface.

export type {
  Element,
  SourcePosition,
  SourceSpan,
  AttrUpdate,
  TagSchemaShape,
  AttrConstraintShape,
  ValidationReport,
  ValidationError,
  ToXmlOpts,
} from './index.js'

import type {
  Element,
  AttrUpdate,
  SourceSpan,
  TagSchemaShape,
  ValidationReport,
  ToXmlOpts,
} from './index.js'

/**
 * Canonical tree node returned by {@link MarkdownDoc.toJson} and mirrored
 * as the parsed shape of {@link MarkdownDoc.toYaml}.
 *
 * Distinct from {@link Element} (the materialized view used by `select()`
 * and `doc.elements`): this shape carries the *direct* text segments of
 * each element joined into `text` (with child-element markup excluded),
 * and the source span under `location` instead of `loc`. Entity references
 * inside `text` are decoded to their literal characters.
 */
export interface MarkdownNode {
  /** Element tag name (e.g. `"task"`). */
  tag: string
  /** Attribute key/value pairs in source order. */
  attrs: Record<string, string>
  /**
   * Direct text content of the element, child-element markup excluded.
   * For `<p>before <em>x</em> after</p>` this is `"before  after"`.
   * Entity references are decoded.
   */
  text: string
  /** Recursively-serialized child elements, in source order. */
  children: MarkdownNode[]
  /** `true` for `<tag/>`, `false` for `<tag>…</tag>`. */
  selfClosing: boolean
  /** Source span covering the entire element. */
  location: SourceSpan
}

/**
 * Parsed document handle. Returned by {@link parse}. Methods close over the
 * parsed handle so subsequent queries and mutations reuse the same parse —
 * there is no per-call reparse.
 *
 * Mutations return the rewritten source as a string; they do NOT update the
 * handle in place. To chain mutations, re-`parse` the returned string.
 */
export interface MarkdownDoc {
  /** Original document source, byte-for-byte. */
  readonly raw: string

  /**
   * Materialized root elements of the document. Re-walks the parsed tree on
   * each access — cache the result if you read it repeatedly.
   */
  readonly elements: Element[]

  /**
   * Run a selector against the document and return every matching element
   * in source order.
   *
   * @example
   * doc.select('task[status="todo"] note')
   *
   * @throws {Error} `InvalidArg` on selector syntax errors.
   */
  select(selector: string): Element[]

  /**
   * Update or insert attributes on every element matching `selector`.
   * Returns the rewritten document. The handle is unchanged.
   *
   * @throws {Error} `InvalidArg` on invalid XML attribute names or
   * duplicate keys in `newAttrs`. Never panics on caller-supplied input.
   */
  updateAttrs(selector: string, newAttrs: AttrUpdate[]): string

  /**
   * Replace inner content verbatim. `newBody` is spliced as raw bytes —
   * `<` / `&` / `"` are NOT escaped.
   *
   * Use {@link MarkdownDoc.replaceText} for untrusted text.
   */
  replaceContent(selector: string, newBody: string): string

  /**
   * Replace inner content with `newText`, escaping `<` / `&` / `"` before
   * splicing. Safe for user-controlled strings.
   */
  replaceText(selector: string, newText: string): string

  /**
   * Run a regex `replace_all` over the inner content of matching elements.
   *
   * Accepts either a string pattern or a `RegExp`. JS regex flags
   * `i`/`m`/`s`/`x` are honored (translated to Rust-side flags); `g` is a
   * no-op (replace_all is global by default); `u`/`y`/`d` have no Rust
   * equivalent and are silently ignored.
   *
   * `replacement` is verbatim text — `$1` / `$name` are NOT interpreted as
   * capture references.
   *
   * @example
   * doc.replaceInContent('task', /draft/i, 'final')
   */
  replaceInContent(selector: string, pattern: string | RegExp, replacement: string): string

  /**
   * Serialize the parsed XML elements back to a string. Surrounding
   * markdown prose is dropped — this is just the structured payload.
   *
   * Options:
   * - `pretty` — indented multi-line output.
   * - `stripText` — drop non-whitespace text between sibling tags
   *   (markdown noise that would otherwise appear as XML mixed content).
   * - `wrapIn` — wrap output in `<name>...</name>` so a multi-root
   *   document becomes a single-root, well-formed XML *document*.
   * - `structured` — convenience shortcut equivalent to
   *   `{ pretty: true, stripText: true, wrapIn: "markdown" }`. Pass
   *   `wrapIn` alongside to override the wrapper name.
   */
  toXml(opts?: ToXmlOpts): string

  /**
   * Serialize the element tree as a JSON value. The native binding emits a
   * JSON string which the wrapper at `marxml.mjs` parses for you — at the
   * call site you receive a structured value, but the cost is two passes
   * (one Rust-side serialize, one V8 `JSON.parse`). For large documents
   * prefer `toXml({ pretty: false })` if you only need a serialized form.
   *
   * Without options: top-level is an array of root elements. See
   * {@link MarkdownNode} for the per-element shape.
   *
   * With `wrapIn`: top-level is `{ [wrapIn]: MarkdownNode[] }` — useful
   * for shape parity with `toXml({ structured: true })`.
   *
   * With `stripText`: the `text` field on every non-leaf element is
   * emptied, dropping the markdown noise that would otherwise sit between
   * sibling tags.
   *
   * `structured: true` is the convenience combination of both, with
   * `wrapIn` defaulting to `"markdown"`.
   *
   * `pretty` and `selfCloseEmpty` are ignored (JSON-irrelevant).
   */
  toJson(opts?: ToXmlOpts): MarkdownNode[] | Record<string, MarkdownNode[]>

  /**
   * Serialize the element tree as a YAML string. Same canonical shape as
   * {@link MarkdownDoc.toJson}. Without options the top-level is a YAML
   * sequence of {@link MarkdownNode}; with `wrapIn` it becomes a mapping
   * `wrapIn: [...]`. See {@link MarkdownDoc.toJson} for the full
   * `stripText` / `wrapIn` / `structured` semantics.
   */
  toYaml(opts?: ToXmlOpts): string

  /**
   * Validate the document against `schema` (per-tag declarations keyed by
   * tag name). Returns a report with every violation.
   *
   * @throws {Error} `InvalidArg` on invalid regex patterns, non-XML
   * tag/attribute names, or duplicate declarations. Never panics on
   * caller-supplied input.
   */
  validate(schema: Record<string, TagSchemaShape>): ValidationReport
}

/**
 * Parse a markdown + XML source string into a {@link MarkdownDoc}.
 *
 * @throws {Error} `InvalidArg` on malformed input.
 */
export declare function parse(source: string): MarkdownDoc
