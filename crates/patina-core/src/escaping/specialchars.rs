//! Core entity-encoding logic — reimplements WordPress's `_wp_specialchars()`.
//!
//! Shared by `esc_html` and `esc_attr`. Encodes `<`, `>`, `&`, `"`, `'`
//! while preserving existing valid HTML entities (no double-encoding).
//!
//! WordPress calls `wp_kses_normalize_entities()` before `htmlspecialchars()`
//! when `double_encode = false`. This normalizes decimal numeric entities by
//! zero-padding to at least 3 digits: `&#38;` → `&#038;`.

use crate::util::entities;

/// Encode special HTML characters, preserving existing valid entities.
///
/// Reimplements WordPress's `_wp_specialchars($string, ENT_QUOTES, false, false)`:
/// 1. Normalize entities (zero-pad decimal numeric entities to 3+ digits)
/// 2. Encode `<`, `>`, `&`, `"`, `'` without double-encoding valid entities
pub fn wp_specialchars(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }

    // Fast path: if no special chars exist, return as-is (matches WordPress's
    // preg_match('/[&<>"\']/', $text) early exit in _wp_specialchars)
    if !input
        .bytes()
        .any(|b| matches!(b, b'&' | b'<' | b'>' | b'"' | b'\''))
    {
        return input.to_string();
    }

    let bytes = input.as_bytes();
    let mut result = String::with_capacity(input.len() + input.len() / 8);
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'&' => {
                // Check if this starts a valid entity — if so, preserve it
                let entity_len = entities::entity_len_at(bytes, i);
                if entity_len > 0 {
                    // Valid entity — normalize and preserve
                    let entity = &input[i..i + entity_len];
                    push_normalized_entity(&mut result, entity);
                    i += entity_len;
                } else {
                    result.push_str("&amp;");
                    i += 1;
                }
            }
            b'<' => {
                result.push_str("&lt;");
                i += 1;
            }
            b'>' => {
                result.push_str("&gt;");
                i += 1;
            }
            b'"' => {
                result.push_str("&quot;");
                i += 1;
            }
            b'\'' => {
                result.push_str("&#039;");
                i += 1;
            }
            _ => {
                // Fast path: scan forward for the next special character
                let start = i;
                i += 1;
                while i < bytes.len() && !matches!(bytes[i], b'&' | b'<' | b'>' | b'"' | b'\'') {
                    i += 1;
                }
                result.push_str(&input[start..i]);
            }
        }
    }

    result
}

/// Push an entity to the result, normalizing decimal numeric entities.
///
/// WordPress's `wp_kses_normalize_entities()` zero-pads decimal numeric
/// entities to at least 3 digits: `&#38;` → `&#038;`, `&#1;` → `&#001;`.
/// Named entities (`&amp;`) and hex entities (`&#x41;`) are unchanged.
fn push_normalized_entity(result: &mut String, entity: &str) {
    let bytes = entity.as_bytes();
    // Check for decimal numeric entity: &#NNN;
    if bytes.len() >= 4 && bytes[1] == b'#' && bytes[2] != b'x' && bytes[2] != b'X' {
        // Extract the decimal digits between &# and ;
        let digits = &entity[2..entity.len() - 1];
        if digits.len() < 3 {
            // Zero-pad to at least 3 digits
            result.push_str("&#");
            for _ in 0..3 - digits.len() {
                result.push('0');
            }
            result.push_str(digits);
            result.push(';');
            return;
        }
    }
    // Named entity or hex entity or already 3+ digits — copy as-is
    result.push_str(entity);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_encoding() {
        assert_eq!(
            wp_specialchars("<script>alert('xss')</script>"),
            "&lt;script&gt;alert(&#039;xss&#039;)&lt;/script&gt;"
        );
    }

    #[test]
    fn preserves_existing_entities() {
        assert_eq!(
            wp_specialchars("&amp; is an ampersand"),
            "&amp; is an ampersand"
        );
        assert_eq!(wp_specialchars("&lt;not a tag&gt;"), "&lt;not a tag&gt;");
    }

    #[test]
    fn encodes_bare_ampersand() {
        assert_eq!(wp_specialchars("foo & bar"), "foo &amp; bar");
    }

    #[test]
    fn quotes() {
        assert_eq!(wp_specialchars(r#"say "hello""#), "say &quot;hello&quot;");
        assert_eq!(wp_specialchars("it's"), "it&#039;s");
    }

    #[test]
    fn multibyte_preserved() {
        assert_eq!(
            wp_specialchars("Ångström <b>bold</b>"),
            "Ångström &lt;b&gt;bold&lt;/b&gt;"
        );
    }

    #[test]
    fn empty_string() {
        assert_eq!(wp_specialchars(""), "");
    }

    #[test]
    fn no_special_chars() {
        let input = "just plain text with no special characters";
        assert_eq!(wp_specialchars(input), input);
    }

    #[test]
    fn numeric_entities_preserved() {
        assert_eq!(wp_specialchars("&#039;"), "&#039;");
        assert_eq!(wp_specialchars("&#x41;"), "&#x41;");
    }

    #[test]
    fn decimal_entities_zero_padded() {
        // WordPress's wp_kses_normalize_entities zero-pads to 3+ digits
        assert_eq!(wp_specialchars("&#38;"), "&#038;");
        assert_eq!(wp_specialchars("&#1;"), "&#001;");
        assert_eq!(wp_specialchars("&#039;"), "&#039;"); // already 3 digits
        assert_eq!(wp_specialchars("&#1234;"), "&#1234;"); // >3 digits, unchanged
    }

    #[test]
    fn hex_entities_not_padded() {
        assert_eq!(wp_specialchars("&#x41;"), "&#x41;");
        assert_eq!(wp_specialchars("&#xA;"), "&#xA;");
    }
}
