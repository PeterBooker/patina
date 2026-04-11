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

// Kill switch: define PATINA_DISABLE or set env var to bypass all replacements.
if (getenv('PATINA_DISABLE') || (defined('PATINA_DISABLE') && PATINA_DISABLE)) {
    return;
}

// Activate non-pluggable function overrides.
// This swaps WordPress's esc_html, esc_attr, etc. in the Zend function table
// to point to Patina's Rust implementations.
//
// Must run AFTER WordPress core has defined these functions (mu-plugins load
// after wp-includes) and BEFORE theme/plugin code calls them (mu-plugins
// load before plugins and themes).
$patina_activated = patina_activate();
