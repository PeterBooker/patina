//! `esc_html()` — escapes a string for safe use in HTML body context.

use std::borrow::Cow;

use super::specialchars;

/// Escape a string for safe use in HTML output.
///
/// Returns `Cow::Borrowed` when no escaping needed, `Cow::Owned` when modified.
pub fn esc_html(text: &str) -> Cow<'_, str> {
    specialchars::wp_specialchars(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_tag() {
        assert_eq!(
            esc_html("<script>alert(1)</script>").as_ref(),
            "&lt;script&gt;alert(1)&lt;/script&gt;"
        );
    }

    #[test]
    fn no_double_encode() {
        assert_eq!(esc_html("&amp;").as_ref(), "&amp;");
    }

    #[test]
    fn empty() {
        assert_eq!(esc_html("").as_ref(), "");
    }

    #[test]
    fn plain_text_passthrough() {
        assert_eq!(esc_html("hello world").as_ref(), "hello world");
        assert!(matches!(esc_html("hello world"), Cow::Borrowed(_)));
    }

    #[test]
    fn matches_wordpress_fixtures() {
        let fixtures = crate::test_support::load_fixtures("esc_html");
        assert!(!fixtures.is_empty(), "no fixtures loaded");
        for (i, f) in fixtures.iter().enumerate() {
            let input = f.input[0].as_str().unwrap();
            let expected = f.output.as_str().unwrap();
            assert_eq!(
                esc_html(input).as_ref(),
                expected,
                "fixture {i} mismatch for input: {:?}",
                &input[..input.len().min(80)]
            );
        }
    }
}
