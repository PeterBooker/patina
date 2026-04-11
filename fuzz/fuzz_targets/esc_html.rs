#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let result = patina_core::escaping::esc_html(s);

        // Invariants that must always hold:
        // 1. Must not contain unescaped angle brackets
        assert!(!result.contains('<'), "unescaped < in output");
        assert!(!result.contains('>'), "unescaped > in output");

        // 2. Plain text without special chars must pass through unchanged
        if !s.bytes().any(|b| matches!(b, b'&' | b'<' | b'>' | b'"' | b'\'')) {
            assert_eq!(result, s, "clean input was modified");
        }

        // 3. Empty input → empty output
        if s.is_empty() {
            assert!(result.is_empty());
        }
    }
});
