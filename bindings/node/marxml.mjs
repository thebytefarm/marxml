// Factory wrapper over the napi class. End users see `parse(src)` returning
// a plain object with bound methods — never `new NativeMarkdown(...)`.

import { parse as nativeParse } from './index.js'

/**
 * Parse a markdown + XML source string into a `MarkdownDoc` handle.
 *
 * The returned object is a plain factory product (no `new`, no class). Each
 * method closes over the parsed handle so subsequent queries and mutations
 * reuse the same parse — no per-call reparse.
 *
 * @param {string} source - The document to parse.
 * @returns {MarkdownDoc}
 * @throws {Error} When `source` is malformed XML/markdown.
 */
export function parse(source) {
  return wrap(nativeParse(source))
}

function wrap(m) {
  return {
    get raw() {
      return m.raw
    },
    get elements() {
      return m.elements
    },
    select(selector) {
      return m.select(selector)
    },
    updateAttrs(selector, newAttrs) {
      return m.updateAttrs(selector, newAttrs)
    },
    replaceContent(selector, newBody) {
      return m.replaceContent(selector, newBody)
    },
    replaceText(selector, newText) {
      return m.replaceText(selector, newText)
    },
    replaceInContent(selector, pattern, replacement) {
      const p =
        pattern instanceof RegExp
          ? { source: pattern.source, flags: pattern.flags }
          : pattern
      return m.replaceInContent(selector, p, replacement)
    },
    toXml(opts) {
      return m.toXml(opts ?? null)
    },
    toJson(opts) {
      return JSON.parse(m.toJson(opts ?? null))
    },
    toYaml(opts) {
      return m.toYaml(opts ?? null)
    },
    validate(schema) {
      return m.validate(schema)
    },
  }
}

/**
 * @typedef {import('./index.js').Element} Element
 * @typedef {import('./index.js').AttrUpdate} AttrUpdate
 * @typedef {import('./index.js').TagSchemaShape} TagSchemaShape
 * @typedef {import('./index.js').ValidationReport} ValidationReport
 * @typedef {import('./index.js').ToXmlOpts} ToXmlOpts
 */

/**
 * @typedef {object} MarkdownDoc
 * @property {string} raw                Original document source.
 * @property {Element[]} elements        Materialized root elements (re-walked on each access).
 * @property {(selector: string) => Element[]} select
 * @property {(selector: string, newAttrs: AttrUpdate[]) => string} updateAttrs
 * @property {(selector: string, newBody: string) => string} replaceContent
 * @property {(selector: string, newText: string) => string} replaceText
 * @property {(selector: string, pattern: string | RegExp, replacement: string) => string} replaceInContent
 * @property {(opts?: ToXmlOpts) => string} toXml
 * @property {(opts?: ToXmlOpts) => unknown} toJson  Returns the parsed JSON tree (not a string).
 * @property {(opts?: ToXmlOpts) => string} toYaml   YAML string mirroring the JSON shape.
 * @property {(schema: Record<string, TagSchemaShape>) => ValidationReport} validate
 */
