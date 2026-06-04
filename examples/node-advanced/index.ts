// node-advanced — selector reuse, mutation, validation, pretty serialize.
//
// Reads samples/plan.md (never modified) and writes the rewritten document
// to out/plan.xml. Run ../reset.sh to clear out/.
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
// document. The handle is unchanged.
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

// (4) Pretty XML written to out/.
mkdirSync(OUT_DIR, { recursive: true })
const xml = doc3.toXml({ pretty: true })
const outPath = resolve(OUT_DIR, 'plan.xml')
writeFileSync(outPath, xml)
console.log(`\nwrote ${outPath} (${xml.length} bytes)`)

console.log('\n-- preview --')
console.log(xml)
