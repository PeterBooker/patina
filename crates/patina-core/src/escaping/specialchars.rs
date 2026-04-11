//! Core entity-encoding logic — reimplements WordPress's `_wp_specialchars()`.
//!
//! Shared by `esc_html` and `esc_attr`. Encodes `<`, `>`, `&`, `"`, `'`
//! while preserving existing valid HTML entities (no double-encoding).
//!
//! WordPress calls `wp_kses_normalize_entities()` before `htmlspecialchars()`
//! when `double_encode = false`. This normalizes decimal numeric entities by
//! zero-padding to at least 3 digits: `&#38;` → `&#038;`.

use std::borrow::Cow;

use crate::util::entities;

/// Byte lookup table: 0 = passthrough, nonzero = needs escaping.
/// Values encode which replacement to use.
const SPECIAL: [u8; 256] = {
    let mut table = [0u8; 256];
    table[b'&' as usize] = 1;
    table[b'<' as usize] = 2;
    table[b'>' as usize] = 3;
    table[b'"' as usize] = 4;
    table[b'\'' as usize] = 5;
    table
};

/// Replacement strings indexed by SPECIAL table value.
const REPLACEMENTS: [&str; 6] = [
    "",       // 0: unreachable
    "&amp;",  // 1: &
    "&lt;",   // 2: <
    "&gt;",   // 3: >
    "&quot;", // 4: "
    "&#039;", // 5: '
];

/// Encode special HTML characters, preserving existing valid entities.
///
/// Returns `Cow::Borrowed` when the input needs no modification (fast path),
/// `Cow::Owned` when escaping was needed.
pub fn wp_specialchars(input: &str) -> Cow<'_, str> {
    if input.is_empty() {
        return Cow::Borrowed(input);
    }

    let bytes = input.as_bytes();

    // SIMD-accelerated scan: find the first byte that needs escaping.
    // memchr is unnecessary here — the lookup table scan is already fast
    // for our typical input sizes, and we need the table for classification anyway.
    let first_special = bytes.iter().position(|&b| SPECIAL[b as usize] != 0);

    let first_special = match first_special {
        Some(pos) => pos,
        None => return Cow::Borrowed(input), // No special chars — zero allocation
    };

    // Slow path: allocate and process from the first special character
    let mut result = String::with_capacity(input.len() + input.len() / 8);

    // Copy the clean prefix (everything before the first special char)
    result.push_str(&input[..first_special]);

    let mut i = first_special;
    while i < bytes.len() {
        let action = SPECIAL[bytes[i] as usize];
        if action == 0 {
            // Scan forward for the next special byte (bulk copy)
            let start = i;
            i += 1;
            while i < bytes.len() && SPECIAL[bytes[i] as usize] == 0 {
                i += 1;
            }
            result.push_str(&input[start..i]);
        } else if action == 1 {
            // '&' — check if it's a valid entity (don't double-encode)
            let entity_len = entities::entity_len_at(bytes, i);
            if entity_len > 0 {
                push_normalized_entity(&mut result, &input[i..i + entity_len]);
                i += entity_len;
            } else {
                result.push_str("&amp;");
                i += 1;
            }
        } else {
            result.push_str(REPLACEMENTS[action as usize]);
            i += 1;
        }
    }

    Cow::Owned(result)
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
        let digits = &entity[2..entity.len() - 1];
        if digits.len() < 3 {
            result.push_str("&#");
            for _ in 0..3 - digits.len() {
                result.push('0');
            }
            result.push_str(digits);
            result.push(';');
            return;
        }
    }
    result.push_str(entity);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_encoding() {
        assert_eq!(
            wp_specialchars("<script>alert('xss')</script>").as_ref(),
            "&lt;script&gt;alert(&#039;xss&#039;)&lt;/script&gt;"
        );
    }

    #[test]
    fn preserves_existing_entities() {
        assert_eq!(
            wp_specialchars("&amp; is an ampersand").as_ref(),
            "&amp; is an ampersand"
        );
        assert_eq!(
            wp_specialchars("&lt;not a tag&gt;").as_ref(),
            "&lt;not a tag&gt;"
        );
    }

    #[test]
    fn encodes_bare_ampersand() {
        assert_eq!(wp_specialchars("foo & bar").as_ref(), "foo &amp; bar");
    }

    #[test]
    fn quotes() {
        assert_eq!(
            wp_specialchars(r#"say "hello""#).as_ref(),
            "say &quot;hello&quot;"
        );
        assert_eq!(wp_specialchars("it's").as_ref(), "it&#039;s");
    }

    #[test]
    fn multibyte_preserved() {
        assert_eq!(
            wp_specialchars("Ångström <b>bold</b>").as_ref(),
            "Ångström &lt;b&gt;bold&lt;/b&gt;"
        );
    }

    #[test]
    fn empty_string() {
        assert_eq!(wp_specialchars("").as_ref(), "");
    }

    #[test]
    fn no_special_chars() {
        let input = "just plain text with no special characters";
        assert_eq!(wp_specialchars(input).as_ref(), input);
        // Verify it's actually borrowed (no allocation)
        assert!(matches!(wp_specialchars(input), Cow::Borrowed(_)));
    }

    #[test]
    fn numeric_entities_preserved() {
        assert_eq!(wp_specialchars("&#039;").as_ref(), "&#039;");
        assert_eq!(wp_specialchars("&#x41;").as_ref(), "&#x41;");
    }

    #[test]
    fn decimal_entities_zero_padded() {
        assert_eq!(wp_specialchars("&#38;").as_ref(), "&#038;");
        assert_eq!(wp_specialchars("&#1;").as_ref(), "&#001;");
        assert_eq!(wp_specialchars("&#039;").as_ref(), "&#039;");
        assert_eq!(wp_specialchars("&#1234;").as_ref(), "&#1234;");
    }

    #[test]
    fn hex_entities_not_padded() {
        assert_eq!(wp_specialchars("&#x41;").as_ref(), "&#x41;");
        assert_eq!(wp_specialchars("&#xA;").as_ref(), "&#xA;");
    }
}
