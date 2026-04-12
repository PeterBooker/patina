<?php
/**
 * Fixture generator: runs WordPress functions against a test corpus
 * and outputs JSON fixtures for use by Rust and PHP tests.
 *
 * Usage:
 *   php generate.php --function=wp_sanitize_redirect
 *   php generate.php --all
 *   php generate.php --all --output=../../fixtures/
 *
 * Must be run within a WordPress environment (the profiling Docker stack).
 */

// Ensure we're in a WordPress context. We can't check for a specific WP
// function here because patina-ext registers some of them — check for
// ABSPATH instead, which is only defined by wp-load.php itself.
if (!defined('ABSPATH')) {
    $wp_load = getenv('WP_LOAD_PATH') ?: '/var/www/html/wp-load.php';
    if (file_exists($wp_load)) {
        require_once $wp_load;
    } else {
        die("ERROR: WordPress not found. Run this inside the profiling Docker stack.\n");
    }
}

$options = getopt('', ['function:', 'all', 'output:']);
$output_dir = $options['output'] ?? __DIR__ . '/../../fixtures';

if (!is_dir($output_dir)) {
    mkdir($output_dir, 0755, true);
}

// Load corpus
require_once __DIR__ . '/corpus/strings.php';
require_once __DIR__ . '/corpus/urls.php';
require_once __DIR__ . '/corpus/html.php';

// Load function definitions
$function_files = glob(__DIR__ . '/functions/*.php');
$available_functions = [];
foreach ($function_files as $file) {
    $def = require $file;
    $available_functions[$def['name']] = $def;
}

// Determine which functions to generate
if (isset($options['all'])) {
    $targets = array_keys($available_functions);
} elseif (isset($options['function'])) {
    $targets = [$options['function']];
} else {
    die("Usage: php generate.php --function=<name> | --all\n");
}

foreach ($targets as $target) {
    if (!isset($available_functions[$target])) {
        fprintf(STDERR, "WARNING: No fixture definition for '%s', skipping.\n", $target);
        continue;
    }

    $def = $available_functions[$target];
    $fixtures = [];

    foreach ($def['inputs'] as $input) {
        $args = (array)$input;
        $output = call_user_func_array($def['callable'], $args);
        $fixtures[] = [
            'input' => $args,
            'output' => $output,
        ];
    }

    $path = $output_dir . '/' . $target . '.json';
    file_put_contents($path, json_encode($fixtures, JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE) . "\n");
    fprintf(STDERR, "Generated %d fixtures for %s -> %s\n", count($fixtures), $target, $path);
}
