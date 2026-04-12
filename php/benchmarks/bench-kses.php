<?php
/**
 * Benchmark the wp_kses family: stock WP PHP vs Patina (shim → Rust) vs raw Patina.
 *
 * Patina is activated via the mu-plugin during wp-load.php. The mu-plugin
 * swaps `wp_kses` in the function table to a PHP user-function shim
 * (`__patina_wp_kses_shim__`) that trampolines to `patina_wp_kses_internal`
 * — this avoids the DO_UCALL→internal crash that affects direct swaps.
 *
 * Three variants per input are measured:
 *  1. wp_kses_post($x)   — goes through wp_kses → shim → Rust
 *  2. wp_kses($x, 'post') — direct call, goes through shim → Rust
 *  3. patina_wp_kses_post($x) — direct Rust, bypasses shim + filter bridge
 *
 * Run with PATINA_DISABLE=1 env var for the pure-PHP baseline.
 */

require '/var/www/html/wp-load.php';

if (!function_exists('patina_wp_kses_post')) {
    fwrite(STDERR, "ERROR: patina_wp_kses_post not available. Is the extension loaded?\n");
    exit(1);
}

$iterations = (int) ($argv[1] ?? 10000);

$inputs = [
    'small_76B'   => '<p>Simple <b>paragraph</b> with <a href="http://example.com">link</a>.</p>',
    'medium_740B' => str_repeat('<p>Paragraph <b>bold</b> and <a href="http://example.com">link</a>.</p>', 10),
    'with_script' => str_repeat('<p>Safe <script>alert("xss")</script> text</p>', 5),
    'large_3KB'   => str_repeat('<div class="container"><p>Content with <strong>formatting</strong>, <a href="http://example.com" title="Link">links</a>, and &amp; entities.</p></div>', 20),
];

function timed(callable $fn, int $iterations): float {
    for ($i = 0; $i < min(100, $iterations); $i++) {
        $fn();
    }
    $start = hrtime(true);
    for ($i = 0; $i < $iterations; $i++) {
        $fn();
    }
    return (hrtime(true) - $start) / 1_000_000; // ms
}

$is_overridden = in_array('wp_kses', patina_status(), true);

$rows = [];
foreach ($inputs as $label => $input) {
    $post_ms  = timed(fn() => wp_kses_post($input), $iterations);
    $kses_ms  = timed(fn() => wp_kses($input, 'post'), $iterations);
    $raw_ms   = timed(fn() => patina_wp_kses_post($input), $iterations);

    $rows[] = [
        'input'     => $label,
        'post_ms'   => $post_ms,
        'kses_ms'   => $kses_ms,
        'raw_ms'    => $raw_ms,
    ];
}

$mode = $is_overridden
    ? 'Patina-on: wp_kses → shim → patina_wp_kses_internal → Rust'
    : 'Stock PHP: wp_kses family';
printf("\nMode: %s\n", $mode);
printf("%-15s %18s %18s %15s\n",
    'Input', 'wp_kses_post (ms)', 'wp_kses (ms)', 'raw patina (ms)');
printf("%s\n", str_repeat('─', 75));
foreach ($rows as $r) {
    printf("%-15s %15.2f ms %15.2f ms %12.2f ms\n",
        $r['input'], $r['post_ms'], $r['kses_ms'], $r['raw_ms']);
}
printf("\nIterations per row: %s\n", number_format($iterations));
