//! Bridge between patina-core's `ParsedBlock` tree and PHP arrays.
//!
//! Converts the Rust block tree into the same ZendHashTable shape that
//! WordPress's `WP_Block_Parser::parse()` produces when its output
//! (an array of `WP_Block_Parser_Block` objects) is cast to an array.
//!
//! Each block becomes an associative array with these keys in this order
//! (matching PHP's property declaration order in `WP_Block_Parser_Block`):
//!
//! - `blockName`    — string or null
//! - `attrs`        — associative array (from JSON object) or empty array
//! - `innerBlocks`  — sequential array of nested blocks
//! - `innerHTML`    — string
//! - `innerContent` — sequential array of strings and null placeholders

use ext_php_rs::boxed::ZBox;
use ext_php_rs::types::{ZendHashTable, Zval};
use patina_core::blocks::types::ParsedBlock;
use serde_json::Value as JsonValue;

/// Convert a slice of `ParsedBlock`s into a PHP sequential array.
pub fn blocks_to_php_array(blocks: &[ParsedBlock]) -> ZBox<ZendHashTable> {
    let mut out = ZendHashTable::with_capacity(blocks.len() as u32);
    for block in blocks {
        let zval = block_to_zval(block);
        let _ = out.push(zval);
    }
    out
}

/// Convert a single `ParsedBlock` into a PHP zval wrapping an associative
/// array.
fn block_to_zval(block: &ParsedBlock) -> Zval {
    let mut arr = ZendHashTable::with_capacity(5);

    // blockName: string or null
    let _ = match &block.block_name {
        Some(name) => arr.insert("blockName", name.as_str()),
        None => arr.insert("blockName", ()),
    };

    // attrs: associative array from the JSON value
    let _ = arr.insert("attrs", json_value_to_zval(&block.attrs));

    // innerBlocks: recursive
    let _ = arr.insert("innerBlocks", blocks_to_php_array(&block.inner_blocks));

    // innerHTML
    let _ = arr.insert("innerHTML", block.inner_html.as_str());

    // innerContent: array of strings and nulls
    let mut ic = ZendHashTable::with_capacity(block.inner_content.len() as u32);
    for item in &block.inner_content {
        let _ = match item {
            Some(s) => ic.push(s.as_str()),
            None => ic.push(()),
        };
    }
    let _ = arr.insert("innerContent", ic);

    let mut zv = Zval::new();
    zv.set_hashtable(arr);
    zv
}

/// Convert a `serde_json::Value` into a PHP zval. Preserves the
/// distinction between empty array `[]` (used for no-attrs blocks) and
/// empty object `{}` — both become empty PHP arrays, which is correct:
/// PHP has no separate empty-object type and cannot represent an empty
/// associative array distinct from an empty sequential one.
fn json_value_to_zval(value: &JsonValue) -> Zval {
    let mut zv = Zval::new();
    match value {
        JsonValue::Null => zv.set_null(),
        JsonValue::Bool(b) => zv.set_bool(*b),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                zv.set_long(i);
            } else if let Some(f) = n.as_f64() {
                zv.set_double(f);
            } else {
                // Arbitrary-precision number that fits neither i64 nor f64 —
                // fall through to string representation.
                zv.set_string(&n.to_string(), false).ok();
            }
        }
        JsonValue::String(s) => {
            zv.set_string(s, false).ok();
        }
        JsonValue::Array(items) => {
            let mut ht = ZendHashTable::with_capacity(items.len() as u32);
            for item in items {
                let _ = ht.push(json_value_to_zval(item));
            }
            zv.set_hashtable(ht);
        }
        JsonValue::Object(map) => {
            let mut ht = ZendHashTable::with_capacity(map.len() as u32);
            for (key, item) in map {
                let key_str: &str = key;
                let _ = ht.insert(key_str, json_value_to_zval(item));
            }
            zv.set_hashtable(ht);
        }
    }
    zv
}
