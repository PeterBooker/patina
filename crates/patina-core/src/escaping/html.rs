//! `esc_html()` — escapes a string for safe use in HTML body context.

use super::specialchars;

/// Escape a string for safe use in HTML output.
///
/// Replaces WordPress's `esc_html()`. Encodes `<`, `>`, `&`, `"`, `'`
/// without double-encoding existing valid HTML entities.
pub fn esc_html(text: &str) -> String {
    specialchars::wp_specialchars(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_tag() {
        assert_eq!(
            esc_html("<script>alert(1)</script>"),
            "&lt;script&gt;alert(1)&lt;/script&gt;"
        );
    }

    #[test]
    fn no_double_encode() {
        assert_eq!(esc_html("&amp;"), "&amp;");
    }

    #[test]
    fn empty() {
        assert_eq!(esc_html(""), "");
    }

    #[test]
    fn plain_text_passthrough() {
        assert_eq!(esc_html("hello world"), "hello world");
    }

    #[test]
    fn matches_wordpress_fixtures() {
        let fixtures = crate::test_support::load_fixtures("esc_html");
        assert!(!fixtures.is_empty(), "no fixtures loaded");
        for (i, f) in fixtures.iter().enumerate() {
            let input = f.input[0].as_str().unwrap();
            let expected = f.output.as_str().unwrap();
            assert_eq!(
                esc_html(input),
                expected,
                "fixture {i} mismatch for input: {:?}",
                &input[..input.len().min(80)]
            );
        }
    }
}
