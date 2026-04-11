//! `esc_attr()` — escapes a string for safe use in an HTML attribute.

use super::specialchars;

/// Escape a string for safe use in an HTML attribute value.
///
/// Replaces WordPress's `esc_attr()`. Same encoding as `esc_html()` —
/// both use `_wp_specialchars()` internally. The difference in WordPress
/// is which filter hook fires on the result; the encoding is identical.
pub fn esc_attr(text: &str) -> String {
    specialchars::wp_specialchars(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_injection() {
        assert_eq!(
            esc_attr(r#"" onclick="alert(1)"#),
            "&quot; onclick=&quot;alert(1)"
        );
    }

    #[test]
    fn no_double_encode() {
        assert_eq!(esc_attr("&amp;"), "&amp;");
    }

    #[test]
    fn matches_wordpress_fixtures() {
        let fixtures = crate::test_support::load_fixtures("esc_attr");
        assert!(!fixtures.is_empty(), "no fixtures loaded");
        for (i, f) in fixtures.iter().enumerate() {
            let input = f.input[0].as_str().unwrap();
            let expected = f.output.as_str().unwrap();
            assert_eq!(
                esc_attr(input),
                expected,
                "fixture {i} mismatch for input: {:?}",
                &input[..input.len().min(80)]
            );
        }
    }
}
