---
default: patch
---

#### Fix: include `index.js` and `index.d.ts` in the published npm tarball

`marxml@0.1.0` shipped without `index.js` and `index.d.ts` — the napi-rs-generated platform-dispatch loader. The two files were in the package's `files` array, but they're gitignored and were only generated in the per-platform build matrix jobs. The publish job checked out fresh and ran `npm publish` against a workspace that didn't have them, so npm silently tarballed only 4 files (`README.md`, `marxml.mjs`, `marxml.d.ts`, `package.json`).

Effect: any consumer running `import { parse } from 'marxml'` got `UNRESOLVED_IMPORT` because `marxml.mjs` re-exports from the missing `./index.js`. The package was effectively non-functional.

Fix: upload `index.js` + `index.d.ts` as a separate `js-bindings` artifact from one build matrix entry, download it into `bindings/node/` in the publish job before `npm publish` runs. No code changes — pure CI pipeline fix.

Also tightens the publish job's existing `Collect artifacts` step to filter `pattern: bindings-*` so the new `js-bindings` artifact doesn't get mixed into the per-platform binary dirs.
