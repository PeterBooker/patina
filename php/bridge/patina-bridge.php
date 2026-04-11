<?php
/**
 * Plugin Name: Patina Bridge
 * Description: Routes WordPress core function calls to Patina native implementations.
 * Version: 0.1.0
 *
 * This mu-plugin is only needed for non-pluggable function replacement (Phase 9+).
 * Pluggable functions are handled directly by the extension — no bridge needed.
 */

// Bail if extension not loaded — WordPress works normally without it.
if (!extension_loaded('patina-ext')) {
    return;
}

// Kill switch: define PATINA_DISABLE or set env var to bypass all replacements.
if (getenv('PATINA_DISABLE') || (defined('PATINA_DISABLE') && PATINA_DISABLE)) {
    return;
}

// --- Non-pluggable function interception goes here (Phase 9+) ---
// The mechanism (uopz, Zend function table, etc.) will be decided in Phase 8.
