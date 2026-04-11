//! Const lookup tables for fast byte-level character classification.

/// URL-safe characters per WordPress's `wp_sanitize_redirect()`.
///
/// Matches the regex class `[a-z0-9-~+_.?#=&;,/:%!*\[\]()@]` (case-insensitive).
pub const URL_SAFE_REDIRECT: [bool; 256] = {
    let mut table = [false; 256];
    let mut i = 0u16;
    while i < 256 {
        let b = i as u8;
        table[i as usize] = matches!(b,
            b'a'..=b'z'
            | b'A'..=b'Z'
            | b'0'..=b'9'
            | b'-' | b'~' | b'+' | b'_' | b'.' | b'?'
            | b'#' | b'=' | b'&' | b';' | b',' | b'/'
            | b':' | b'%' | b'!' | b'*' | b'[' | b']'
            | b'(' | b')' | b'@'
        );
        i += 1;
    }
    table
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_letters_are_safe() {
        for b in b'a'..=b'z' {
            assert!(URL_SAFE_REDIRECT[b as usize], "expected {b} to be safe");
        }
        for b in b'A'..=b'Z' {
            assert!(URL_SAFE_REDIRECT[b as usize], "expected {b} to be safe");
        }
    }

    #[test]
    fn digits_are_safe() {
        for b in b'0'..=b'9' {
            assert!(URL_SAFE_REDIRECT[b as usize], "expected {b} to be safe");
        }
    }

    #[test]
    fn special_chars_are_safe() {
        for b in b"-~+_.?#=&;,/:%!*[]()@" {
            assert!(
                URL_SAFE_REDIRECT[*b as usize],
                "expected {:?} to be safe",
                *b as char
            );
        }
    }

    #[test]
    fn angle_brackets_are_not_safe() {
        assert!(!URL_SAFE_REDIRECT[b'<' as usize]);
        assert!(!URL_SAFE_REDIRECT[b'>' as usize]);
    }

    #[test]
    fn null_and_space_are_not_safe() {
        assert!(!URL_SAFE_REDIRECT[0]);
        assert!(!URL_SAFE_REDIRECT[b' ' as usize]);
    }
}
