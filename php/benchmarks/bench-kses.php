<?php
/**
 * Benchmark wp_kses_post: WordPress PHP vs Patina Rust.
 * Must be run inside a WordPress environment with the Patina extension loaded.
 */

require '/var/www/html/wp-load.php';
require __DIR__ . '/Runner.php';

use Patina\Benchmarks\Runner;

if (!function_exists('patina_wp_kses_post')) {
    fwrite(STDERR, "ERROR: patina_wp_kses_post not available. Is the extension loaded?\n");
    exit(1);
}

$iterations = (int) ($argv[1] ?? 10000);
$bench = new Runner($iterations);

$inputs = [
    'small_76B'   => '<p>Simple <b>paragraph</b> with <a href="http://example.com">link</a>.</p>',
    'medium_740B' => str_repeat('<p>Paragraph <b>bold</b> and <a href="http://example.com">link</a>.</p>', 10),
    'with_script' => str_repeat('<p>Safe <script>alert("xss")</script> text</p>', 5),
    'large_3KB'   => str_repeat('<div class="container"><p>Content with <strong>formatting</strong>, <a href="http://example.com" title="Link">links</a>, and &amp; entities.</p></div>', 20),
];

foreach ($inputs as $label => $input) {
    $bench->run('wp_kses_post', $label, 'wp_kses_post', 'patina_wp_kses_post', [$input]);
}

$bench->report();
