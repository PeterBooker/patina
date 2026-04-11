//! `wp_sanitize_redirect()` — sanitizes a URL for use in a redirect.
//!
//! Pure string processing, no WordPress state dependencies.

use crate::util::byte_class;

/// Hex encoding lookup table — avoids branch in the hot loop.
const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";

/// Sanitize a URL for use in a redirect.
///
/// Replaces WordPress's `wp_sanitize_redirect()` from `pluggable.php`.
///
/// Single-pass: handles space encoding, multibyte percent-encoding,
/// and disallowed character stripping together.
///
/// WordPress runs `wp_kses_no_null()` after the allowlist filter, but all
/// characters it would strip (null bytes, control chars, backslash sequences)
/// are already not URL-safe, so the allowlist filter handles them.
pub fn sanitize_redirect(location: &str) -> String {
    let bytes = location.as_bytes();
    let mut result = String::with_capacity(location.len() + location.len() / 4);
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];

        if b == b' ' {
            // Spaces → %20
            result.push_str("%20");
            i += 1;
        } else if b >= 0x80 {
            // Multibyte UTF-8: percent-encode the full character
            let ch_len = utf8_char_len(b);
            let end = (i + ch_len).min(bytes.len());
            for &byte in &bytes[i..end] {
                result.push('%');
                result.push(HEX_UPPER[(byte >> 4) as usize] as char);
                result.push(HEX_UPPER[(byte & 0x0F) as usize] as char);
            }
            i = end;
        } else if byte_class::URL_SAFE_REDIRECT[b as usize] {
            // ASCII URL-safe character — pass through
            result.push(b as char);
            i += 1;
        } else {
            // Disallowed ASCII character — strip
            i += 1;
        }
    }

    result
}

/// Determine UTF-8 character length from the first byte.
#[inline]
fn utf8_char_len(first_byte: u8) -> usize {
    match first_byte {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xFF => 4,
        _ => 1,
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
