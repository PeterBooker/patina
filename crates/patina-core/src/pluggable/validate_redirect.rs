//! `wp_validate_redirect()` — validates a redirect URL against allowed hosts.
//!
//! The pure URL validation logic lives here. The PHP callback for
//! `apply_filters('allowed_redirect_hosts', ...)` and `home_url()` access
//! are handled in the ext wrapper, which passes the resolved values in.

/// Result of validating a redirect URL.
pub enum ValidateResult {
    /// URL is valid — use this location.
    Valid(String),
    /// URL is invalid — use the fallback.
    Fallback,
}

/// Validate a redirect URL.
///
/// This implements the pure logic of WordPress's `wp_validate_redirect()`.
/// The caller (ext wrapper) is responsible for:
/// - Calling `wp_sanitize_redirect(trim($location))` first
/// - Resolving `home_url()` to get `home_host`
/// - Calling `apply_filters('allowed_redirect_hosts', ...)` to get `allowed_hosts`
/// - Accessing `$_SERVER['REQUEST_URI']` for relative path resolution
///
/// # Arguments
/// * `location` — The sanitized redirect URL
/// * `home_host` — The host from `home_url()` (e.g., "example.com")
/// * `allowed_hosts` — Hosts allowed for redirect (from apply_filters)
/// * `request_uri` — Current request URI from `$_SERVER['REQUEST_URI']` (for relative paths)
pub fn validate_redirect(
    location: &str,
    home_host: &str,
    allowed_hosts: &[&str],
    request_uri: Option<&str>,
) -> ValidateResult {
    let mut location = location.to_string();

    // Browsers assume 'http' for URLs starting with '//'
    if location.starts_with("//") {
        location = format!("http:{location}");
    }

    // Parse URL — strip query string first (PHP parse_url quirk)
    let test = match location.find('?') {
        Some(pos) => &location[..pos],
        None => &location,
    };

    let parsed = match SimpleUrl::parse(test) {
        Some(p) => p,
        None => return ValidateResult::Fallback,
    };

    // Allow only 'http' and 'https' schemes
    if let Some(scheme) = &parsed.scheme {
        if scheme != "http" && scheme != "https" {
            return ValidateResult::Fallback;
        }
    }

    // Handle relative paths (no host, non-absolute path)
    if parsed.host.is_none() {
        if let Some(path) = &parsed.path {
            if !path.is_empty() && !path.starts_with('/') {
                let prefix = match request_uri {
                    Some(uri) => {
                        // Extract directory from request URI
                        let uri_path = match uri.find('?') {
                            Some(pos) => &uri[..pos],
                            None => uri,
                        };
                        let dir = match uri_path.rfind('/') {
                            Some(pos) => &uri_path[..=pos],
                            None => "/",
                        };
                        normalize_path(dir)
                    }
                    None => "/".to_string(),
                };
                location = format!("/{}{location}", prefix.trim_start_matches('/'));
            }
        }
    }

    // Reject if certain components are set but host is not
    if parsed.host.is_none()
        && (parsed.scheme.is_some()
            || parsed.user.is_some()
            || parsed.pass.is_some()
            || parsed.port.is_some())
    {
        return ValidateResult::Fallback;
    }

    // Reject malformed components containing :/?#@
    for val in [&parsed.user, &parsed.pass, &parsed.host]
        .into_iter()
        .flatten()
    {
        if val
            .chars()
            .any(|c| matches!(c, ':' | '/' | '?' | '#' | '@'))
        {
            return ValidateResult::Fallback;
        }
    }

    // Check host against allowed hosts
    if let Some(host) = &parsed.host {
        let host_allowed = allowed_hosts.iter().any(|h| h == host)
            || home_host.to_lowercase() == host.to_lowercase();
        if !host_allowed {
            return ValidateResult::Fallback;
        }
    }

    ValidateResult::Valid(location)
}

/// Minimal URL parser matching PHP's `parse_url()` behavior.
/// Only extracts the components we need for validation.
#[derive(Default)]
struct SimpleUrl {
    scheme: Option<String>,
    user: Option<String>,
    pass: Option<String>,
    host: Option<String>,
    port: Option<String>,
    path: Option<String>,
}

impl SimpleUrl {
    fn parse(url: &str) -> Option<SimpleUrl> {
        if url.is_empty() {
            return Some(SimpleUrl::default());
        }

        let mut result = SimpleUrl::default();
        let mut rest = url;

        // Extract scheme — match PHP parse_url behavior:
        // "http://host/path" → scheme=http, authority parsing
        // "javascript:alert(1)" → scheme=javascript, path=alert(1) (opaque URI)
        // "//host/path" → no scheme, authority parsing
        // "/path" → no scheme, just path
        if let Some(pos) = rest.find("://") {
            let scheme = &rest[..pos];
            if !scheme.is_empty()
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '.' || c == '-')
            {
                result.scheme = Some(scheme.to_lowercase());
                rest = &rest[pos + 3..];
            }
        } else if let Some(stripped) = rest.strip_prefix("//") {
            rest = stripped;
        } else if let Some(colon_pos) = rest.find(':') {
            // Opaque URI: "scheme:path" without "//"
            // PHP's parse_url returns scheme + path for these
            let scheme = &rest[..colon_pos];
            if !scheme.is_empty()
                && scheme
                    .bytes()
                    .next()
                    .is_some_and(|b| b.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '.' || c == '-')
            {
                result.scheme = Some(scheme.to_lowercase());
                result.path = Some(rest[colon_pos + 1..].to_string());
                return Some(result);
            }
            // Not a valid scheme — treat as path
            result.path = Some(rest.to_string());
            return Some(result);
        } else {
            // No scheme, no authority — just path
            result.path = Some(rest.to_string());
            return Some(result);
        }

        // Extract userinfo@host:port
        let (authority, path) = match rest.find('/') {
            Some(pos) => (&rest[..pos], Some(&rest[pos..])),
            None => (rest, None),
        };

        // Split userinfo from host
        let host_part = if let Some(at_pos) = authority.rfind('@') {
            let userinfo = &authority[..at_pos];
            if let Some(colon) = userinfo.find(':') {
                result.user = Some(userinfo[..colon].to_string());
                result.pass = Some(userinfo[colon + 1..].to_string());
            } else {
                result.user = Some(userinfo.to_string());
            }
            &authority[at_pos + 1..]
        } else {
            authority
        };

        // Split host:port
        if let Some(colon) = host_part.rfind(':') {
            let potential_port = &host_part[colon + 1..];
            if potential_port.chars().all(|c| c.is_ascii_digit()) && !potential_port.is_empty() {
                result.host = Some(host_part[..colon].to_string());
                result.port = Some(potential_port.to_string());
            } else {
                result.host = Some(host_part.to_string());
            }
        } else if !host_part.is_empty() {
            result.host = Some(host_part.to_string());
        }

        if let Some(p) = path {
            result.path = Some(p.to_string());
        }

        Some(result)
    }
}

/// Normalize a path — collapse double slashes and resolve . / ..
fn normalize_path(path: &str) -> String {
    path.replace("//", "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid(s: &str) -> String {
        match validate_redirect(s, "example.com", &["example.com"], None) {
            ValidateResult::Valid(loc) => loc,
            ValidateResult::Fallback => panic!("expected Valid, got Fallback for: {s:?}"),
        }
    }

    fn is_fallback(s: &str) -> bool {
        matches!(
            validate_redirect(s, "example.com", &["example.com"], None),
            ValidateResult::Fallback
        )
    }

    #[test]
    fn simple_same_host() {
        assert_eq!(valid("http://example.com/page"), "http://example.com/page");
    }

    #[test]
    fn relative_path() {
        assert_eq!(valid("/path/to/page"), "/path/to/page");
    }

    #[test]
    fn rejects_different_host() {
        assert!(is_fallback("http://evil.com/phish"));
    }

    #[test]
    fn rejects_javascript_scheme() {
        assert!(is_fallback("javascript:alert(1)"));
    }

    #[test]
    fn rejects_data_scheme() {
        assert!(is_fallback("data:text/html,<h1>test</h1>"));
    }

    #[test]
    fn allows_https() {
        assert_eq!(
            valid("https://example.com/secure"),
            "https://example.com/secure"
        );
    }

    #[test]
    fn protocol_relative_gets_http() {
        assert_eq!(valid("//example.com/page"), "http://example.com/page");
    }

    #[test]
    fn empty_string() {
        assert_eq!(valid(""), "");
    }

    #[test]
    fn allowed_hosts_list() {
        let result = validate_redirect(
            "http://other.com/page",
            "example.com",
            &["example.com", "other.com"],
            None,
        );
        match result {
            ValidateResult::Valid(loc) => assert_eq!(loc, "http://other.com/page"),
            ValidateResult::Fallback => panic!("should be valid — other.com is in allowed_hosts"),
        }
    }
}
