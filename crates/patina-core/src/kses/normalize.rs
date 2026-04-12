//! Entity normalization — `wp_kses_normalize_entities()`.
//!
//! Normalizes HTML entities: validates named entities, zero-pads numeric
//! entities, and converts invalid entities to `&amp;`.

use std::borrow::Cow;

use crate::util::entities;

/// Normalize HTML entities in a string.
///
/// Matches WordPress's `wp_kses_normalize_entities()`:
/// 1. Convert all `&` to `&amp;`
/// 2. Restore valid numeric entities (decimal and hex)
/// 3. Restore valid named entities from the allowlist
///
/// Returns `Cow::Borrowed` when no `&` found (common fast path).
pub fn normalize_entities(input: &str) -> Cow<'_, str> {
    let bytes = input.as_bytes();
    if !bytes.contains(&b'&') {
        return Cow::Borrowed(input);
    }

    let mut result = String::with_capacity(input.len() + input.len() / 8);
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'&' {
            let entity_len = entities::entity_len_at(bytes, i);
            if entity_len > 0 {
                entities::push_normalized_entity(&mut result, &input[i..i + entity_len]);
                i += entity_len;
            } else {
                result.push_str("&amp;");
                i += 1;
            }
        } else {
            let start = i;
            while i < bytes.len() && bytes[i] != b'&' {
                i += 1;
            }
            result.push_str(&input[start..i]);
        }
    }

    Cow::Owned(result)
}
