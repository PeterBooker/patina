//! Bridge between PHP runtime values and patina-core's kses engine.
//!
//! Responsible for calling WordPress's filter APIs from Rust and converting
//! the resulting PHP arrays into the Rust types that patina-core consumes.

use ext_php_rs::call_user_func;
use ext_php_rs::convert::IntoZvalDyn;
use ext_php_rs::types::{ArrayKey, Zval};

use patina_core::kses::allowed_html::{AllowedHtmlSpec, TagSpec};
use patina_core::kses::get_post_spec;
use patina_core::kses::protocols::{DEFAULT_PROTOCOLS, DEFAULT_URI_ATTRIBUTES};

use std::collections::HashMap;

/// Owned or cached-static allowed HTML spec.
///
/// Fast path hands back a `&'static` reference to patina-core's cached
/// `post` preset. Slow path owns a freshly-built spec resolved from PHP
/// (via `wp_kses_allowed_html` + its filter chain).
pub enum SpecRef {
    StaticPost,
    Owned(AllowedHtmlSpec),
}

impl SpecRef {
    pub fn as_ref(&self) -> &AllowedHtmlSpec {
        match self {
            SpecRef::StaticPost => get_post_spec(),
            SpecRef::Owned(s) => s,
        }
    }
}

/// Owned or cached-static string list. Used for protocols and URI attrs.
pub enum StrListRef {
    Static(&'static [&'static str]),
    Owned(Vec<String>),
}

impl StrListRef {
    pub fn as_slice(&self) -> Vec<&str> {
        match self {
            StrListRef::Static(s) => s.to_vec(),
            StrListRef::Owned(v) => v.iter().map(|s| s.as_str()).collect(),
        }
    }
}

/// Returns true iff `has_filter($hook)` returns anything other than PHP `false`.
pub fn has_filter(hook: &str) -> bool {
    let mut func = Zval::new();
    if func.set_string("has_filter", false).is_err() {
        return false;
    }
    match call_user_func!(func, hook) {
        Ok(result) => !matches!(result.bool(), Some(false)),
        Err(_) => false,
    }
}

/// Fire `apply_filters('pre_kses', $content, $allowed_html, $allowed_protocols)`.
///
/// Returns the filtered content. On any error, falls back to the original
/// content — matches WordPress's "filters shouldn't be able to break kses".
pub fn apply_pre_kses(content: &str, allowed_html: &Zval, allowed_protocols: &Zval) -> String {
    let mut func = Zval::new();
    if func.set_string("apply_filters", false).is_err() {
        return content.to_string();
    }

    let filter_name = "pre_kses";
    let args: Vec<&dyn IntoZvalDyn> = vec![&filter_name, &content, allowed_html, allowed_protocols];

    match func.callable().and_then(|c| c.try_call(args).ok()) {
        Some(result) => result.string().unwrap_or_else(|| content.to_string()),
        None => content.to_string(),
    }
}

/// Resolve the `$allowed_html` argument of `wp_kses` into a concrete spec.
///
/// - If caller passed a string context (`'post'`, `'data'`, `'strip'`, ...)
///   we call `wp_kses_allowed_html($context)` which fires the filter chain
///   for us and returns the resolved array. We then convert it to Rust.
/// - If caller passed an explicit array, we call `wp_kses_allowed_html($array)`
///   so the filter still fires on it (matching WP's behavior with an array
///   argument), then convert.
/// - If fast path is eligible (context == "post" && no filter registered),
///   we return `SpecRef::StaticPost` and skip conversion entirely.
pub fn resolve_allowed_html(allowed_html: &Zval) -> SpecRef {
    let is_post_context = allowed_html.str() == Some("post");
    if is_post_context && !has_filter("wp_kses_allowed_html") {
        return SpecRef::StaticPost;
    }

    let mut func = Zval::new();
    if func.set_string("wp_kses_allowed_html", false).is_err() {
        return SpecRef::StaticPost;
    }

    let args: Vec<&dyn IntoZvalDyn> = vec![allowed_html];
    let resolved = match func.callable().and_then(|c| c.try_call(args).ok()) {
        Some(r) => r,
        None => return SpecRef::StaticPost,
    };

    let Some(ht) = resolved.array() else {
        return SpecRef::StaticPost;
    };

    let mut tags: HashMap<String, TagSpec> = HashMap::with_capacity(ht.len());
    for (tag_key, attrs_zval) in ht.iter() {
        let tag_name = match tag_key {
            ArrayKey::String(s) => s,
            ArrayKey::Str(s) => s.to_string(),
            ArrayKey::Long(i) => i.to_string(),
        };

        let mut spec = TagSpec::default();
        if let Some(attrs_ht) = attrs_zval.array() {
            for (attr_key, attr_val) in attrs_ht.iter() {
                let attr_name = match attr_key {
                    ArrayKey::String(s) => s,
                    ArrayKey::Str(s) => s.to_string(),
                    ArrayKey::Long(i) => i.to_string(),
                };

                // WordPress uses true/false for simple allow, or a nested
                // array with `required` / `values` / `value_callback` for
                // attributes whose values must be validated in PHP.
                let needs_php = attr_val.array().is_some_and(|obj| {
                    obj.get("required").is_some()
                        || obj.get("values").is_some()
                        || obj.get("value_callback").is_some()
                });

                if needs_php {
                    spec.attrs_needing_php_validation
                        .insert(attr_name.to_lowercase());
                } else {
                    spec.add_attr_name(&attr_name);
                }
            }
        }

        tags.insert(tag_name.to_lowercase(), spec);
    }

    SpecRef::Owned(AllowedHtmlSpec { tags })
}

/// Resolve the `$allowed_protocols` argument of `wp_kses`.
///
/// - If caller passed a non-empty array, convert those entries.
/// - Otherwise call `wp_allowed_protocols()` which applies the
///   `kses_allowed_protocols` filter.
/// - Fast path: if caller didn't specify and no filter is registered,
///   return `StrListRef::Static(DEFAULT_PROTOCOLS)`.
pub fn resolve_protocols(allowed_protocols: &Zval) -> StrListRef {
    if let Some(arr) = allowed_protocols.array() {
        if !arr.is_empty() {
            let protos: Vec<String> = arr.values().filter_map(|v| v.string()).collect();
            return StrListRef::Owned(protos);
        }
    }

    if !has_filter("kses_allowed_protocols") {
        return StrListRef::Static(DEFAULT_PROTOCOLS);
    }

    let mut func = Zval::new();
    if func.set_string("wp_allowed_protocols", false).is_err() {
        return StrListRef::Static(DEFAULT_PROTOCOLS);
    }

    let result = match call_user_func!(func) {
        Ok(r) => r,
        Err(_) => return StrListRef::Static(DEFAULT_PROTOCOLS),
    };

    match result.array() {
        Some(arr) => StrListRef::Owned(arr.values().filter_map(|v| v.string()).collect()),
        None => StrListRef::Static(DEFAULT_PROTOCOLS),
    }
}

/// Resolve WordPress's URI attribute list.
///
/// Fast path: `StrListRef::Static(DEFAULT_URI_ATTRIBUTES)` when no filter.
/// Slow path: call `wp_kses_uri_attributes()` which fires the filter.
pub fn resolve_uri_attrs() -> StrListRef {
    if !has_filter("wp_kses_uri_attributes") {
        return StrListRef::Static(DEFAULT_URI_ATTRIBUTES);
    }

    let mut func = Zval::new();
    if func.set_string("wp_kses_uri_attributes", false).is_err() {
        return StrListRef::Static(DEFAULT_URI_ATTRIBUTES);
    }

    let result = match call_user_func!(func) {
        Ok(r) => r,
        Err(_) => return StrListRef::Static(DEFAULT_URI_ATTRIBUTES),
    };

    match result.array() {
        Some(arr) => StrListRef::Owned(arr.values().filter_map(|v| v.string()).collect()),
        None => StrListRef::Static(DEFAULT_URI_ATTRIBUTES),
    }
}
