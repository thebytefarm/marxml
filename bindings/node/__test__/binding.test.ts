import { describe, expect, it } from 'vitest'

import { parse } from '../marxml.mjs'

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
    const doc = parse('<task id="1"/><task id="2"/><phase id="3"/>')
    const tasks = doc.select('task')
    expect(tasks.map((t) => t.attrs.id)).toEqual(['1', '2'])
  })

  it('supports attribute filters', () => {
    const doc = parse('<task id="1.1"/><task id="2.1"/>')
    const matches = doc.select('task[id^="1."]')
    expect(matches).toHaveLength(1)
    expect(matches[0].attrs.id).toBe('1.1')
  })

  it('supports descendant combinator', () => {
    const doc = parse('<phase><task><note/></task></phase>')
    const matches = doc.select('phase note')
    expect(matches).toHaveLength(1)
    expect(matches[0].tag).toBe('note')
  })

  it('throws on bad selector', () => {
    const doc = parse('<a/>')
    expect(() => doc.select('a[')).toThrow()
  })
})

describe('updateAttrs', () => {
  it('replaces an existing attribute value', () => {
    const doc = parse('<task id="1" status="todo"/>')
    const out = doc.updateAttrs('task', [{ name: 'status', value: 'done' }])
    expect(out).toBe('<task id="1" status="done"/>')
  })

  it('appends a new attribute', () => {
    const doc = parse('<task id="1"/>')
    const out = doc.updateAttrs('task', [{ name: 'status', value: 'done' }])
    expect(out).toBe('<task id="1" status="done"/>')
  })

  it('no-op when no selector match', () => {
    const doc = parse('<task/>')
    expect(doc.updateAttrs('phase', [{ name: 'x', value: 'y' }])).toBe('<task/>')
  })

  it('throws (not panics) on invalid XML attr name', () => {
    const doc = parse('<task id="1"/>')
    expect(() => doc.updateAttrs('task', [{ name: '1bad', value: 'x' }])).toThrow()
  })

  it('throws on duplicate attribute name in update slice', () => {
    const doc = parse('<task id="1"/>')
    expect(() =>
      doc.updateAttrs('task', [
        { name: 'id', value: 'a' },
        { name: 'id', value: 'b' },
      ]),
    ).toThrow()
  })
})

describe('replaceContent', () => {
  it('replaces inner content', () => {
    const doc = parse('<task>old</task>')
    expect(doc.replaceContent('task', 'new')).toBe('<task>new</task>')
  })

  it('skips self-closing tags', () => {
    const doc = parse('<spacer/>')
    expect(doc.replaceContent('spacer', 'ignored')).toBe('<spacer/>')
  })

  it('splices raw bytes — markup is NOT escaped', () => {
    const doc = parse('<task>old</task>')
    expect(doc.replaceContent('task', '<inner/>')).toBe('<task><inner/></task>')
  })
})

describe('replaceText', () => {
  it('escapes special characters before splicing', () => {
    const doc = parse('<task>old</task>')
    // `<`, `&`, `"` should round-trip as `&lt;`, `&amp;`, etc.
    expect(doc.replaceText('task', '<not-a-tag> & "quote"')).toBe(
      '<task>&lt;not-a-tag&gt; &amp; "quote"</task>',
    )
  })

  it('safe for user-controlled strings (cannot escape parent tag)', () => {
    const doc = parse('<task>x</task>')
    const out = doc.replaceText('task', '</task><evil/>')
    expect(out).toBe('<task>&lt;/task&gt;&lt;evil/&gt;</task>')
  })
})

describe('replaceInContent', () => {
  it('accepts a string pattern', () => {
    const doc = parse('<note>foo foo</note>')
    expect(doc.replaceInContent('note', 'foo', 'bar')).toBe('<note>bar bar</note>')
  })

  it('accepts a JS RegExp instance directly', () => {
    const doc = parse('<note>foo</note>')
    expect(doc.replaceInContent('note', /foo/g, 'bar')).toBe('<note>bar</note>')
  })

  it('honors the `i` flag from a JS RegExp', () => {
    const doc = parse('<note>FOO foo</note>')
    expect(doc.replaceInContent('note', /foo/gi, 'bar')).toBe('<note>bar bar</note>')
  })

  it('honors the `m` flag from a JS RegExp', () => {
    const doc = parse('<note>line1\nline2</note>')
    // ^ anchors to line start under `m`
    expect(doc.replaceInContent('note', /^line/gm, 'row')).toBe(
      '<note>row1\nrow2</note>',
    )
  })

  it('accepts a destructured RegExpShape', () => {
    const doc = parse('<note>FOO</note>')
    expect(doc.replaceInContent('note', { source: 'foo', flags: 'i' }, 'bar')).toBe(
      '<note>bar</note>',
    )
  })

  it('throws on bad regex', () => {
    const doc = parse('<a>x</a>')
    expect(() => doc.replaceInContent('a', '[', 'x')).toThrow()
  })
})

describe('toXml', () => {
  it('emits just the structured payload', () => {
    const doc = parse('intro\n<task/>\noutro')
    expect(doc.toXml()).toBe('<task/>')
  })

  it('pretty-prints when asked', () => {
    const doc = parse('<root><a/><b/></root>')
    expect(doc.toXml({ pretty: true })).toBe('<root>\n  <a/>\n  <b/>\n</root>')
  })
})

describe('toJson', () => {
  it('returns a parsed tree (not a string)', () => {
    const doc = parse('<task id="1"/>')
    const json = doc.toJson() as Array<{ tag: string; attrs: Record<string, string>; selfClosing: boolean }>
    expect(json).toHaveLength(1)
    expect(json[0].tag).toBe('task')
    expect(json[0].attrs.id).toBe('1')
    expect(json[0].selfClosing).toBe(true)
  })
})

describe('validate', () => {
  it('passes a compliant doc', () => {
    const doc = parse('<task id="1" status="todo">write tests<status>todo</status></task>')
    const report = doc.validate({
      task: {
        attrs: {
          id: { kind: 'string', required: true },
          status: { kind: 'enum', values: ['todo', 'done'] },
        },
        childrenRequired: ['status'],
        contentRequired: true,
      },
    })
    expect(report.valid).toBe(true)
    expect(report.errors).toHaveLength(0)
  })

  it('reports missing required attr', () => {
    const doc = parse('<task status="todo"/>')
    const report = doc.validate({
      task: { attrs: { id: { kind: 'string', required: true } } },
    })
    expect(report.valid).toBe(false)
    expect(report.errors.some((e) => e.kind === 'missing_attr')).toBe(true)
  })

  it('reports invalid enum value', () => {
    const doc = parse('<task id="1" status="bogus"/>')
    const report = doc.validate({
      task: {
        attrs: {
          id: { kind: 'string', required: true },
          status: { kind: 'enum', values: ['todo', 'done'] },
        },
      },
    })
    expect(report.errors.some((e) => e.kind === 'invalid_attr')).toBe(true)
  })

  it('throws (not panics) on invalid regex pattern in schema', () => {
    const doc = parse('<task id="1"/>')
    expect(() =>
      doc.validate({
        task: { attrs: { id: { kind: 'regex', pattern: '[' } } },
      }),
    ).toThrow()
  })
})

describe('handle reuse', () => {
  it('parses once; subsequent calls reuse the handle', () => {
    const doc = parse('<task id="1" status="todo"/>')
    // Multiple calls in a row don't re-parse the source.
    const matches = doc.select('task')
    const updated = doc.updateAttrs('task', [{ name: 'status', value: 'done' }])
    const xml = doc.toXml()
    expect(matches).toHaveLength(1)
    expect(updated).toBe('<task id="1" status="done"/>')
    expect(xml).toBe('<task id="1" status="todo"/>')
  })
})
