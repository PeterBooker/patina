use ext_php_rs::boxed::ZBox;
use ext_php_rs::call_user_func;
use ext_php_rs::ffi::{
    ext_php_rs_executor_globals, zend_fetch_function_str, zend_function, zend_hash_str_find,
};
use ext_php_rs::prelude::*;
use ext_php_rs::types::{ZendHashTable, Zval};

use std::ffi::c_char;
use std::sync::Mutex;

mod blocks_bridge;
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

/// Core function overrides. **Every** entry is shimmed via a PHP user
/// function defined in `PATINA_SHIMS_PHP` — we never swap a target directly
/// to an ext-php-rs internal function. The shim approach is the only one
/// we trust across every WordPress function signature, for two reasons:
///
/// 1. **User→user dispatch stays valid for pre-compiled callers.** When
///    `wp-includes/kses.php` parses `wp_kses_post`, `wp_kses` is a user
///    function; the emitted `DO_UCALL` opcodes assume user-function ABI.
///    Swapping `wp_kses` directly to an ext-php-rs internal function made
///    pre-compiled callers crash in `execute_ex`. The shim is a real user
///    function, so user→user dispatch stays valid.
///
/// 2. **`(string)` cast in the shim handles PHP's loose typing.** Stock
///    PHP's `esc_html()` / `htmlspecialchars()` internally coerce any
///    scalar via `zval_get_string()`. ext-php-rs's `&str` parameter
///    parser is strict and throws "Invalid value given for argument"
///    for non-string zvals — even in PHP 8 non-strict mode, it bypasses
///    `zend_parse_parameters`'s coercion path. Putting the `(string)$x`
///    cast in the PHP shim reproduces stock coercion at zero boilerplate
///    cost on the Rust side.
///
/// Every shim trampolines to an internal Rust function named
/// `patina_<name>_internal` that holds the real logic.
///
/// Entry format: `(wordpress_name, shim_name)`.
const SHIM_OVERRIDES: &[(&str, &str)] = &[
    ("esc_html", "__patina_esc_html_shim__"),
    ("esc_attr", "__patina_esc_attr_shim__"),
    ("wp_kses", "__patina_wp_kses_shim__"),
    ("parse_blocks", "__patina_parse_blocks_shim__"),
];

/// PHP source for all shims. Compiled and executed once by
/// `patina_activate()`. Each shim:
///
/// - matches the WordPress function signature byte-for-byte, including
///   optional args and their default values,
/// - applies `(string)` to string-typed params so the Rust internal
///   function can keep a clean `&str` signature,
/// - forwards polymorphic params (like `$allowed_html` in `wp_kses`,
///   which is string-or-array) as-is,
/// - honors any WordPress filter that lets plugins swap the underlying
///   implementation class (as `parse_blocks` does with `block_parser_class`).
const PATINA_SHIMS_PHP: &str = r#"<?php
function __patina_esc_html_shim__($text) {
    return patina_esc_html_internal((string) $text);
}
function __patina_esc_attr_shim__($text) {
    return patina_esc_attr_internal((string) $text);
}
function __patina_wp_kses_shim__($content, $allowed_html, $allowed_protocols = array()) {
    return patina_wp_kses_internal((string) $content, $allowed_html, $allowed_protocols);
}
function __patina_parse_blocks_shim__($content) {
    // WP stock parse_blocks() fires this filter to let plugins swap in
    // their own parser class. If anything other than the default is
    // returned we must fall back to the PHP implementation so plugin
    // custom parsers keep working.
    $parser_class = apply_filters('block_parser_class', 'WP_Block_Parser');
    if ($parser_class !== 'WP_Block_Parser') {
        $parser = new $parser_class();
        return $parser->parse((string) $content);
    }
    return patina_parse_blocks_internal((string) $content);
}
"#;

/// Activate Patina: replace WordPress core functions with Rust implementations.
///
/// Call this from a mu-plugin AFTER WordPress has loaded its core functions.
/// Returns the number of functions successfully overridden.
///
/// `skip_list` is an optional PHP array of WordPress function names to
/// leave untouched. Used by the bench runner to A/B individual overrides
/// without rebuilding the `.so`:
///
/// ```php
/// // skip the wp_kses override; keep esc_html / esc_attr / parse_blocks
/// patina_activate(['wp_kses']);
/// ```
///
/// The PHP bridge mu-plugin builds this list from `PATINA_DISABLE_ESC`,
/// `PATINA_DISABLE_KSES`, and `PATINA_DISABLE_PARSE_BLOCKS` — see
/// `php/bridge/patina-bridge.php`.
#[php_function]
pub fn patina_activate(skip_list: Option<&Zval>) -> PhpResult<i64> {
    // Idempotency: if any of the shim functions already exist we've
    // already activated this request, so skip the eval.
    if !php_function_exists("__patina_wp_kses_shim__") {
        if let Err(e) = ext_php_rs::php_eval::execute(PATINA_SHIMS_PHP) {
            return Err(PhpException::default(format!(
                "patina: shim eval failed: {e}"
            )));
        }
    }

    let skip: Vec<String> = skip_list
        .and_then(|z| z.array())
        .map(|arr| arr.values().filter_map(|v| v.string()).collect::<Vec<_>>())
        .unwrap_or_default();

    let mut count = 0i64;
    for &(target, replacement) in SHIM_OVERRIDES {
        if skip.iter().any(|s| s == target) {
            continue;
        }
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
// Escaping — shim trampoline targets
// ============================================================================

// These are the Rust internal functions that the PHP shims in
// `PATINA_SHIMS_PHP` forward to. The shim casts `$text` to string before
// calling here, so the Rust side can stay strictly typed.

/// esc_html internal. Called from `__patina_esc_html_shim__`.
///
/// Applies `apply_filters('esc_html', $result, $text)` to match WordPress
/// behavior.
#[php_function]
pub fn patina_esc_html_internal(text: &str) -> PhpResult<String> {
    let safe_text = panic_guard::guarded("esc_html", || {
        patina_core::escaping::esc_html(text).into_owned()
    })?;

    let mut func = Zval::new();
    func.set_string("apply_filters", false)
        .map_err(|e| PhpException::default(format!("patina: apply_filters setup: {e}")))?;

    match call_user_func!(func, "esc_html", safe_text.as_str(), text) {
        Ok(result) => Ok(result.string().unwrap_or(safe_text)),
        Err(_) => Ok(safe_text),
    }
}

/// esc_attr internal. Called from `__patina_esc_attr_shim__`.
#[php_function]
pub fn patina_esc_attr_internal(text: &str) -> PhpResult<String> {
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
    content: &str,
    allowed_html: &Zval,
    allowed_protocols: &Zval,
) -> PhpResult<String> {
    // `$content` is cast via `(string)` in `__patina_wp_kses_shim__` so
    // it's always a real string by the time we get here. `$allowed_html`
    // stays polymorphic (string context like `"post"` or a raw tags
    // array), and `$allowed_protocols` can be null or an array.
    let filtered_content = kses_bridge::apply_pre_kses(content, allowed_html, allowed_protocols);

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
// Block parser — shim trampoline target
// ============================================================================

/// parse_blocks internal. Called from `__patina_parse_blocks_shim__`.
///
/// Returns a PHP array of block associative arrays matching the exact
/// shape WordPress's `parse_blocks()` produces — each element has keys
/// `blockName`, `attrs`, `innerBlocks`, `innerHTML`, `innerContent` in
/// that order.
///
/// The shim handles the `block_parser_class` filter at the PHP layer —
/// when a plugin swaps in a custom parser class, the shim falls back
/// to instantiating it and never reaches this function. When this
/// function runs, we know we can use the Rust parser safely.
#[php_function]
pub fn patina_parse_blocks_internal(content: &str) -> PhpResult<ZBox<ZendHashTable>> {
    panic_guard::guarded("parse_blocks", || {
        let blocks = patina_core::blocks::parse_blocks(content);
        blocks_bridge::blocks_to_php_array(&blocks)
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
        // escaping (raw — no filters, for benchmarks / direct use)
        .function(wrap_function!(patina_esc_html))
        .function(wrap_function!(patina_esc_attr))
        // escaping (shim trampoline targets — called by the eval'd PHP shims)
        .function(wrap_function!(patina_esc_html_internal))
        .function(wrap_function!(patina_esc_attr_internal))
        // kses
        .function(wrap_function!(patina_wp_kses_post))
        .function(wrap_function!(patina_wp_kses_internal))
        // block parser
        .function(wrap_function!(patina_parse_blocks_internal))
        // pluggable
        .function(wrap_function!(wp_sanitize_redirect))
        .function(wrap_function!(wp_validate_redirect))
}
