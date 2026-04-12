//! HTML entity detection and preservation.
//!
//! Used by esc_html/esc_attr to avoid double-encoding existing entities,
//! and by kses for entity normalization.
//!
//! Named entities are validated against WordPress's `$allowedentitynames` list
//! (253 entries). Unknown named entities like `&invalid;` are NOT recognized —
//! their `&` will be encoded to `&amp;`.

/// Check if the string at position `pos` starts with a valid HTML entity.
///
/// Recognizes:
/// - Named entities from WordPress's allowlist: `&amp;`, `&lt;`, `&nbsp;`, etc.
/// - Decimal numeric: `&#123;`, `&#039;`
/// - Hex numeric: `&#x41;`, `&#xA0;`
///
/// Returns the byte length of the entity (including `&` and `;`), or 0 if
/// no valid entity starts at `pos`.
pub fn entity_len_at(s: &[u8], pos: usize) -> usize {
    if pos >= s.len() || s[pos] != b'&' {
        return 0;
    }

    let remaining = &s[pos..];
    if remaining.len() < 3 {
        return 0;
    }

    if remaining[1] == b'#' {
        // Numeric entity: &#123; or &#x41;
        numeric_entity_len(remaining)
    } else if remaining[1].is_ascii_alphabetic() {
        // Named entity: &amp; — must validate against allowlist
        named_entity_len(remaining)
    } else {
        0
    }
}

/// Length of a numeric entity (`&#123;` or `&#x4A;`) starting at `s[0]`.
fn numeric_entity_len(s: &[u8]) -> usize {
    debug_assert!(s.len() >= 3 && s[0] == b'&' && s[1] == b'#');

    let hex = s.len() > 3 && (s[2] == b'x' || s[2] == b'X');
    let digit_start = if hex { 3 } else { 2 };

    let mut i = digit_start;
    while i < s.len() && i < digit_start + 7 {
        let b = s[i];
        let valid = if hex {
            b.is_ascii_hexdigit()
        } else {
            b.is_ascii_digit()
        };
        if !valid {
            break;
        }
        i += 1;
    }

    // Must have at least one digit and end with ';'
    if i > digit_start && i < s.len() && s[i] == b';' {
        i + 1
    } else {
        0
    }
}

/// Length of a named entity (`&amp;`) starting at `s[0]`.
/// Validates the name against WordPress's allowed entity names list.
fn named_entity_len(s: &[u8]) -> usize {
    debug_assert!(s.len() >= 3 && s[0] == b'&' && s[1].is_ascii_alphabetic());

    // Extract the name (between & and ;)
    let mut i = 1;
    while i < s.len() && i < 33 {
        let b = s[i];
        if b == b';' {
            if i < 2 {
                return 0;
            }
            let name = &s[1..i];
            // Validate against WordPress's allowlist
            if is_allowed_entity_name(name) {
                return i + 1;
            }
            return 0;
        }
        if !b.is_ascii_alphanumeric() {
            return 0;
        }
        i += 1;
    }

    0
}

/// Check if a byte slice matches a WordPress allowed entity name.
/// The allowlist is sorted, so we use binary search.
fn is_allowed_entity_name(name: &[u8]) -> bool {
    ALLOWED_ENTITY_NAMES
        .binary_search_by(|probe| probe.as_bytes().cmp(name))
        .is_ok()
}

/// Push an entity to a result string, normalizing decimal numeric entities.
///
/// WordPress's `wp_kses_normalize_entities()` zero-pads decimal numeric
/// entities to at least 3 digits: `&#38;` → `&#038;`, `&#1;` → `&#001;`.
/// Named entities (`&amp;`) and hex entities (`&#x41;`) are unchanged.
pub fn push_normalized_entity(result: &mut String, entity: &str) {
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
    // Named entity, hex entity, or already 3+ digits — copy as-is
    result.push_str(entity);
}

/// WordPress's `$allowedentitynames` — 253 HTML entity names.
/// Sorted for binary search.
const ALLOWED_ENTITY_NAMES: &[&str] = &[
    "AElig", "Aacute", "Acirc", "Agrave", "Alpha", "Aring", "Atilde", "Auml", "Beta", "Ccedil",
    "Chi", "Dagger", "Delta", "ETH", "Eacute", "Ecirc", "Egrave", "Epsilon", "Eta", "Euml",
    "Gamma", "Iacute", "Icirc", "Igrave", "Iota", "Iuml", "Kappa", "Lambda", "Mu", "Ntilde", "Nu",
    "OElig", "Oacute", "Ocirc", "Ograve", "Omega", "Omicron", "Oslash", "Otilde", "Ouml", "Phi",
    "Pi", "Prime", "Psi", "Rho", "Scaron", "Sigma", "THORN", "Tau", "Theta", "Uacute", "Ucirc",
    "Ugrave", "Upsilon", "Uuml", "Xi", "Yacute", "Yuml", "Zeta", "aacute", "acirc", "acute",
    "aelig", "agrave", "alefsym", "alpha", "amp", "and", "ang", "apos", "aring", "asymp", "atilde",
    "auml", "bdquo", "beta", "brvbar", "bull", "cap", "ccedil", "cedil", "cent", "chi", "circ",
    "clubs", "cong", "copy", "crarr", "cup", "curren", "dArr", "dagger", "darr", "deg", "delta",
    "diams", "divide", "eacute", "ecirc", "egrave", "empty", "emsp", "ensp", "epsilon", "equiv",
    "eta", "eth", "euml", "euro", "exist", "fnof", "forall", "frac12", "frac14", "frac34", "frasl",
    "gamma", "ge", "gt", "hArr", "harr", "hearts", "hellip", "iacute", "icirc", "iexcl", "igrave",
    "image", "infin", "int", "iota", "iquest", "isin", "iuml", "kappa", "lArr", "lambda", "lang",
    "laquo", "larr", "lceil", "ldquo", "le", "lfloor", "lowast", "loz", "lrm", "lsaquo", "lsquo",
    "lt", "macr", "mdash", "micro", "middot", "minus", "mu", "nabla", "nbsp", "ndash", "ne", "ni",
    "not", "notin", "nsub", "ntilde", "nu", "oacute", "ocirc", "oelig", "ograve", "oline", "omega",
    "omicron", "oplus", "or", "ordf", "ordm", "oslash", "otilde", "otimes", "ouml", "para", "part",
    "permil", "perp", "phi", "pi", "piv", "plusmn", "pound", "prime", "prod", "prop", "psi",
    "quot", "rArr", "radic", "rang", "raquo", "rarr", "rceil", "rdquo", "real", "reg", "rfloor",
    "rho", "rlm", "rsaquo", "rsquo", "sbquo", "scaron", "sdot", "sect", "shy", "sigma", "sigmaf",
    "sim", "spades", "sub", "sube", "sum", "sup", "sup1", "sup2", "sup3", "supe", "szlig", "tau",
    "there4", "theta", "thetasym", "thinsp", "thorn", "tilde", "times", "trade", "uArr", "uacute",
    "uarr", "ucirc", "ugrave", "uml", "upsih", "upsilon", "uuml", "weierp", "xi", "yacute", "yen",
    "yuml", "zeta", "zwj", "zwnj",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_entities() {
        assert_eq!(entity_len_at(b"&amp;", 0), 5);
        assert_eq!(entity_len_at(b"&lt;", 0), 4);
        assert_eq!(entity_len_at(b"&gt;", 0), 4);
        assert_eq!(entity_len_at(b"&quot;", 0), 6);
        assert_eq!(entity_len_at(b"&nbsp;", 0), 6);
        assert_eq!(entity_len_at(b"&copy;", 0), 6);
    }

    #[test]
    fn invalid_named_entities() {
        // Not in WordPress's allowlist
        assert_eq!(entity_len_at(b"&invalid;", 0), 0);
        assert_eq!(entity_len_at(b"&foo;", 0), 0);
        assert_eq!(entity_len_at(b"&nosuch;", 0), 0);
        // &apos; IS in the allowlist (added in WP for XML compat)
        assert_eq!(entity_len_at(b"&apos;", 0), 6);
    }

    #[test]
    fn decimal_numeric() {
        assert_eq!(entity_len_at(b"&#039;", 0), 6);
        assert_eq!(entity_len_at(b"&#123;", 0), 6);
        assert_eq!(entity_len_at(b"&#38;", 0), 5);
    }

    #[test]
    fn hex_numeric() {
        assert_eq!(entity_len_at(b"&#x41;", 0), 6);
        assert_eq!(entity_len_at(b"&#xA0;", 0), 6);
        assert_eq!(entity_len_at(b"&#X1F;", 0), 6);
    }

    #[test]
    fn invalid_entities() {
        assert_eq!(entity_len_at(b"&;", 0), 0);
        assert_eq!(entity_len_at(b"&#;", 0), 0);
        assert_eq!(entity_len_at(b"&#x;", 0), 0);
        assert_eq!(entity_len_at(b"&123;", 0), 0);
        assert_eq!(entity_len_at(b"hello", 0), 0);
        assert_eq!(entity_len_at(b"&amp", 0), 0);
    }

    #[test]
    fn offset_position() {
        assert_eq!(entity_len_at(b"foo&amp;bar", 3), 5);
        assert_eq!(entity_len_at(b"foo&amp;bar", 0), 0);
    }

    #[test]
    fn allowlist_is_sorted() {
        for window in ALLOWED_ENTITY_NAMES.windows(2) {
            assert!(
                window[0] < window[1],
                "allowlist not sorted: {:?} >= {:?}",
                window[0],
                window[1]
            );
        }
    }
}
