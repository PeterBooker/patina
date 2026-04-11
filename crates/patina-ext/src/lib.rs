use ext_php_rs::call_user_func;
use ext_php_rs::ffi::{
    ext_php_rs_executor_globals, zend_fetch_function_str, zend_function, zend_hash_str_find,
};
use ext_php_rs::prelude::*;
use ext_php_rs::types::Zval;

use std::ffi::c_char;
use std::sync::Mutex;

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
const OVERRIDES: &[(&str, &str)] = &[
    ("esc_html", "patina_esc_html_filtered"),
    ("esc_attr", "patina_esc_attr_filtered"),
    ("wp_kses_post", "patina_wp_kses_post_filtered"),
];

/// Activate Patina: replace WordPress core functions with Rust implementations.
///
/// Call this from a mu-plugin AFTER WordPress has loaded its core functions.
/// Returns the number of functions successfully overridden.
#[php_function]
pub fn patina_activate() -> PhpResult<i64> {
    let mut count = 0i64;

    for &(target, replacement) in OVERRIDES {
        match swap_function(target, replacement) {
            Ok(()) => count += 1,
            Err(_) => {
                // Skip this function silently — caller can check count vs expected
            }
        }
    }

    Ok(count)
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
pub fn patina_esc_html(text: &str) -> PhpResult<String> {
    panic_guard::guarded("patina_esc_html", || {
        patina_core::escaping::esc_html(text).into_owned()
    })
}

#[php_function]
pub fn patina_esc_attr(text: &str) -> PhpResult<String> {
    panic_guard::guarded("patina_esc_attr", || {
        patina_core::escaping::esc_attr(text).into_owned()
    })
}

// ============================================================================
// Escaping functions — filtered (calls apply_filters, for WordPress override)
// ============================================================================

/// esc_html replacement that calls apply_filters('esc_html', $result, $text).
/// This is what gets swapped into the function table for esc_html().
#[php_function]
pub fn patina_esc_html_filtered(text: &str) -> PhpResult<String> {
    let safe_text = panic_guard::guarded("esc_html", || {
        patina_core::escaping::esc_html(text).into_owned()
    })?;

    // Call apply_filters('esc_html', $safe_text, $text) — matching WordPress behavior
    let mut func = Zval::new();
    func.set_string("apply_filters", false)
        .map_err(|e| PhpException::default(format!("patina: apply_filters setup: {e}")))?;

    match call_user_func!(func, "esc_html", safe_text.as_str(), text) {
        Ok(result) => Ok(result.string().unwrap_or(safe_text)),
        Err(_) => Ok(safe_text),
    }
}

/// esc_attr replacement that calls apply_filters('esc_attr', $result, $text).
#[php_function]
pub fn patina_esc_attr_filtered(text: &str) -> PhpResult<String> {
    let safe_text = panic_guard::guarded("esc_attr", || {
        patina_core::escaping::esc_attr(text).into_owned()
    })?;

    let mut func = Zval::new();
    func.set_string("apply_filters", false)
        .map_err(|e| PhpException::default(format!("patina: apply_filters setup: {e}")))?;

    match call_user_func!(func, "esc_attr", safe_text.as_str(), text) {
        Ok(result) => Ok(result.string().unwrap_or(safe_text)),
        Err(_) => Ok(safe_text),
    }
}

// ============================================================================
// Pluggable functions (registered under original WordPress names)
// ============================================================================

#[php_function]
pub fn wp_sanitize_redirect(location: &str) -> PhpResult<String> {
    panic_guard::guarded("wp_sanitize_redirect", || {
        patina_core::pluggable::sanitize_redirect(location)
    })
}

#[php_function]
pub fn wp_validate_redirect(location: &str, fallback_url: &str) -> PhpResult<String> {
    let trimmed = location.trim_matches(&[' ', '\t', '\n', '\r', '\0', '\x08', '\x0B'][..]);
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
        patina_core::pluggable::ValidateResult::Fallback => Ok(fallback_url.to_string()),
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
pub fn patina_wp_kses_post(content: &str) -> PhpResult<String> {
    panic_guard::guarded("patina_wp_kses_post", || {
        patina_core::kses::wp_kses_post(content)
    })
}

/// wp_kses_post filtered — calls apply_filters('pre_kses', ...) on INPUT
/// before processing, matching WordPress's wp_kses_hook behavior.
/// This is what gets swapped into the function table.
#[php_function]
pub fn patina_wp_kses_post_filtered(content: &str) -> PhpResult<String> {
    // wp_kses_hook fires 'pre_kses' on the input BEFORE processing.
    // For wp_kses_post, the allowed_html is 'post' and protocols are defaults.
    let mut func = Zval::new();
    func.set_string("apply_filters", false)
        .map_err(|e| PhpException::default(format!("patina: apply_filters setup: {e}")))?;

    let filtered_content = match call_user_func!(func, "pre_kses", content, "post") {
        Ok(result) => result.string().unwrap_or_else(|| content.to_string()),
        Err(_) => content.to_string(),
    };

    panic_guard::guarded("wp_kses_post", || {
        patina_core::kses::wp_kses_post(&filtered_content)
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
        .function(wrap_function!(patina_wp_kses_post_filtered))
        // pluggable
        .function(wrap_function!(wp_sanitize_redirect))
        .function(wrap_function!(wp_validate_redirect))
}
