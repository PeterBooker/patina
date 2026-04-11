//! WordPress pluggable function replacements.
//!
//! These functions are defined in `wp-includes/pluggable.php` with
//! `function_exists()` guards. The extension registers them at MINIT
//! (before any PHP script runs), so WordPress skips its PHP definitions.
//!
//! No mu-plugin bridge is needed for these — the extension handles it directly.

pub mod sanitize_redirect;
pub mod validate_redirect;

pub use sanitize_redirect::sanitize_redirect;
pub use validate_redirect::{validate_redirect, ValidateResult};
