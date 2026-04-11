<?php

declare(strict_types=1);

require_once __DIR__ . '/Runner.php';
require_once __DIR__ . '/reference/escaping.php';
require_once __DIR__ . '/reference/pluggable.php';

use Patina\Benchmarks\Runner;

if (!extension_loaded('patina-ext')) {
    fwrite(STDERR, "ERROR: patina extension not loaded.\n");
    exit(1);
}

$iterations = (int) ($argv[1] ?? 50_000);
$bench = new Runner($iterations);

// --- Escaping benchmarks ---

$inputs = [
    'tiny'    => '<b>hi</b>',
    'medium'  => str_repeat('Hello <b>world</b> & "quotes" ', 20),
    'large'   => str_repeat('Text with <script>alert("xss")</script> and &amp; entities. ', 200),
    'clean'   => str_repeat('Plain text with no special characters at all. ', 100),
];

foreach ($inputs as $label => $input) {
    $bench->run('esc_html', $label, 'reference_esc_html', 'patina_esc_html', [$input]);
}

foreach ($inputs as $label => $input) {
    $bench->run('esc_attr', $label, 'reference_esc_attr', 'patina_esc_attr', [$input]);
}

// --- Pluggable benchmarks ---

$urlInputs = [
    'simple'  => 'http://example.com/page',
    'spaces'  => 'http://example.com/my page/here today',
    'unicode' => 'http://example.com/日本語/ページ?q=テスト',
    'dirty'   => 'http://example.com/<script>alert(1)</script>?foo=bar&baz=qux',
];

foreach ($urlInputs as $label => $input) {
    $bench->run('wp_sanitize_redirect', $label, 'reference_wp_sanitize_redirect', 'wp_sanitize_redirect', [$input]);
}

// --- KSES benchmarks ---

if (function_exists('patina_wp_kses_post') && function_exists('wp_kses_post')) {
    $ksesInputs = [
        'small'  => '<p>Simple <b>paragraph</b> with <a href="http://example.com">link</a>.</p>',
        'medium' => str_repeat('<p>Paragraph <b>bold</b> and <a href="http://example.com">link</a>.</p>', 10),
        'script' => str_repeat('<p>Safe <script>alert("xss")</script> text</p>', 5),
        'large'  => str_repeat('<div class="container"><p>Content with <strong>formatting</strong>, <a href="http://example.com" title="Link">links</a>, and &amp; entities.</p></div>', 20),
    ];

    foreach ($ksesInputs as $label => $input) {
        $bench->run('wp_kses_post', $label, 'wp_kses_post', 'patina_wp_kses_post', [$input]);
    }
}

$bench->report();

// Print JIT status
echo PHP_EOL;
$jit = opcache_get_status(false)['jit'] ?? null;
if ($jit && $jit['enabled']) {
    printf("JIT: enabled (buffer: %s, opt_level: %s)\n",
        $jit['buffer_size'] ?? 'unknown',
        $jit['opt_level'] ?? 'unknown');
} else {
    echo "JIT: disabled\n";
}
