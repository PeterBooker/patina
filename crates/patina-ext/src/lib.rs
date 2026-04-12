use ext_php_rs::call_user_func;
use ext_php_rs::ffi::{
    ext_php_rs_executor_globals, zend_fetch_function_str, zend_function, zend_hash_str_find,
};
use ext_php_rs::prelude::*;
use ext_php_rs::types::Zval;

use std::ffi::c_char;
use std::sync::Mutex;

mod kses_bridge;
mod panic_guard;
pub mod php_callback;

// ============================================================================
// Activation: Zend function table swap
// ============================================================================

/// Wrapper to make *mut zend_function safe to store in a static Mutex.
/// PHP-FPM workers are single-threaded — the pointer is only accessed
/// from the same thread that created it.
struct FuncPtr(*mut zend_function);
unsafe impl Send for FuncPtr {}

/// Original function pointers saved during activation, keyed by function name.
/// Used by patina_deactivate() to restore originals.
static ORIGINALS: Mutex<Vec<(&'static str, FuncPtr)>> = Mutex::new(Vec::new());

/// Function overrides to apply: (wordpress_name, patina_name).
/// The patina_name function replaces the wordpress_name in the function table.
///
/// Each entry's replacement must have an **identical PHP signature** to the
/// WordPress function it replaces. Otherwise pre-compiled callers (e.g.
/// `wp_kses_post` in wp-includes/kses.php) emit specialized `DO_UCALL`
/// opcodes assuming the original user-function ABI, and dispatching them
/// to an ext-php-rs internal-function replacement crashes in `execute_ex`.
///
/// For functions whose signature can't be matched exactly by a Rust
/// `&str` parameter list, use a PHP user-function shim instead — see
/// `SHIM_OVERRIDES` and `KSES_SHIM_PHP` below.
const OVERRIDES: &[(&str, &str)] = &[
    ("esc_html", "patina_esc_html_filtered"),
    ("esc_attr", "patina_esc_attr_filtered"),
];

/// Shim overrides: a PHP user-function shim is defined via `php_eval::execute`
/// during activation, then swapped into the target slot. The shim trampolines
/// to an ext-php-rs internal function. This works for callers compiled against
/// the original WP function because user→user dispatch stays valid.
///
/// Entry format: `(wordpress_name, shim_name)`. The shim body is embedded in
/// `KSES_SHIM_PHP` and must define the shim function with a signature that
/// matches the WordPress function byte-for-byte.
const SHIM_OVERRIDES: &[(&str, &str)] = &[("wp_kses", "__patina_wp_kses_shim__")];

/// PHP source that declares the wp_kses shim. Compiled and executed once
/// during `patina_activate`. The shim signature mirrors WordPress's
/// `wp_kses($content, $allowed_html, $allowed_protocols = array())` exactly,
/// so that pre-compiled `DO_UCALL` callers (wp_kses_post et al.) dispatch
/// into user-function territory as expected.
const KSES_SHIM_PHP: &str = r#"<?php
function __patina_wp_kses_shim__($content, $allowed_html, $allowed_protocols = array()) {
    return patina_wp_kses_internal($content, $allowed_html, $allowed_protocols);
}
"#;

/// Activate Patina: replace WordPress core functions with Rust implementations.
///
/// Call this from a mu-plugin AFTER WordPress has loaded its core functions.
/// Returns the number of functions successfully overridden.
#[php_function]
pub fn patina_activate() -> PhpResult<i64> {
    let mut count = 0i64;

    for &(target, replacement) in OVERRIDES {
        if swap_function(target, replacement).is_ok() {
            count += 1;
        }
    }

    // Shim overrides: define the PHP user-function shim once, then swap.
    // The shim definition is idempotent-ish: if __patina_wp_kses_shim__
    // already exists, compiling again will emit a warning but not fail.
    // We check first to stay clean on re-activation.
    if !php_function_exists("__patina_wp_kses_shim__") {
        if let Err(e) = ext_php_rs::php_eval::execute(KSES_SHIM_PHP) {
            return Err(PhpException::default(format!(
                "patina: shim eval failed: {e}"
            )));
        }
    }
    for &(target, replacement) in SHIM_OVERRIDES {
        if swap_function(target, replacement).is_ok() {
            count += 1;
        }
    }

    Ok(count)
}

/// Check whether a PHP function exists in the current function table.
fn php_function_exists(name: &str) -> bool {
    unsafe {
        let eg = ext_php_rs_executor_globals();
        if eg.is_null() {
            return false;
        }
        let fn_table = (*eg).function_table;
        if fn_table.is_null() {
            return false;
        }
        !zend_hash_str_find(fn_table, name.as_ptr() as *const c_char, name.len()).is_null()
    }
}

/// Deactivate Patina: restore all original WordPress function implementations.
#[php_function]
pub fn patina_deactivate() -> PhpResult<i64> {
    let mut originals = ORIGINALS
        .lock()
        .map_err(|_| PhpException::default("patina: failed to lock originals".to_string()))?;

    let count = originals.len() as i64;

    for (name, func_ptr) in originals.drain(..) {
        unsafe {
            let eg = ext_php_rs_executor_globals();
            let fn_table = (*eg).function_table;
            let target_zval =
                zend_hash_str_find(fn_table, name.as_ptr() as *const c_char, name.len());
            if !target_zval.is_null() {
                (*target_zval).value.ptr = func_ptr.0 as *mut std::ffi::c_void;
            }
        }
    }

    Ok(count)
}

/// Return activation status: which functions are currently overridden.
#[php_function]
pub fn patina_status() -> PhpResult<Vec<String>> {
    let originals = ORIGINALS
        .lock()
        .map_err(|_| PhpException::default("patina: failed to lock originals".to_string()))?;

    Ok(originals
        .iter()
        .map(|(name, _)| (*name).to_string())
        .collect())
}

/// Swap a function in the Zend function table.
///
/// Finds `replacement`'s zval in the table, then copies its zend_function pointer
/// into `target`'s zval. The original pointer is saved for rollback.
fn swap_function(target: &'static str, replacement: &str) -> Result<(), String> {
    unsafe {
        let eg = ext_php_rs_executor_globals();
        if eg.is_null() {
            return Err("executor globals not available".to_string());
        }
        let fn_table = (*eg).function_table;
        if fn_table.is_null() {
            return Err("function table not available".to_string());
        }

        // Find the target's zval in the hash table (lowercase key)
        let target_zval =
            zend_hash_str_find(fn_table, target.as_ptr() as *const c_char, target.len());

        if target_zval.is_null() {
            return Err(format!(
                "target function '{target}' not found (WordPress not loaded?)"
            ));
        }

        // Save the original zval for deactivation by reading value.ptr.
        let original_ptr = (*target_zval).value.ptr;
        if let Ok(mut originals) = ORIGINALS.lock() {
            if !originals.iter().any(|(n, _)| *n == target) {
                originals.push((target, FuncPtr(original_ptr as *mut zend_function)));
            }
        }

        // Get the replacement function via zend_fetch_function_str.
        let replacement_func =
            zend_fetch_function_str(replacement.as_ptr() as *const c_char, replacement.len());

        if replacement_func.is_null() {
            return Err(format!("replacement '{replacement}' not found via fetch"));
        }

        // Direct pointer write into the existing zval. We do NOT use
        // zend_hash_str_update because that triggers the hash table's destructor
        // on the old value, which frees the PHP function's op_array and corrupts
        // the runtime. Instead, we just overwrite the pointer in-place.
        (*target_zval).value.ptr = replacement_func as *mut std::ffi::c_void;

        Ok(())
    }
}

// ============================================================================
// Info functions
// ============================================================================

#[php_function]
pub fn patina_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[php_function]
pub fn patina_loaded() -> bool {
    true
}

// ============================================================================
// Escaping functions — raw (no filters, for direct use and benchmarking)
// ============================================================================

#[php_function]
pub fn patina_esc_html(text: &Zval) -> PhpResult<String> {
    let text_str = text.coerce_to_string().unwrap_or_default();
    panic_guard::guarded("patina_esc_html", || {
        patina_core::escaping::esc_html(&text_str).into_owned()
    })
}

#[php_function]
pub fn patina_esc_attr(text: &Zval) -> PhpResult<String> {
    let text_str = text.coerce_to_string().unwrap_or_default();
    panic_guard::guarded("patina_esc_attr", || {
        patina_core::escaping::esc_attr(&text_str).into_owned()
    })
}

// ============================================================================
// Escaping functions — filtered (calls apply_filters, for WordPress override)
// ============================================================================

/// esc_html replacement that calls apply_filters('esc_html', $result, $text).
/// This is what gets swapped into the function table for esc_html().
///
/// Accepts `&Zval` rather than `&str` so that non-string scalars (int,
/// float, bool, null) are coerced to a string the same way stock PHP's
/// `esc_html()` / `htmlspecialchars()` do. Without this, wp-admin screens
/// that call `esc_html($integer)` — e.g. pagination controls — throw
/// "Invalid value given for argument `text`" from ext-php-rs's strict
/// parameter parsing.
#[php_function]
pub fn patina_esc_html_filtered(text: &Zval) -> PhpResult<String> {
    let text_str = text.coerce_to_string().unwrap_or_default();

    let safe_text = panic_guard::guarded("esc_html", || {
        patina_core::escaping::esc_html(&text_str).into_owned()
    })?;

    // Call apply_filters('esc_html', $safe_text, $text) — matching WordPress behavior
    let mut func = Zval::new();
    func.set_string("apply_filters", false)
        .map_err(|e| PhpException::default(format!("patina: apply_filters setup: {e}")))?;

    match call_user_func!(func, "esc_html", safe_text.as_str(), text_str.as_str()) {
        Ok(result) => Ok(result.string().unwrap_or(safe_text)),
        Err(_) => Ok(safe_text),
    }
}

/// esc_attr replacement that calls apply_filters('esc_attr', $result, $text).
///
/// Accepts `&Zval` for PHP loose-typing compatibility — see `patina_esc_html_filtered`.
#[php_function]
pub fn patina_esc_attr_filtered(text: &Zval) -> PhpResult<String> {
    let text_str = text.coerce_to_string().unwrap_or_default();

    let safe_text = panic_guard::guarded("esc_attr", || {
        patina_core::escaping::esc_attr(&text_str).into_owned()
    })?;

    let mut func = Zval::new();
    func.set_string("apply_filters", false)
        .map_err(|e| PhpException::default(format!("patina: apply_filters setup: {e}")))?;

    match call_user_func!(func, "esc_attr", safe_text.as_str(), text_str.as_str()) {
        Ok(result) => Ok(result.string().unwrap_or(safe_text)),
        Err(_) => Ok(safe_text),
    }
}

// ============================================================================
// Pluggable functions (registered under original WordPress names)
// ============================================================================

#[php_function]
pub fn wp_sanitize_redirect(location: &Zval) -> PhpResult<String> {
    // PHP's wp_sanitize_redirect is untyped; callers pass whatever WP hands
    // them (strings from $_REQUEST, or sometimes bool/null from functions
    // like wp_get_referer). Coerce to match stock PHP's loose behavior.
    let location_str = location.coerce_to_string().unwrap_or_default();
    panic_guard::guarded("wp_sanitize_redirect", || {
        patina_core::pluggable::sanitize_redirect(&location_str)
    })
}

#[php_function]
pub fn wp_validate_redirect(location: &Zval, fallback_url: Option<&Zval>) -> PhpResult<String> {
    // Match PHP's `function wp_validate_redirect( $location, $fallback_url = '' )`:
    // - both params untyped (coerce_to_string for loose-typing compat)
    // - fallback_url is optional with default ''
    let location_str = location.coerce_to_string().unwrap_or_default();
    let fallback_str = fallback_url
        .and_then(|z| z.coerce_to_string())
        .unwrap_or_default();

    let trimmed = location_str.trim_matches(&[' ', '\t', '\n', '\r', '\0', '\x08', '\x0B'][..]);
    let sanitized = patina_core::pluggable::sanitize_redirect(trimmed);

    let home_host = {
        let mut func = Zval::new();
        func.set_string("home_url", false)
            .map_err(|e| PhpException::default(format!("patina: home_url setup: {e}")))?;
        match call_user_func!(func) {
            Ok(r) => extract_host(&r.string().unwrap_or_default()).unwrap_or_default(),
            Err(_) => String::new(),
        }
    };

    let request_uri = get_server_var("REQUEST_URI");
    let location_host = extract_host_from_redirect(&sanitized);

    let allowed_hosts = {
        let default_hosts = vec![home_host.clone()];
        let host_arg = location_host.as_deref().unwrap_or("");
        match call_apply_filters_redirect_hosts(&default_hosts, host_arg) {
            Ok(hosts) => hosts,
            Err(_) => default_hosts,
        }
    };

    let allowed_refs: Vec<&str> = allowed_hosts.iter().map(|s| s.as_str()).collect();
    match patina_core::pluggable::validate_redirect(
        &sanitized,
        &home_host,
        &allowed_refs,
        request_uri.as_deref(),
    ) {
        patina_core::pluggable::ValidateResult::Valid(loc) => Ok(loc),
        patina_core::pluggable::ValidateResult::Fallback => Ok(fallback_str),
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn extract_host(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split('/').next()?;
    let host = host.split(':').next()?;
    Some(host.to_string())
}

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

fn get_server_var(_key: &str) -> Option<String> {
    None // TODO: access $_SERVER via executor globals
}

// ============================================================================
// KSES functions (HTML sanitization)
// ============================================================================

/// wp_kses_post raw — no filters, for direct use and benchmarking.
#[php_function]
pub fn patina_wp_kses_post(content: &Zval) -> PhpResult<String> {
    let content_str = content.coerce_to_string().unwrap_or_default();
    panic_guard::guarded("patina_wp_kses_post", || {
        patina_core::kses::wp_kses_post(&content_str)
    })
}

/// Internal wp_kses implementation — the Rust side of the shim trampoline.
///
/// Called from `__patina_wp_kses_shim__` which is itself bound to `wp_kses`
/// in the function table. The shim's signature mirrors WordPress's
/// `wp_kses()` exactly (3 params, optional 3rd), so pre-compiled PHP callers
/// dispatch into user-function territory safely. The shim then forwards all
/// three args to this function, which does the full filter bridge work:
///
/// 1. Fires `apply_filters('pre_kses', $content, $allowed_html, $allowed_protocols)`
///    so `wp_pre_kses_less_than` and any plugin filters run.
/// 2. Resolves the allowed HTML spec via `wp_kses_allowed_html($allowed_html)`
///    (fires the `wp_kses_allowed_html` filter). Falls back to the cached
///    Rust `post` spec when no filter is registered and the context is `'post'`.
/// 3. Resolves protocols via `wp_allowed_protocols()` (fires
///    `kses_allowed_protocols`) or uses the caller's explicit list.
/// 4. Resolves URI attributes via `wp_kses_uri_attributes()` (fires
///    `wp_kses_uri_attributes`).
/// 5. Runs the Rust sanitization pipeline.
#[php_function]
pub fn patina_wp_kses_internal(
    content: &Zval,
    allowed_html: &Zval,
    allowed_protocols: &Zval,
) -> PhpResult<String> {
    // PHP's wp_kses accepts any scalar — coerce here so int/float/bool/null
    // callers (which stock PHP handles transparently) don't trip the strict
    // ext-php-rs parameter parser.
    let content_str = content.coerce_to_string().unwrap_or_default();

    let filtered_content =
        kses_bridge::apply_pre_kses(&content_str, allowed_html, allowed_protocols);

    let spec_ref = kses_bridge::resolve_allowed_html(allowed_html);
    let protocols_ref = kses_bridge::resolve_protocols(allowed_protocols);
    let uri_attrs_ref = kses_bridge::resolve_uri_attrs();

    panic_guard::guarded("wp_kses", || {
        let protocols = protocols_ref.as_slice();
        let uri_attrs = uri_attrs_ref.as_slice();
        patina_core::kses::wp_kses_with_uri_attrs(
            &filtered_content,
            spec_ref.as_ref(),
            &protocols,
            &uri_attrs,
        )
    })
}

// ============================================================================
// Module registration
// ============================================================================

#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
        // info + activation
        .function(wrap_function!(patina_version))
        .function(wrap_function!(patina_loaded))
        .function(wrap_function!(patina_activate))
        .function(wrap_function!(patina_deactivate))
        .function(wrap_function!(patina_status))
        // escaping (raw — no filters)
        .function(wrap_function!(patina_esc_html))
        .function(wrap_function!(patina_esc_attr))
        // escaping (filtered — for WordPress override)
        .function(wrap_function!(patina_esc_html_filtered))
        .function(wrap_function!(patina_esc_attr_filtered))
        // kses
        .function(wrap_function!(patina_wp_kses_post))
        .function(wrap_function!(patina_wp_kses_internal))
        // pluggable
        .function(wrap_function!(wp_sanitize_redirect))
        .function(wrap_function!(wp_validate_redirect))
}
