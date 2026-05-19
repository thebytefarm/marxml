---
default: patch
---

#### Fix: build the published `.node` loader as ESM (`--esm`) so `import { parse }` actually works

`marxml@0.1.1` and `0.1.2` shipped with `index.js` and `index.d.ts` in the tarball (the 0.1.1 fix), but the loader was emitted as CJS by `napi build`. Meanwhile `marxml.mjs` does `import { parse as nativeParse } from './index.js'` — an ESM named-import against a CJS module. Node rejects that with:

```
SyntaxError: The requested module './index.js' does not provide an export named 'parse'
```

Root cause: the local `pnpm build` script passes `--esm`, but the matrix `napi build` invocations in `release.yml` didn't. The published loader was CJS while the wrapper expected ESM.

Fix: add `--esm` to every matrix entry in `release.yml#jobs.build.strategy.matrix.include[].build`.

Also adds a smoke-test step to the publish job that does a real `import { parse } from 'marxml'` against the assembled package on the runner (with `NAPI_RS_NATIVE_LIBRARY_PATH` pointing at the linux-x64-gnu binary). If parse can't be loaded or doesn't return the expected shape, the publish job aborts before anything reaches a registry. Prevents this exact class of "tarball ships, package broken" failure from recurring.
