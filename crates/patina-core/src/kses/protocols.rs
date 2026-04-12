//! Protocol allowlist validation for kses.
//!
//! Checks that URLs in attributes use allowed protocols.

use std::borrow::Cow;

/// WordPress's default allowed protocols.
pub const DEFAULT_PROTOCOLS: &[&str] = &[
    "http", "https", "ftp", "ftps", "mailto", "news", "irc", "irc6", "ircs", "gopher", "nntp",
    "feed", "telnet", "mms", "rtsp", "sms", "svn", "tel", "fax", "xmpp", "webcal", "urn",
];

/// WordPress's default URI-bearing attribute names. Matches the hardcoded
/// list in `wp_kses_uri_attributes()` before the filter is applied.
pub const DEFAULT_URI_ATTRIBUTES: &[&str] = &[
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
/// `uri_attrs` is expected to contain lowercase names.
pub fn is_uri_attribute(attr: &str, uri_attrs: &[&str]) -> bool {
    uri_attrs.iter().any(|u| u.eq_ignore_ascii_case(attr))
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
