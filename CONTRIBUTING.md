# Contributing to marxml

Thanks for your interest. marxml is a single Rust workspace that ships a [crates.io](https://crates.io/crates/marxml) crate plus an [npm](https://www.npmjs.com/package/marxml) package built via [napi-rs](https://napi.rs/).

## Layout

```
crates/marxml/    # Rust core library
bindings/node/    # napi-rs Node bindings
benches/          # criterion benchmarks
```

## Setup

```sh
# Rust toolchain (pinned via rust-toolchain.toml)
rustup show

# Node binding deps
pnpm install
```

## Common tasks

```sh
just test         # cargo test + pnpm test
just lint         # cargo fmt --check + cargo clippy + oxlint
just fmt          # cargo fmt + oxfmt
just bench        # cargo bench
just cov          # cargo-llvm-cov report
just check        # everything (run before opening a PR)
```

If you don't have `just` installed: `brew install just` (macOS) or `cargo install just`.

## Coverage

We hold a **100% line coverage** bar workspace-wide. Genuinely-unreachable code (`unreachable!()`, defensive panics the compiler proves can't fire) gets explicit `// LCOV_EXCL_LINE` markers; these are reviewed in PR. If you can't cover a line, justify the exclusion in the PR description.

## Conventions

- Conventional Commits (`feat:`, `fix:`, `chore:`, `docs:`, etc.).
- One phase per PR, see [the plan](https://github.com/thebytefarm/marxml/blob/main/.specs/PLAN.md) for ordering.
- No `unsafe` (forbidden at workspace level).
- Run `just check` locally before pushing.

## Reporting bugs

Open an issue with a minimal repro. For parser bugs, include the input markdown and what was expected vs what happened.
