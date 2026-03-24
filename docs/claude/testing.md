# Testing — Coffee Crossword

## Where tests live

| File | Purpose |
|---|---|
| `src-tauri/src/engine/tests.rs` | Engine pattern matching tests |
| `src-tauri/src/engine/test_utils.rs` | Shared test word list and helpers |
| `src-tauri/src/cache.rs` | Inline `#[cfg(test)]` module for cache build/read |
| `src-tauri/src/registry.rs` | Inline `#[cfg(test)]` module for registry logic |
| `src-tauri/src/dedup.rs` | Inline `#[cfg(test)]` module for deduplication |

## How to run tests

```bash
cd src-tauri && cargo test
```

---

## Coverage requirements

- Every Rust function in `engine/`, `cache.rs`, `registry.rs`, and `dedup.rs` must have unit tests.
- Tests must be added in the same PR/session as the code they cover — no deferred test writing.

---

## Test rules (standing — always follow)

### Test word list rule
The test word list in `test_utils.rs` must contain at least one word matching
every test pattern. Before writing a new test, verify a matching word exists in
the list. Add words to the list if needed.

### Example pattern validation rule
Any example pattern + match pair that appears in tests, documentation, or the
reference panel must be manually verified before including it. Do not assume a
pattern works — run it.

### Test cross-product rule
Each pattern type must be tested:
1. Standalone (by itself)
2. In combination with every other pattern type at least once

This ensures interactions between features are covered. The cross-product matrix
lives implicitly in `tests.rs` — when adding a new pattern type, scan existing
tests and add any missing combination cases.

### Pattern reference maintenance (test component)
Every time a new pattern type is implemented, a test must be added to
`engine/tests.rs`. This is part of the broader pattern reference maintenance
rule in `CLAUDE.md`.

---

## Test word list notes

The word list in `test_utils.rs` is intentionally small and hand-curated. It is
not a dictionary — it exists only to provide matchable words for tests. When a
new pattern type is added:
1. Write the test
2. Check whether a matching word already exists in `test_utils.rs`
3. If not, add the minimum words needed to make the test meaningful
4. Do not remove existing words — other tests may depend on them
