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

  it('stripText drops markdown noise between siblings', () => {
    const doc = parse('<phase>\nprose\n<task id="1"/>\nmore prose\n<task id="2"/>\n</phase>')
    const out = doc.toXml({ pretty: true, stripText: true })
    expect(out).toBe('<phase>\n  <task id="1"/>\n  <task id="2"/>\n</phase>')
  })

  it('wrapIn produces a single-root document', () => {
    const doc = parse('<a/><b/>')
    const out = doc.toXml({ pretty: true, wrapIn: 'doc' })
    expect(out).toBe('<doc>\n  <a/>\n  <b/>\n</doc>')
  })

  it('structured: true is a shortcut for the clean valid-XML shape', () => {
    const doc = parse('<phase>\nprose\n<task id="1"/>\n</phase>\n\n<phase>\n<task id="2"/>\n</phase>')
    const out = doc.toXml({ structured: true })
    expect(out.startsWith('<markdown>')).toBe(true)
    expect(out.endsWith('</markdown>')).toBe(true)
    expect(out.includes('prose')).toBe(false)
    expect(out.includes('<task id="1"/>')).toBe(true)
    expect(out.includes('<task id="2"/>')).toBe(true)
  })

  it('structured + wrapIn lets callers override the wrapper name', () => {
    const doc = parse('<a/>')
    const out = doc.toXml({ structured: true, wrapIn: 'plan' })
    expect(out.startsWith('<plan>')).toBe(true)
    expect(out.endsWith('</plan>')).toBe(true)
  })
})

describe('toYaml', () => {
  it('returns a YAML string for a simple element', () => {
    const doc = parse('<task id="1">body</task>')
    const yaml = doc.toYaml()
    expect(typeof yaml).toBe('string')
    expect(yaml).toMatch(/tag: task/)
    expect(yaml).toMatch(/id: '1'|id: "1"/) // quoted by saphyr
    expect(yaml).toMatch(/body/)
  })

  it('shape mirrors toJson', () => {
    const doc = parse('<phase id="1"><task id="1.1"/></phase>')
    const json = doc.toJson() as Array<{ tag: string; children: Array<{ tag: string }> }>
    expect(json).toHaveLength(1)
    expect(json[0].tag).toBe('phase')
    expect(json[0].children[0].tag).toBe('task')

    // The YAML string must mention the same tags.
    const yaml = doc.toYaml()
    expect(yaml).toMatch(/tag: phase/)
    expect(yaml).toMatch(/tag: task/)
  })

  it('emits an empty sequence for a document with no elements', () => {
    const doc = parse('just markdown, no tags here')
    const yaml = doc.toYaml()
    // serde-saphyr renders an empty array as `[]`.
    expect(yaml.trim()).toBe('[]')
  })
})

describe('toJson/toYaml opts', () => {
  it('toJson wrapIn produces a top-level object', () => {
    const doc = parse('<a/><b/>')
    const out = doc.toJson({ wrapIn: 'markdown' }) as Record<string, Array<{ tag: string }>>
    expect(out.markdown).toBeDefined()
    expect(out.markdown).toHaveLength(2)
    expect(out.markdown[0].tag).toBe('a')
  })

  it('toJson stripText empties non-leaf text', () => {
    const doc = parse('<phase>noise<task>body</task></phase>')
    const out = doc.toJson({ stripText: true }) as Array<{ text: string; children: Array<{ text: string }> }>
    expect(out[0].text).toBe('')        // non-leaf stripped
    expect(out[0].children[0].text).toBe('body') // leaf kept
  })

  it('toJson structured: true combines wrap + strip with markdown key', () => {
    const doc = parse('<phase>noise<task id="1">leaf</task></phase>')
    const out = doc.toJson({ structured: true }) as Record<string, Array<{ tag: string; text: string }>>
    expect(out.markdown).toBeDefined()
    expect(out.markdown[0].tag).toBe('phase')
    expect(out.markdown[0].text).toBe('')
  })

  it('toYaml structured: true emits a mapping root', () => {
    const doc = parse('<a/>')
    const yaml = doc.toYaml({ structured: true })
    expect(yaml).toMatch(/^markdown:/)
  })

  it('toYaml wrapIn overrides the default markdown wrapper', () => {
    const doc = parse('<a/>')
    const yaml = doc.toYaml({ structured: true, wrapIn: 'plan' })
    expect(yaml).toMatch(/^plan:/)
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
