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
  TagSchemaShape,
  ValidationReport,
  ToXmlOpts,
} from './index.js'

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
   * Pass `{ pretty: true }` for indented multi-line output.
   */
  toXml(opts?: ToXmlOpts): string

  /**
   * Serialize the element tree as a JSON value (already parsed; no string
   * round-trip needed at the call site).
   *
   * Top-level is an array of root elements. Each element carries
   * `tag` / `attrs` / `text` / `children` / `selfClosing` / `location`.
   */
  toJson(): unknown

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
