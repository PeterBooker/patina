//! Utilities for calling PHP functions from Rust.
//!
//! Used by functions that need to call `apply_filters()` or access WordPress state.
//! The Rust→PHP→Rust re-entrant call path must be tested carefully.

use ext_php_rs::convert::IntoZvalDyn;
use ext_php_rs::prelude::*;
use ext_php_rs::types::Zval;

/// Call a PHP function by name with the given arguments.
pub fn call_php_func(name: &str, args: Vec<&dyn IntoZvalDyn>) -> PhpResult<Zval> {
    let mut func = Zval::new();
    func.set_string(name, false).map_err(|e| {
        PhpException::default(format!("patina: failed to set function name '{name}': {e}"))
    })?;
    func.try_call(args)
        .map_err(|e| PhpException::default(format!("patina: call to '{name}' failed: {e}")))
}
