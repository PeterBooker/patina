//! `esc_attr()` — escapes a string for safe use in an HTML attribute.

use std::borrow::Cow;

use super::specialchars;

/// Escape a string for safe use in an HTML attribute value.
///
/// Same encoding as `esc_html` — both use `_wp_specialchars` internally.
pub fn esc_attr(text: &str) -> Cow<'_, str> {
    specialchars::wp_specialchars(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_injection() {
        assert_eq!(
            esc_attr(r#"" onclick="alert(1)"#).as_ref(),
            "&quot; onclick=&quot;alert(1)"
        );
    }

    #[test]
    fn no_double_encode() {
        assert_eq!(esc_attr("&amp;").as_ref(), "&amp;");
    }

    #[test]
    fn matches_wordpress_fixtures() {
        let fixtures = crate::test_support::load_fixtures("esc_attr");
        assert!(!fixtures.is_empty(), "no fixtures loaded");
        for (i, f) in fixtures.iter().enumerate() {
            let input = f.input[0].as_str().unwrap();
            let expected = f.output.as_str().unwrap();
            assert_eq!(
                esc_attr(input).as_ref(),
                expected,
                "fixture {i} mismatch for input: {:?}",
                &input[..input.len().min(80)]
            );
        }
    }
}
