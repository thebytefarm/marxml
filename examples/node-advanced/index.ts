// node-advanced — selector reuse, mutation, validation, structured serialize.
//
// Reads samples/plan.md (a real markdown document with embedded XML),
// mutates it surgically, and writes two outputs:
//
// - out/plan.md  — full markdown document with edits applied. Headings,
//                  bullets, and prose are byte-preserved; only the inside
//                  of <task> / <phase> tags changes. Still renders cleanly
//                  on GitHub.
// - out/plan.xml — structured payload via toXml({ structured: true }). Single
//                  <markdown> root, indented, markdown noise stripped between
//                  siblings. Valid XML document (passes xmllint).
//
// From examples/node-advanced/:
//   pnpm install
//   pnpm start

import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { parse, type TagSchemaShape } from 'marxml'

const __dirname = dirname(fileURLToPath(import.meta.url))
const SAMPLE = resolve(__dirname, 'samples', 'plan.md')
const OUT_DIR = resolve(__dirname, 'out')

const src = readFileSync(SAMPLE, 'utf8')
const doc = parse(src)

const PENDING = 'task[status="todo"]'

console.log(`pending: ${doc.select(PENDING).length}`)
console.log(`total:   ${doc.select('task').length}`)

// (1) Mark every "todo" task as "done". updateAttrs returns the rewritten
// document. Markdown prose, headings, and bullets come back unchanged.
const afterUpdate = doc.updateAttrs(PENDING, [{ name: 'status', value: 'done' }])

// Mutators return strings; re-parse to chain further mutations.
const doc2 = parse(afterUpdate)

// (2) Swap one task's body. replaceText escapes `<`, `&`, `"` so the splice
// cannot break out of the parent tag.
const afterSwap = doc2.replaceText(
  'task[id="1.1"]',
  'design + document the schema DSL (priority)',
)
const doc3 = parse(afterSwap)

// (3) Validate the rewritten document.
const schema: Record<string, TagSchemaShape> = {
  task: {
    attrs: {
      id: { kind: 'string', required: true },
      status: { kind: 'enum', values: ['todo', 'done'], required: true },
    },
  },
  phase: {
    attrs: {
      id: { kind: 'string', required: true },
      status: {
        kind: 'enum',
        values: ['todo', 'in-progress', 'done'],
        required: true,
      },
    },
  },
}

const report = doc3.validate(schema)
if (report.valid) {
  console.log('validation: OK')
} else {
  console.log(`validation: ${report.errors.length} error(s)`)
  for (const e of report.errors) {
    console.log(`  - ${e.message}`)
  }
}

mkdirSync(OUT_DIR, { recursive: true })

// (4a) Full markdown document with surgical edits. `raw` is the source the
// most recent parse was built from — i.e. the post-mutation markdown, with
// every byte outside the touched tags identical to the input.
const mdPath = resolve(OUT_DIR, 'plan.md')
writeFileSync(mdPath, doc3.raw)
console.log(`\nwrote ${mdPath} (${doc3.raw.length} bytes) — full markdown with edits`)

// (4b) Structured payload as a single-root, valid XML document.
// `structured: true` = pretty + stripText + wrapIn:"markdown".
const xml = doc3.toXml({ structured: true })
const xmlPath = resolve(OUT_DIR, 'plan.xml')
writeFileSync(xmlPath, xml)
console.log(`wrote ${xmlPath} (${xml.length} bytes) — structured XML extract`)

console.log('\n-- markdown preview (out/plan.md) --')
console.log(doc3.raw)
console.log('\n-- xml preview (out/plan.xml) --')
console.log(xml)
