#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let result = patina_core::pluggable::sanitize_redirect(s);

        // Invariants:
        // 1. Must not contain angle brackets (stripped by URL-safe allowlist)
        assert!(!result.contains('<'), "unescaped < in output");
        assert!(!result.contains('>'), "unescaped > in output");

        // 2. Must not contain literal null bytes
        assert!(!result.contains('\0'), "null byte in output");

        // 3. Must not contain spaces (replaced with %20)
        assert!(!result.contains(' '), "space in output");

        // 4. Empty input → empty output
        if s.is_empty() {
            assert!(result.is_empty());
        }
    }
});
