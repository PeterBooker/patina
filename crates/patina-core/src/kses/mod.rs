//! HTML sanitization — the `wp_kses` family.
//!
//! Strips disallowed HTML tags and attributes based on an allowlist.

pub mod allowed_html;
pub mod normalize;
pub mod protocols;
pub mod tag_parser;

use std::sync::OnceLock;

use allowed_html::AllowedHtmlSpec;
use protocols::DEFAULT_PROTOCOLS;

/// Cached "post" preset spec (built once, reused across calls).
static POST_SPEC: OnceLock<AllowedHtmlSpec> = OnceLock::new();

fn get_post_spec() -> &'static AllowedHtmlSpec {
    POST_SPEC.get_or_init(allowed_html::build_post_spec)
}

/// Sanitize HTML content using the "post" preset (same as `wp_kses_post`).
///
/// Includes `wp_pre_kses_less_than` preprocessing, which is always active
/// in WordPress as a default `pre_kses` filter.
pub fn wp_kses_post(content: &str) -> String {
    let content = pre_kses_less_than(content);
    wp_kses(&content, get_post_spec(), DEFAULT_PROTOCOLS)
}

/// Sanitize HTML content against an allowed HTML spec.
///
/// Matches WordPress's `wp_kses()`:
/// 1. Strip control characters (wp_kses_no_null with slash_zero=keep)
/// 2. Normalize entities
/// 3. Split on HTML tokens and filter tags/attributes
pub fn wp_kses(
    content: &str,
    allowed_html: &AllowedHtmlSpec,
    allowed_protocols: &[&str],
) -> String {
    if content.is_empty() {
        return String::new();
    }

    // Step 1: Strip control characters (must run before any fast path)
    let content = strip_control_chars(content);

    // Fast path: no < or > means no tags to process.
    // Still need entity normalization.
    if !content.bytes().any(|b| b == b'<' || b == b'>') {
        return normalize::normalize_entities(&content).into_owned();
    }

    // Step 2: Normalize entities
    let content = normalize::normalize_entities(&content);

    // Step 3: Split on HTML tokens and process
    kses_split(&content, allowed_html, allowed_protocols)
}

/// WordPress's `wp_pre_kses_less_than()` — a default `pre_kses` filter.
///
/// Finds `<` followed by content that doesn't properly close with `>` (hits
/// another `<` or end of string first). Entity-encodes the malformed match.
/// This catches nested `<` inside attributes and comments containing tags.
fn pre_kses_less_than(content: &str) -> String {
    let bytes = content.as_bytes();
    if !bytes.contains(&b'<') {
        return content.to_string();
    }

    let mut result = String::with_capacity(content.len() + content.len() / 8);
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'<' {
            let start = i;
            while i < bytes.len() && bytes[i] != b'<' {
                i += 1;
            }
            result.push_str(&content[start..i]);
            continue;
        }

        // Found '<' — scan forward for '>' or another '<' or end of string
        let start = i;
        i += 1;
        let mut found_close = false;
        while i < bytes.len() {
            if bytes[i] == b'>' {
                found_close = true;
                i += 1;
                break;
            }
            if bytes[i] == b'<' {
                // Hit another '<' before closing '>' — malformed
                break;
            }
            i += 1;
        }

        if found_close {
            // Well-formed tag — pass through
            result.push_str(&content[start..i]);
        } else {
            // No closing '>' — entity-encode the match
            let segment = &content[start..i];
            let encoded = crate::escaping::specialchars::wp_specialchars(segment);
            result.push_str(&encoded);
        }
    }

    result
}

/// Basic CSS sanitization matching WordPress's `safecss_filter_attr()`.
///
/// Strips trailing semicolons from individual properties, blocks dangerous
/// CSS patterns (url(), expression(), etc.), and normalizes spacing.
pub fn safecss_filter_attr(css: &str) -> String {
    let css = css.trim();
    if css.is_empty() {
        return String::new();
    }

    let lower = css.to_lowercase();

    // Block dangerous CSS patterns entirely
    if lower.contains("expression(")
        || lower.contains("url(")
        || lower.contains("import")
        || lower.contains("binding")
    {
        return String::new();
    }

    // Split on ';', sanitize each property, rejoin
    let parts: Vec<&str> = css.split(';').collect();
    let mut cleaned = Vec::new();

    for part in &parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        // Must contain a colon (property: value)
        if !part.contains(':') {
            continue;
        }
        let lower_part = part.to_lowercase();
        // Block dangerous properties
        if lower_part.contains("expression")
            || lower_part.contains("javascript")
            || lower_part.contains("vbscript")
        {
            continue;
        }
        cleaned.push(part);
    }

    if cleaned.is_empty() {
        return String::new();
    }

    // WordPress joins with ';' between properties but no trailing ';'
    cleaned.join(";")
}

/// Strip null bytes and control chars for kses (with slash_zero = 'keep').
/// Removes 0x00-0x08, 0x0B, 0x0C, 0x0E-0x1F. Keeps tab, newline, CR.
fn strip_control_chars(input: &str) -> std::borrow::Cow<'_, str> {
    let needs_strip = input
        .bytes()
        .any(|b| matches!(b, 0x00..=0x08 | 0x0B | 0x0C | 0x0E..=0x1F));

    if !needs_strip {
        return std::borrow::Cow::Borrowed(input);
    }

    std::borrow::Cow::Owned(
        input
            .bytes()
            .filter(|b| !matches!(b, 0x00..=0x08 | 0x0B | 0x0C | 0x0E..=0x1F))
            .map(|b| b as char)
            .collect(),
    )
}

// ============================================================================
// HTML token splitting
// ============================================================================

/// Split content on HTML tokens and process each.
fn kses_split(content: &str, allowed_html: &AllowedHtmlSpec, allowed_protocols: &[&str]) -> String {
    let mut result = String::with_capacity(content.len());
    let bytes = content.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'<' => {
                let span = extract_tag(content, i);
                process_tag(span.content, allowed_html, allowed_protocols, &mut result);
                i = span.end;
            }
            b'>' => {
                result.push_str("&gt;");
                i += 1;
            }
            _ => {
                // Bulk copy text until next < or >
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i] != b'<' && bytes[i] != b'>' {
                    i += 1;
                }
                result.push_str(&content[start..i]);
            }
        }
    }

    result
}

struct TagSpan<'a> {
    content: &'a str,
    end: usize,
}

/// Extract a complete tag starting at `<`.
fn extract_tag(content: &str, start: usize) -> TagSpan<'_> {
    let rest = &content[start..];

    // HTML comment: <!-- ... -->
    if let Some(after_open) = rest.strip_prefix("<!--") {
        let end = after_open
            .find("-->")
            .map(|p| start + 4 + p + 3)
            .unwrap_or(content.len());
        return TagSpan {
            content: &content[start..end],
            end,
        };
    }

    // Bogus comment: </non-alpha...> or <!lowercase...>
    let bytes = rest.as_bytes();
    if bytes.len() >= 3 && is_bogus_comment(bytes) {
        let end = rest
            .find('>')
            .map(|p| start + p + 1)
            .unwrap_or(content.len());
        return TagSpan {
            content: &content[start..end],
            end,
        };
    }

    // Regular tag: find closing >
    match rest.find('>') {
        Some(p) => TagSpan {
            content: &content[start..start + p + 1],
            end: start + p + 1,
        },
        None => TagSpan {
            content: &content[start..],
            end: content.len(),
        },
    }
}

fn is_bogus_comment(bytes: &[u8]) -> bool {
    (bytes[1] == b'/' && bytes.get(2).is_some_and(|b| !b.is_ascii_alphabetic()))
        || (bytes[1] == b'!' && bytes.get(2).is_some_and(|b| b.is_ascii_lowercase()))
}

// ============================================================================
// Tag processing (matches wp_kses_split2)
// ============================================================================

/// Process a single HTML token, writing the result into `out`.
fn process_tag(
    content: &str,
    allowed_html: &AllowedHtmlSpec,
    allowed_protocols: &[&str],
    out: &mut String,
) {
    if !content.starts_with('<') {
        out.push_str("&gt;");
        return;
    }

    if content.starts_with("<!--") {
        process_comment(content, allowed_html, allowed_protocols, out);
        return;
    }

    if content.len() >= 3 && is_bogus_comment(content.as_bytes()) {
        process_bogus_comment(content, allowed_html, allowed_protocols, out);
        return;
    }

    process_normal_tag(content, allowed_html, allowed_protocols, out);
}

/// Process an HTML comment: <!-- ... -->
fn process_comment(
    content: &str,
    allowed_html: &AllowedHtmlSpec,
    allowed_protocols: &[&str],
    out: &mut String,
) {
    let inner = content
        .strip_prefix("<!--")
        .and_then(|s| s.strip_suffix("-->"))
        .unwrap_or(&content[4..]);

    let mut cleaned = wp_kses(inner, allowed_html, allowed_protocols);

    // Prevent multiple dashes and trailing dash
    while cleaned.contains("--") {
        cleaned = cleaned.replace("--", "-");
    }
    if cleaned.ends_with('-') {
        cleaned.pop();
    }

    if !cleaned.is_empty() {
        out.push_str("<!--");
        out.push_str(&cleaned);
        out.push_str("-->");
    }
}

/// Process a bogus comment: </non-alpha...> or <!lowercase...>
fn process_bogus_comment(
    content: &str,
    allowed_html: &AllowedHtmlSpec,
    allowed_protocols: &[&str],
    out: &mut String,
) {
    let opener = content.as_bytes()[1] as char;
    let inner = &content[2..content.len().saturating_sub(1)];

    let mut cleaned = wp_kses(inner, allowed_html, allowed_protocols);
    loop {
        let next = wp_kses(&cleaned, allowed_html, allowed_protocols);
        if next == cleaned {
            break;
        }
        cleaned = next;
    }

    out.push('<');
    out.push(opener);
    out.push_str(&cleaned);
    out.push('>');
}

/// Process a normal HTML tag: <tagname attrs> or </tagname>
fn process_normal_tag(
    content: &str,
    allowed_html: &AllowedHtmlSpec,
    allowed_protocols: &[&str],
    out: &mut String,
) {
    let tag_inner = &content[1..]; // Strip leading <

    // Tags without closing > are malformed — entity-encode the whole thing
    if !tag_inner.ends_with('>') {
        let encoded = crate::escaping::specialchars::wp_specialchars(content);
        out.push_str(&encoded);
        return;
    }
    let tag_inner = &tag_inner[..tag_inner.len() - 1]; // Strip trailing >

    let trimmed = tag_inner.trim_start();
    let (is_closing, rest) = match trimmed.strip_prefix('/') {
        Some(r) => (true, r.trim_start()),
        None => (false, trimmed),
    };

    let name_end = rest
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .unwrap_or(rest.len());

    if name_end == 0 {
        return; // No tag name — seriously malformed
    }

    let elem = &rest[..name_end];
    let elem_lower = elem.to_lowercase();

    if !allowed_html.is_tag_allowed(&elem_lower) {
        return; // Disallowed tag — strip entirely
    }

    if is_closing {
        out.push_str("</");
        out.push_str(elem);
        out.push('>');
        return;
    }

    let attrlist = &rest[name_end..];
    let xhtml_slash = if attrlist.trim_end().ends_with('/') {
        " /"
    } else {
        ""
    };

    let tag_spec = match allowed_html.get_tag(&elem_lower) {
        Some(spec) if !spec.attrs.is_empty() || spec.allow_data_attrs => spec,
        _ => {
            out.push('<');
            out.push_str(elem);
            out.push_str(xhtml_slash);
            out.push('>');
            return;
        }
    };

    // Parse and filter attributes
    let is_uri = |name: &str| protocols::is_uri_attribute(name);
    let check_proto = |value: &str| protocols::check_url_protocol(value, allowed_protocols);
    let parsed_attrs = tag_parser::parse_attributes(attrlist, &is_uri, &check_proto);

    out.push('<');
    out.push_str(elem);

    for attr in &parsed_attrs {
        let attr_lower = attr.name.to_lowercase();
        if !tag_spec.is_attr_allowed(&attr_lower) {
            continue;
        }

        // Style attributes get CSS sanitization
        if attr_lower == "style" {
            if let Some(value) = extract_attr_value(&attr.whole) {
                let sanitized = safecss_filter_attr(value);
                if sanitized.is_empty() {
                    continue; // Dangerous CSS — strip the attribute
                }
                out.push(' ');
                out.push_str(&format!("{}=\"{}\"", attr.name, sanitized));
                continue;
            }
        }

        out.push(' ');
        if attr.whole.contains('<') || attr.whole.contains('>') {
            out.push_str(&attr.whole.replace(['<', '>'], ""));
        } else {
            out.push_str(&attr.whole);
        }
    }

    out.push_str(xhtml_slash);
    out.push('>');
}

/// Extract the value from a reconstructed attribute string like `name="value"`.
fn extract_attr_value(whole: &str) -> Option<&str> {
    let eq = whole.find('=')?;
    let rest = &whole[eq + 1..];
    if rest.len() >= 2
        && ((rest.starts_with('"') && rest.ends_with('"'))
            || (rest.starts_with('\'') && rest.ends_with('\'')))
    {
        Some(&rest[1..rest.len() - 1])
    } else {
        Some(rest)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_script_tags() {
        assert_eq!(wp_kses_post("<script>alert(1)</script>"), "alert(1)");
    }

    #[test]
    fn preserves_allowed_tags() {
        assert_eq!(wp_kses_post("<b>bold</b>"), "<b>bold</b>");
        assert_eq!(wp_kses_post("<em>italic</em>"), "<em>italic</em>");
    }

    #[test]
    fn filters_disallowed_attributes() {
        assert_eq!(
            wp_kses_post(r#"<a href="http://example.com" onclick="alert(1)">link</a>"#),
            r#"<a href="http://example.com">link</a>"#
        );
    }

    #[test]
    fn strips_javascript_protocol() {
        let result = wp_kses_post(r#"<a href="javascript:alert(1)">xss</a>"#);
        assert!(!result.contains("javascript"));
    }

    #[test]
    fn plain_text_passthrough() {
        assert_eq!(wp_kses_post("Just plain text"), "Just plain text");
    }

    #[test]
    fn empty_string() {
        assert_eq!(wp_kses_post(""), "");
    }

    #[test]
    fn preserves_entities() {
        let result = wp_kses_post("<p>&amp; &lt; &gt;</p>");
        assert!(result.contains("&amp;"));
    }

    #[test]
    fn matches_wordpress_fixtures() {
        let fixtures = crate::test_support::load_fixtures("wp_kses_post");
        assert!(!fixtures.is_empty(), "no fixtures loaded");

        let mut mismatches = Vec::new();
        for (i, f) in fixtures.iter().enumerate() {
            let input = f.input[0].as_str().unwrap();
            let expected = f.output.as_str().unwrap();
            let actual = wp_kses_post(input);
            if actual != expected {
                mismatches.push(i);
            }
        }

        for idx in &mismatches {
            let input = fixtures[*idx].input[0].as_str().unwrap();
            let expected = fixtures[*idx].output.as_str().unwrap();
            let actual = wp_kses_post(input);
            eprintln!(
                "MISMATCH fixture {idx}:\n  Input:    {:?}\n  Expected: {:?}\n  Got:      {:?}\n",
                &input[..input.len().min(100)],
                &expected[..expected.len().min(100)],
                &actual[..actual.len().min(100)]
            );
        }
        assert!(
            mismatches.is_empty(),
            "{} fixture mismatches",
            mismatches.len()
        );
    }
}
