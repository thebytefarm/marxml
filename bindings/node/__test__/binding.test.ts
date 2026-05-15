import { describe, expect, it } from 'vitest'

import {
  parse,
  select,
  updateAttrs,
  replaceContent,
  replaceInContent,
  toXml,
  toJson,
  validateSchema,
} from '../index.js'

describe('parse', () => {
  it('returns raw + elements', () => {
    const doc = parse('<task id="1">body</task>')
    expect(doc.raw).toBe('<task id="1">body</task>')
    expect(doc.elements).toHaveLength(1)
    const el = doc.elements[0]
    expect(el.tag).toBe('task')
    expect(el.attrs.id).toBe('1')
    expect(el.content).toBe('body')
    expect(el.selfClosing).toBe(false)
  })

  it('parses self-close', () => {
    const doc = parse('<spacer/>')
    expect(doc.elements[0].selfClosing).toBe(true)
  })

  it('parses nested elements', () => {
    const doc = parse('<phase id="1"><task id="1.1"/></phase>')
    const phase = doc.elements[0]
    expect(phase.children).toHaveLength(1)
    expect(phase.children[0].tag).toBe('task')
    expect(phase.children[0].attrs.id).toBe('1.1')
  })

  it('throws on malformed input', () => {
    expect(() => parse('<unclosed')).toThrow()
  })

  it('reports source location', () => {
    const doc = parse('first\n<task/>')
    const el = doc.elements[0]
    expect(el.loc.start.line).toBe(2)
    expect(el.loc.start.offset).toBe(6)
  })
})

describe('select', () => {
  it('returns matching elements', () => {
    const doc = '<task id="1"/><task id="2"/><phase id="3"/>'
    const tasks = select(doc, 'task')
    expect(tasks.map((t) => t.attrs.id)).toEqual(['1', '2'])
  })

  it('supports attribute filters', () => {
    const doc = '<task id="1.1"/><task id="2.1"/>'
    const matches = select(doc, 'task[id^="1."]')
    expect(matches).toHaveLength(1)
    expect(matches[0].attrs.id).toBe('1.1')
  })

  it('supports descendant combinator', () => {
    const matches = select('<phase><task><note/></task></phase>', 'phase note')
    expect(matches).toHaveLength(1)
    expect(matches[0].tag).toBe('note')
  })

  it('throws on bad selector', () => {
    expect(() => select('<a/>', 'a[')).toThrow()
  })
})

describe('updateAttrs', () => {
  it('replaces an existing attribute value', () => {
    const out = updateAttrs('<task id="1" status="todo"/>', 'task', [
      { name: 'status', value: 'done' },
    ])
    expect(out).toBe('<task id="1" status="done"/>')
  })

  it('appends a new attribute', () => {
    const out = updateAttrs('<task id="1"/>', 'task', [{ name: 'status', value: 'done' }])
    expect(out).toBe('<task id="1" status="done"/>')
  })

  it('no-op when no selector match', () => {
    const src = '<task/>'
    expect(updateAttrs(src, 'phase', [{ name: 'x', value: 'y' }])).toBe(src)
  })
})

describe('replaceContent', () => {
  it('replaces inner content', () => {
    const out = replaceContent('<task>old</task>', 'task', 'new')
    expect(out).toBe('<task>new</task>')
  })

  it('skips self-closing tags', () => {
    const src = '<spacer/>'
    expect(replaceContent(src, 'spacer', 'ignored')).toBe(src)
  })
})

describe('replaceInContent', () => {
  it('accepts a string pattern', () => {
    const out = replaceInContent('<note>foo foo</note>', 'note', 'foo', 'bar')
    expect(out).toBe('<note>bar bar</note>')
  })

  it('accepts a JS RegExp via {source, flags}', () => {
    const re = /foo/g
    const out = replaceInContent('<note>foo</note>', 'note', { source: re.source, flags: re.flags }, 'bar')
    expect(out).toBe('<note>bar</note>')
  })

  it('throws on bad regex', () => {
    expect(() => replaceInContent('<a>x</a>', 'a', '[', 'x')).toThrow()
  })
})

describe('toXml', () => {
  it('emits just the structured payload', () => {
    expect(toXml('intro\n<task/>\noutro')).toBe('<task/>')
  })

  it('pretty-prints when asked', () => {
    expect(toXml('<root><a/><b/></root>', true)).toBe('<root>\n  <a/>\n  <b/>\n</root>')
  })
})

describe('toJson', () => {
  it('returns a JSON string of the tree', () => {
    const json = JSON.parse(toJson('<task id="1"/>'))
    expect(json).toHaveLength(1)
    expect(json[0].tag).toBe('task')
    expect(json[0].attrs.id).toBe('1')
    expect(json[0].selfClosing).toBe(true)
  })
})

describe('validateSchema', () => {
  it('passes a compliant doc', () => {
    // `contentRequired` checks direct text content — child-only bodies
    // don't satisfy it.
    const report = validateSchema(
      '<task id="1" status="todo">write tests<status>todo</status></task>',
      {
        task: {
          attrs: {
            id: { kind: 'string', required: true },
            status: { kind: 'enum', values: ['todo', 'done'] },
          },
          childrenRequired: ['status'],
          contentRequired: true,
        },
      },
    )
    expect(report.valid).toBe(true)
    expect(report.errors).toHaveLength(0)
  })

  it('reports missing required attr', () => {
    const report = validateSchema('<task status="todo"/>', {
      task: { attrs: { id: { kind: 'string', required: true } } },
    })
    expect(report.valid).toBe(false)
    expect(report.errors.some((e) => e.kind === 'missing_attr')).toBe(true)
  })

  it('reports invalid enum value', () => {
    const report = validateSchema('<task id="1" status="bogus"/>', {
      task: {
        attrs: {
          id: { kind: 'string', required: true },
          status: { kind: 'enum', values: ['todo', 'done'] },
        },
      },
    })
    expect(report.errors.some((e) => e.kind === 'invalid_attr')).toBe(true)
  })
})
