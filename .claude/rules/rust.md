---
paths:
  - "crates/**/*.rs"
  - "fuzz/**/*.rs"
  - "Cargo.toml"
  - "crates/*/Cargo.toml"
---

# Rust Rules

## Performance is the priority

This project exists to make WordPress faster. Every implementation choice should favor throughput. Every change must be benchmarked — use `/benchmark` to prove it is faster before merging.

**Scan with SIMD, then process byte-by-byte.** Use `memchr::memchr` or `memchr::memchr3` to find the first byte needing work before entering any loop. For the common case (no special chars), this is a single SIMD pass with no allocation.

**Skip-and-copy pattern.** When no transformation is needed, return the input without allocating. When a special byte is found, allocate, copy everything before it, then switch to byte-by-byte for the rest. Return `Cow<'_, str>` from patina-core functions — `Cow::Borrowed` for the fast path, `Cow::Owned` when modified.

**Byte-level lookup tables.** Use `static TABLE: [u8; 256]` for byte classification instead of `match` expressions. Encode the action in the value (0 = passthrough, 1 = `&amp;`, 2 = `&lt;`, etc.) — one table lookup both detects and classifies. Eliminates branch mispredictions.

**Work with `&[u8]` not chars.** Use `.as_bytes()` and byte iteration. HTML escaping only affects ASCII bytes. Fall back to UTF-8-aware processing only for bytes >= 0x80 (rare in WordPress content).

**Pre-allocate outputs.** `String::with_capacity(input.len() + input.len() / 8)` for escaping. Use `.push_str()` for replacement sequences, not repeated `.push()`.

**Benchmark with throughput.** Use `Throughput::Bytes(input.len() as u64)` in Criterion so results show MB/s. Test both fast path (clean input) and slow path (every char needs work) separately.

## patina-core

- Zero PHP dependency. Must compile and test without PHP headers.
- Every public function gets a `matches_wordpress_fixtures` test using `crate::test_support::load_fixtures()`.
- Shared utilities (entities, null bytes, char tables) go in `src/util/`.
- Match WordPress quirks exactly. When unsure, generate a fixture and test against it.

## patina-ext

- All `#[php_function]` in `lib.rs` — ext-php-rs 0.15 `_internal_*` types must be visible to `wrap_function!()`.
- Every function wrapped in `panic_guard::guarded()`.
- Non-pluggable overrides need two variants: `patina_foo` (raw) and `patina_foo_filtered` (with `apply_filters` callback). Add filtered variant to `OVERRIDES` array.
- Function table swap writes `zval.value.ptr` directly. Never use `zend_hash_str_update` — it triggers destructors.
- No `unwrap()` in patina-ext. Fine in tests and patina-core.

## Style

- `cargo fmt --all` and `cargo clippy --workspace -- -D warnings` must pass.
- Do not change `panic = "unwind"` in the release profile.
