# Adding a New Function

This guide walks through adding a new WordPress function replacement to Patina.

## Prerequisites

- Rust stable toolchain
- PHP with development headers
- Docker (for fixture generation)

## Steps

### 1. Study the WordPress source

Read the function in WordPress core. Document every operation, edge case, and filter hook. Pay attention to:
- Does it call other WordPress functions? (those may need Rust implementations too)
- Does it use `apply_filters`? (needs PHP callback from Rust)
- Is it pluggable? (`function_exists()` guard in `pluggable.php`)
- What types does it accept and return?

### 2. Generate fixtures

Add a fixture definition:

```php
// php/fixture-generator/functions/your_function.php
<?php
return [
    'name' => 'your_function',
    'callable' => 'your_function',
    'inputs' => array_merge(
        corpus_strings(),     // from corpus/strings.php
        corpus_html(),        // if HTML-related
        corpus_urls(),        // if URL-related
        [
            // Function-specific edge cases
            'specific_input_1',
            'specific_input_2',
        ]
    ),
];
```

Generate:
```bash
cd profiling && docker compose up -d
./setup-wordpress.sh  # if not already done
docker compose exec php-fpm php -d memory_limit=512M \
    /app/php/fixture-generator/generate.php --function=your_function
```

### 3. Implement in `patina-core`

Add a source file in the appropriate module:

```
crates/patina-core/src/
├── escaping/your_function.rs   # if it's an escaping function
├── sanitize/your_function.rs   # if it's a sanitizer
├── formatting/your_function.rs # if it's a formatter
└── pluggable/your_function.rs  # if it's from pluggable.php
```

The implementation must:
- Take `&str` and return `String`
- Be pure Rust — no PHP types, no ext-php-rs
- Include `#[cfg(test)]` unit tests
- Include a `matches_wordpress_fixtures` test

```rust
pub fn your_function(input: &str) -> String {
    // Implementation
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_wordpress_fixtures() {
        let fixtures = crate::test_support::load_fixtures("your_function");
        assert!(!fixtures.is_empty(), "no fixtures loaded");
        for (i, f) in fixtures.iter().enumerate() {
            let input = f.input[0].as_str().unwrap();
            let expected = f.output.as_str().unwrap();
            assert_eq!(
                your_function(input), expected,
                "fixture {i} mismatch for input: {:?}",
                &input[..input.len().min(80)]
            );
        }
    }
}
```

Register the module in the parent `mod.rs` and re-export.

### 4. Register in `patina-ext`

Add a `#[php_function]` in `crates/patina-ext/src/lib.rs`:

```rust
// For pluggable functions (original WordPress name):
#[php_function]
pub fn your_function(input: &str) -> PhpResult<String> {
    panic_guard::guarded("your_function", || {
        patina_core::module::your_function(input)
    })
}

// For non-pluggable functions (patina_ prefix):
#[php_function]
pub fn patina_your_function(input: &str) -> PhpResult<String> {
    panic_guard::guarded("patina_your_function", || {
        patina_core::module::your_function(input)
    })
}
```

Add to the `get_module` builder:
```rust
.function(wrap_function!(your_function))
```

### 5. Add PHP tests

```php
// php/tests/Module/YourFunctionTest.php
class YourFunctionTest extends FixtureTestCase {
    public static function fixtureProvider(): array {
        return static::fixturesAsProvider('your_function');
    }

    /** @dataProvider fixtureProvider */
    public function test_matches_wordpress_output(array $input, mixed $expected): void {
        $result = your_function($input[0]);
        $this->assertSame($expected, $result);
    }

    public function test_fuzz_no_crash(): void {
        // Generate random valid UTF-8 and call the function 1000 times
    }
}
```

### 6. Add fuzz target

```rust
// fuzz/fuzz_targets/your_function.rs
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = patina_core::module::your_function(s);
    }
});
```

Add the `[[bin]]` entry to `fuzz/Cargo.toml`.

### 7. Add benchmarks

Add to the appropriate Criterion bench file in `crates/patina-bench/benches/`.
Add to the PHP benchmark suite in `php/benchmarks/`.

### 8. Verify

```bash
cargo test --workspace                    # All Rust tests pass
cargo clippy --workspace -- -D warnings   # No warnings
cargo fmt --all --check                   # Formatted

# PHP tests
cargo build --release -p patina-ext
php -d extension=target/release/libpatina.so php/vendor/bin/phpunit --configuration php/phpunit.xml
```

## Checklist

- [ ] WordPress source studied and edge cases documented
- [ ] Fixtures generated from WordPress (committed to `fixtures/`)
- [ ] Pure Rust implementation in `patina-core`
- [ ] Fixture-based Rust tests passing
- [ ] `#[php_function]` registered in `patina-ext/src/lib.rs`
- [ ] PHP test class written
- [ ] Fuzz target added
- [ ] Criterion benchmark added
- [ ] PHP benchmark added
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] PHP tests pass
