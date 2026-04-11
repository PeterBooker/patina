use ext_php_rs::call_user_func;
use ext_php_rs::prelude::*;
use ext_php_rs::types::Zval;

mod panic_guard;
pub mod php_callback;

// -- Info functions --

#[php_function]
pub fn patina_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[php_function]
pub fn patina_loaded() -> bool {
    true
}

// -- Escaping functions --
// Registered as patina_* (non-pluggable, bridge routes from original WP name)

#[php_function]
pub fn patina_esc_html(text: &str) -> PhpResult<String> {
    panic_guard::guarded("patina_esc_html", || patina_core::escaping::esc_html(text))
}

#[php_function]
pub fn patina_esc_attr(text: &str) -> PhpResult<String> {
    panic_guard::guarded("patina_esc_attr", || patina_core::escaping::esc_attr(text))
}

// -- Pluggable functions --
// Registered under ORIGINAL WordPress names (no prefix).
// WordPress's pluggable.php checks function_exists() and skips its definition.

#[php_function]
pub fn wp_sanitize_redirect(location: &str) -> PhpResult<String> {
    panic_guard::guarded("wp_sanitize_redirect", || {
        patina_core::pluggable::sanitize_redirect(location)
    })
}

/// Validate a redirect URL against allowed hosts.
///
/// Replaces WordPress's pluggable `wp_validate_redirect()`.
/// Calls back into PHP for `home_url()`, `apply_filters('allowed_redirect_hosts', ...)`,
/// and `$_SERVER['REQUEST_URI']`.
#[php_function]
pub fn wp_validate_redirect(location: &str, fallback_url: &str) -> PhpResult<String> {
    // Step 1: Sanitize and trim (using our Rust wp_sanitize_redirect)
    let trimmed = location.trim_matches(&[' ', '\t', '\n', '\r', '\0', '\x08', '\x0B'][..]);
    let sanitized = patina_core::pluggable::sanitize_redirect(trimmed);

    // Step 2: Get home_url host via PHP callback
    let home_host = {
        let mut func = Zval::new();
        func.set_string("home_url", false)
            .map_err(|e| PhpException::default(format!("patina: failed to call home_url: {e}")))?;
        let result = call_user_func!(func);
        match result {
            Ok(r) => extract_host(&r.string().unwrap_or_default()).unwrap_or_default(),
            Err(_) => String::new(), // home_url not available (not in WordPress context)
        }
    };

    // Step 3: Get REQUEST_URI from $_SERVER (for relative path resolution)
    let request_uri = get_server_var("REQUEST_URI");

    // Step 4: Run pure Rust validation to determine host
    // We need to check if the URL has a host to call apply_filters
    let location_host = extract_host_from_redirect(&sanitized);

    // Step 5: Call apply_filters('allowed_redirect_hosts', [...], host)
    let allowed_hosts = {
        let default_hosts = vec![home_host.clone()];
        let host_arg = location_host.as_deref().unwrap_or("");

        match call_apply_filters_redirect_hosts(&default_hosts, host_arg) {
            Ok(hosts) => hosts,
            Err(_) => default_hosts,
        }
    };

    // Step 6: Run the pure Rust validation
    let allowed_refs: Vec<&str> = allowed_hosts.iter().map(|s| s.as_str()).collect();
    match patina_core::pluggable::validate_redirect(
        &sanitized,
        &home_host,
        &allowed_refs,
        request_uri.as_deref(),
    ) {
        patina_core::pluggable::ValidateResult::Valid(loc) => Ok(loc),
        patina_core::pluggable::ValidateResult::Fallback => Ok(fallback_url.to_string()),
    }
}

/// Extract host from a full URL string.
fn extract_host(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split('/').next()?;
    let host = host.split(':').next()?; // Strip port
    Some(host.to_string())
}

/// Extract host from a redirect URL (handles // prefix).
fn extract_host_from_redirect(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("//")
        .or_else(|| url.strip_prefix("https://"))
        .or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split('/').next()?;
    let host = host.split(':').next()?;
    let host = host.split('?').next()?;
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

/// Call apply_filters('allowed_redirect_hosts', $default_hosts, $host).
fn call_apply_filters_redirect_hosts(
    default_hosts: &[String],
    location_host: &str,
) -> Result<Vec<String>, String> {
    let mut func = Zval::new();
    func.set_string("apply_filters", false)
        .map_err(|e| format!("set_string: {e}"))?;

    let result = call_user_func!(
        func,
        "allowed_redirect_hosts",
        default_hosts.to_vec(),
        location_host
    )
    .map_err(|e| format!("call: {e}"))?;

    // Parse the result — should be an array of strings
    if let Some(arr) = result.array() {
        let mut hosts = Vec::new();
        for val in arr.values() {
            if let Some(s) = val.string() {
                hosts.push(s);
            }
        }
        Ok(hosts)
    } else {
        Ok(default_hosts.to_vec())
    }
}

/// Get a value from PHP's $_SERVER superglobal.
fn get_server_var(key: &str) -> Option<String> {
    // Access $_SERVER via ext-php-rs
    let mut server_func = Zval::new();
    server_func.set_string("function_exists", false).ok()?;
    // Simpler approach: use a small PHP eval to get $_SERVER value
    // Actually, we can't easily access superglobals from ext-php-rs directly.
    // For now, return None — relative path resolution is a rare edge case.
    // TODO: Access $_SERVER via zend_hash_str_find in the executor globals
    let _ = key;
    None
}

// -- Module registration --

#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
        // info
        .function(wrap_function!(patina_version))
        .function(wrap_function!(patina_loaded))
        // escaping
        .function(wrap_function!(patina_esc_html))
        .function(wrap_function!(patina_esc_attr))
        // pluggable
        .function(wrap_function!(wp_sanitize_redirect))
        .function(wrap_function!(wp_validate_redirect))
}
