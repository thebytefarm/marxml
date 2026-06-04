// node-simple — parse, select, serialize.
//
// Reads samples/notes.md and prints the structured payload back as XML and
// JSON. Read-only: never writes to disk.
//
// From examples/node-simple/:
//   pnpm install
//   pnpm start

import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { parse, type Element } from 'marxml'

const __dirname = dirname(fileURLToPath(import.meta.url))
const SAMPLE = resolve(__dirname, 'samples', 'notes.md')

const src = readFileSync(SAMPLE, 'utf8')
const doc = parse(src)

const ideas = doc.select('note[tag="idea"]')

console.log(`ideas (${ideas.length}):`)
for (const n of ideas) {
  const id = n.attrs.id ?? '?'
  console.log(`  ${id}  ${n.content.trim()}`)
}

console.log('\n-- toXml (compact) --')
console.log(doc.toXml())

console.log('\n-- toJson --')
const tree = doc.toJson() as Element[]
console.log(JSON.stringify(tree, null, 2))
