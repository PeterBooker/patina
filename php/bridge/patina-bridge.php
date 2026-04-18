<?php
/**
 * Plugin Name: Patina Bridge
 * Description: Routes WordPress core function calls to Patina native implementations.
 * Version: 0.1.0
 */

// Bail if extension not loaded — WordPress works normally without it.
if (!extension_loaded('patina-ext')) {
    return;
}

/**
 * Kill switch: define PATINA_DISABLE or set env var to bypass all replacements.
 */
if (getenv('PATINA_DISABLE') || (defined('PATINA_DISABLE') && PATINA_DISABLE)) {
    return;
}

/**
 * Activation cache: after the first request in this FPM worker the Rust
 * side already holds the function-table swaps installed and the whole
 * skip-list construction below is dead work. A single PHP→Rust call to
 * `patina_is_activated()` reads a Rust-side AtomicBool and returns —
 * cheap enough to pay on every request, expensive enough to recover on
 * requests 2..N.
 *
 * `function_exists` guard keeps the bridge working with older builds of
 * the extension that predate `patina_is_activated`; on those it falls
 * through to the unconditional activation path below.
 */
if (function_exists('patina_is_activated') && patina_is_activated()) {
    return;
}

/**
 * Per-override toggles (Phase 3 of docs/BENCHMARK_PLAN.md).
 *
 * Each flag below maps to one or more Zend function-table swaps that
 * `patina_activate()` would otherwise install. Setting the flag (via env
 * var or PHP constant) appends the target names to a skip list, which
 * `patina_activate()` consults before swapping.
 *
 * Why: the bench runner needs to A/B individual overrides without
 * rebuilding the `.so` — otherwise decomposing patina's total effect
 * into per-override contributions means a full rebuild per configuration.
 *
 * Flags (env var OR constant, either works):
 *   PATINA_DISABLE_ESC            — skip esc_html + esc_attr
 *   PATINA_DISABLE_KSES           — skip wp_kses (and every wrapper)
 *   PATINA_DISABLE_PARSE_BLOCKS   — skip parse_blocks
 */
$patina_flag = static function (string $name): bool {
    if (getenv($name)) {
        return true;
    }
    return defined($name) && constant($name);
};

$patina_skip = [];
if ($patina_flag('PATINA_DISABLE_ESC')) {
    $patina_skip[] = 'esc_html';
    $patina_skip[] = 'esc_attr';
}
if ($patina_flag('PATINA_DISABLE_KSES')) {
    $patina_skip[] = 'wp_kses';
}
if ($patina_flag('PATINA_DISABLE_PARSE_BLOCKS')) {
    $patina_skip[] = 'parse_blocks';
}

// Activate non-pluggable function overrides.
// This swaps WordPress's esc_html, esc_attr, etc. in the Zend function table
// to point to Patina's Rust implementations.
//
// Must run AFTER WordPress core has defined these functions (mu-plugins load
// after wp-includes) and BEFORE theme/plugin code calls them (mu-plugins
// load before plugins and themes).
$patina_activated = patina_activate($patina_skip);
