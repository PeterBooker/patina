//! Protocol allowlist validation for kses.
//!
//! Checks that URLs in attributes use allowed protocols.

use std::borrow::Cow;

/// WordPress's default allowed protocols.
pub const DEFAULT_PROTOCOLS: &[&str] = &[
    "http", "https", "ftp", "ftps", "mailto", "news", "irc", "irc6", "ircs", "gopher", "nntp",
    "feed", "telnet", "mms", "rtsp", "sms", "svn", "tel", "fax", "xmpp", "webcal", "urn",
];

/// WordPress attributes that contain URIs (must be protocol-checked).
/// Sorted for binary search.
const URI_ATTRIBUTES: &[&str] = &[
    "action",
    "archive",
    "background",
    "cite",
    "classid",
    "codebase",
    "data",
    "formaction",
    "href",
    "icon",
    "longdesc",
    "manifest",
    "poster",
    "profile",
    "src",
    "srcset",
    "usemap",
];

/// Check if an attribute name is a URI attribute that needs protocol checking.
pub fn is_uri_attribute(attr: &str) -> bool {
    // Input may be mixed case; the table is lowercase.
    let lower = attr.to_lowercase();
    URI_ATTRIBUTES.binary_search(&lower.as_str()).is_ok()
}

/// Check if a URL value uses an allowed protocol.
///
/// Matches WordPress's `wp_kses_bad_protocol()` behavior:
/// strips control chars, extracts protocol, checks against allowlist.
pub fn check_url_protocol(url: &str, allowed_protocols: &[&str]) -> String {
    let cleaned = strip_control_chars(url);

    if let Some(colon_pos) = cleaned.find(':') {
        let protocol = cleaned[..colon_pos].trim().to_lowercase();

        if allowed_protocols
            .iter()
            .any(|p| p.eq_ignore_ascii_case(&protocol))
        {
            return cleaned.into_owned();
        }

        // Disallowed protocol — return the part after the colon
        return cleaned[colon_pos + 1..].to_string();
    }

    // No protocol found — safe, return as-is
    cleaned.into_owned()
}

/// Strip ASCII control chars (except tab) from a URL.
/// Returns Cow::Borrowed when no stripping was needed.
fn strip_control_chars(url: &str) -> Cow<'_, str> {
    if !url.bytes().any(|b| b.is_ascii_control() && b != b'\t') {
        return Cow::Borrowed(url);
    }
    Cow::Owned(
        url.chars()
            .filter(|&c| !c.is_ascii_control() || c == '\t')
            .collect(),
    )
}
