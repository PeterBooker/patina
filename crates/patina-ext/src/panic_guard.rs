use ext_php_rs::prelude::*;

/// Wraps a closure in `catch_unwind`, converting panics to PHP exceptions.
///
/// Every `#[php_function]` must route through this. A panic in a PHP extension
/// would unwind through C frames (undefined behavior) or abort the process.
/// This converts panics to catchable PHP exceptions instead.
pub fn guarded<F, T>(func_name: &str, f: F) -> PhpResult<T>
where
    F: FnOnce() -> T + std::panic::UnwindSafe,
{
    std::panic::catch_unwind(f)
        .map_err(|_| PhpException::default(format!("patina: internal error in {func_name}")))
}
