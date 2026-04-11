//! Null byte stripping — reimplements `wp_kses_no_null()`.
//!
//! Called by wp_kses and wp_sanitize_redirect to remove null bytes
//! and related escape sequences from strings.

/// Strip null bytes and related escape sequences from a string.
///
/// Matches WordPress's `wp_kses_no_null()` behavior:
/// - Removes literal null bytes and control characters 0x00-0x08, 0x0B, 0x0C, 0x0E-0x1F
/// - Removes backslash-zero sequences (`\0`, `\\0`, `\\\0`, etc.)
///
/// Note: does NOT strip percent-encoded nulls (`%00`) — WordPress doesn't either.
pub fn strip_null_bytes(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut result = String::with_capacity(input.len());
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];

        // Strip control characters: 0x00-0x08, 0x0B, 0x0C, 0x0E-0x1F
        // (matching WordPress's preg_replace('/[\x00-\x08\x0B\x0C\x0E-\x1F]/', '', ...))
        if b <= 0x1F && b != b'\t' && b != b'\n' && b != b'\r' {
            i += 1;
            continue;
        }

        // Strip backslash+zero sequences: \0, \\0, \\\0, etc.
        // (matching WordPress's preg_replace('/\\\\+0+/', '', ...))
        if b == b'\\' {
            let start = i;
            while i < bytes.len() && bytes[i] == b'\\' {
                i += 1;
            }
            // Check if the backslashes are followed by one or more zeros
            let zeros_start = i;
            while i < bytes.len() && bytes[i] == b'0' {
                i += 1;
            }
            if i > zeros_start {
                // Had backslashes followed by zeros — strip entire sequence
                continue;
            }
            // Not followed by zeros — output the backslashes as-is
            result.push_str(&input[start..i]);
            continue;
        }

        result.push(b as char);
        i += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_nulls() {
        assert_eq!(strip_null_bytes("hello world"), "hello world");
    }

    #[test]
    fn literal_null_byte() {
        assert_eq!(strip_null_bytes("hello\0world"), "helloworld");
    }

    #[test]
    fn percent_encoded_null_preserved() {
        // WordPress does NOT strip %00 — only literal nulls and \0 sequences
        assert_eq!(strip_null_bytes("hello%00world"), "hello%00world");
    }

    #[test]
    fn backslash_zero() {
        assert_eq!(strip_null_bytes("hello\\0world"), "helloworld");
    }

    #[test]
    fn multiple_backslash_zero() {
        assert_eq!(strip_null_bytes("hello\\\\0world"), "helloworld");
        assert_eq!(strip_null_bytes("hello\\\\00world"), "helloworld");
    }

    #[test]
    fn control_chars_stripped() {
        // 0x01-0x08, 0x0B, 0x0C, 0x0E-0x1F stripped
        assert_eq!(strip_null_bytes("a\x01b\x08c"), "abc");
        assert_eq!(strip_null_bytes("a\x0Bb\x0Cc"), "abc");
    }

    #[test]
    fn tab_newline_cr_preserved() {
        // 0x09 (tab), 0x0A (newline), 0x0D (CR) are kept
        assert_eq!(strip_null_bytes("a\tb\nc\rd"), "a\tb\nc\rd");
    }

    #[test]
    fn backslash_without_zero_preserved() {
        assert_eq!(strip_null_bytes("path\\to\\file"), "path\\to\\file");
    }

    #[test]
    fn empty_string() {
        assert_eq!(strip_null_bytes(""), "");
    }
}
