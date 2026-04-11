//! `wp_sanitize_redirect()` — sanitizes a URL for use in a redirect.
//!
//! This is the first function implemented as a proof of concept (Phase 4).
//! Pure string processing, no WordPress state dependencies.

use crate::util::{byte_class, null_bytes};

/// Sanitize a URL for use in a redirect.
///
/// Replaces WordPress's `wp_sanitize_redirect()` from `pluggable.php`.
///
/// Steps (matching WordPress behavior):
/// 1. Replace spaces with `%20`
/// 2. Percent-encode multibyte UTF-8 characters
/// 3. Strip characters not in the URL-safe allowlist
/// 4. Strip null bytes (wp_kses_no_null behavior)
pub fn sanitize_redirect(location: &str) -> String {
    // Step 1: Replace spaces with %20
    let mut result = String::with_capacity(location.len() + location.len() / 4);

    for ch in location.chars() {
        match ch {
            // Step 1: spaces → %20
            ' ' => result.push_str("%20"),

            // Step 2: multibyte chars → percent-encoded UTF-8 bytes
            c if c.len_utf8() > 1 => {
                let mut buf = [0u8; 4];
                let encoded = c.encode_utf8(&mut buf);
                for byte in encoded.as_bytes() {
                    result.push('%');
                    result.push(hex_char(byte >> 4));
                    result.push(hex_char(byte & 0x0F));
                }
            }

            // ASCII characters — will be filtered by allowlist in step 3
            c => result.push(c),
        }
    }

    // Step 3: Strip characters not in the URL-safe allowlist
    let filtered: String = result
        .bytes()
        .filter(|&b| byte_class::URL_SAFE_REDIRECT[b as usize])
        .map(|b| b as char)
        .collect();

    // Step 4: Strip null bytes
    null_bytes::strip_null_bytes(&filtered)
}

/// Convert a nibble (0-15) to its uppercase hex character.
fn hex_char(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'A' + nibble - 10) as char,
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_url_passthrough() {
        assert_eq!(
            sanitize_redirect("http://example.com/page"),
            "http://example.com/page"
        );
    }

    #[test]
    fn spaces_to_percent20() {
        assert_eq!(
            sanitize_redirect("http://example.com/my page"),
            "http://example.com/my%20page"
        );
    }

    #[test]
    fn multibyte_percent_encoded() {
        let result = sanitize_redirect("http://example.com/日本");
        // Each CJK character is 3 UTF-8 bytes → 3 percent-encoded sequences
        assert!(result.contains("%E6%97%A5")); // 日
        assert!(result.contains("%E6%9C%AC")); // 本
        assert!(!result.contains("日"));
    }

    #[test]
    fn strips_disallowed_chars() {
        let result = sanitize_redirect("http://example.com/<script>alert(1)</script>");
        assert!(!result.contains('<'));
        assert!(!result.contains('>'));
    }

    #[test]
    fn strips_null_bytes() {
        assert_eq!(
            sanitize_redirect("http://example.com/\0page"),
            "http://example.com/page"
        );
        // %00 is NOT stripped — WordPress preserves percent-encoded nulls
        assert_eq!(
            sanitize_redirect("http://example.com/%00page"),
            "http://example.com/%00page"
        );
    }

    #[test]
    fn empty_string() {
        assert_eq!(sanitize_redirect(""), "");
    }

    #[test]
    fn preserves_query_and_fragment() {
        assert_eq!(
            sanitize_redirect("http://example.com/path?key=value&other=1#section"),
            "http://example.com/path?key=value&other=1#section"
        );
    }

    #[test]
    fn matches_wordpress_fixtures() {
        let fixtures = crate::test_support::load_fixtures("wp_sanitize_redirect");
        assert!(!fixtures.is_empty(), "no fixtures loaded");
        for (i, f) in fixtures.iter().enumerate() {
            let input = f.input[0].as_str().unwrap();
            let expected = f.output.as_str().unwrap();
            assert_eq!(
                sanitize_redirect(input),
                expected,
                "fixture {i} mismatch for input: {:?}",
                &input[..input.len().min(80)]
            );
        }
    }
}
