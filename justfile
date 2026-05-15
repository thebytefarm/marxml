# marxml task runner
# Run `just` (no args) to see all tasks.

default:
    @just --list

# Run all checks (fmt, clippy, test, cov) — same gates as CI
check: fmt-check clippy test

# Format Rust + Node sources
fmt:
    cargo fmt --all

# Verify formatting without modifying files
fmt-check:
    cargo fmt --all -- --check

# Run clippy with workspace lints
clippy:
    cargo clippy --all-targets --all-features --workspace -- -D warnings

# Run the test suites (Rust core + Node binding)
test:
    cargo test --all-features --workspace

# Property tests with more cases (slower; run before pushing)
test-thorough:
    PROPTEST_CASES=10000 cargo test --all-features --workspace

# Run benchmarks (criterion locally; CodSpeed wraps this in CI)
bench:
    cargo bench --workspace

# Verify version sync (crate vs npm)
versions:
    ./scripts/check-versions.sh

# Generate coverage report (HTML in target/llvm-cov/html)
cov:
    cargo llvm-cov --all-features --workspace --html
    @echo "report: target/llvm-cov/html/index.html"

# Print coverage summary
cov-summary:
    cargo llvm-cov --all-features --workspace --summary-only

# Clean build artifacts
clean:
    cargo clean
    rm -rf target/llvm-cov
