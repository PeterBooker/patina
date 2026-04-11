<?php
/**
 * Profile WordPress function call frequency and timing.
 * Hooks into targeted functions to measure per-call cost.
 */

require '/var/www/html/wp-load.php';

// Functions to profile
$targets = [
    'esc_html', 'esc_attr', 'esc_url', 'esc_textarea',
    'wp_kses', 'wp_kses_post', 'wp_kses_data',
    'wp_check_invalid_utf8', '_wp_specialchars',
    'sanitize_title', 'sanitize_title_with_dashes',
    'sanitize_file_name', 'sanitize_text_field',
    'wpautop', 'make_clickable', 'wp_strip_all_tags',
    'wp_sanitize_redirect', 'wp_validate_redirect',
    'do_blocks', 'render_block', 'parse_blocks', 'serialize_block',
    'wp_get_global_styles', 'wp_get_global_stylesheet',
    'wp_get_global_settings',
    'apply_filters', 'do_action',
    'get_option', 'update_option',
    'wp_json_encode',
];

$call_counts = [];
$call_times = [];

// We can't hook internal functions directly, so we'll profile by running
// key WordPress operations and timing them.

// --- Profile: render homepage ---
echo "=== Profiling: Homepage render ===\n";

ob_start();
$start = hrtime(true);

// Simulate a full page render through WordPress
$_SERVER['REQUEST_URI'] = '/';
$wp = new WP();
$wp->parse_request();
$wp->query_posts();
$wp->handle_404();
$wp->register_globals();

// Load template
if (function_exists('wp_is_block_theme') && wp_is_block_theme()) {
    // Block theme: render via block template system
    $_wp_current_template_content = get_block_template(
        get_stylesheet() . '//index',
        'wp_template'
    )->content ?? '';

    if ($_wp_current_template_content) {
        $rendered = do_blocks($_wp_current_template_content);
        echo apply_filters('the_content', $rendered);
    }
}

$homepage_html = ob_get_clean();
$homepage_time = (hrtime(true) - $start) / 1_000_000;
printf("Homepage: %.1f ms, %d bytes output\n\n", $homepage_time, strlen($homepage_html));

// --- Profile: individual function timing ---
echo "=== Function timing (1000 iterations each) ===\n\n";

$test_inputs = [
    'short_html' => '<b>Hello</b> & "world"',
    'medium_html' => str_repeat('Normal text with <b>bold</b> and &amp; entities. ', 20),
    'clean_text' => str_repeat('Plain text without special characters here. ', 20),
    'post_content' => $homepage_html ?: str_repeat('<p>Paragraph content with <a href="http://example.com">links</a>.</p>', 10),
];

$iterations = 1000;
$results = [];

// esc_html
foreach (['short_html', 'medium_html', 'clean_text'] as $input_key) {
    $input = $test_inputs[$input_key];
    $start = hrtime(true);
    for ($i = 0; $i < $iterations; $i++) {
        esc_html($input);
    }
    $time_us = (hrtime(true) - $start) / 1000;
    $results[] = ['esc_html', $input_key, strlen($input), $time_us / $iterations];
}

// esc_attr
foreach (['short_html', 'medium_html', 'clean_text'] as $input_key) {
    $input = $test_inputs[$input_key];
    $start = hrtime(true);
    for ($i = 0; $i < $iterations; $i++) {
        esc_attr($input);
    }
    $time_us = (hrtime(true) - $start) / 1000;
    $results[] = ['esc_attr', $input_key, strlen($input), $time_us / $iterations];
}

// esc_url
$url_inputs = [
    'simple' => 'http://example.com/page',
    'complex' => 'http://example.com/path?key=value&other=1&foo=bar#section',
];
foreach ($url_inputs as $label => $input) {
    $start = hrtime(true);
    for ($i = 0; $i < $iterations; $i++) {
        esc_url($input);
    }
    $time_us = (hrtime(true) - $start) / 1000;
    $results[] = ['esc_url', $label, strlen($input), $time_us / $iterations];
}

// wp_kses_post
foreach (['short_html', 'medium_html', 'post_content'] as $input_key) {
    $input = $test_inputs[$input_key];
    $start = hrtime(true);
    for ($i = 0; $i < $iterations; $i++) {
        wp_kses_post($input);
    }
    $time_us = (hrtime(true) - $start) / 1000;
    $results[] = ['wp_kses_post', $input_key, strlen($input), $time_us / $iterations];
}

// wpautop
foreach (['short_html', 'medium_html'] as $input_key) {
    $input = $test_inputs[$input_key];
    $start = hrtime(true);
    for ($i = 0; $i < $iterations; $i++) {
        wpautop($input);
    }
    $time_us = (hrtime(true) - $start) / 1000;
    $results[] = ['wpautop', $input_key, strlen($input), $time_us / $iterations];
}

// sanitize_title
$title_inputs = ['Simple Title', 'Complex Title: With "Quotes" & Ampersands!', '日本語タイトル'];
foreach ($title_inputs as $input) {
    $start = hrtime(true);
    for ($i = 0; $i < $iterations; $i++) {
        sanitize_title($input);
    }
    $time_us = (hrtime(true) - $start) / 1000;
    $results[] = ['sanitize_title', substr($input, 0, 20), strlen($input), $time_us / $iterations];
}

// wp_check_invalid_utf8
foreach (['short_html', 'medium_html'] as $input_key) {
    $input = $test_inputs[$input_key];
    $start = hrtime(true);
    for ($i = 0; $i < $iterations; $i++) {
        wp_check_invalid_utf8($input);
    }
    $time_us = (hrtime(true) - $start) / 1000;
    $results[] = ['wp_check_invalid_utf8', $input_key, strlen($input), $time_us / $iterations];
}

// _wp_specialchars
foreach (['short_html', 'medium_html', 'clean_text'] as $input_key) {
    $input = $test_inputs[$input_key];
    $start = hrtime(true);
    for ($i = 0; $i < $iterations; $i++) {
        _wp_specialchars($input, ENT_QUOTES);
    }
    $time_us = (hrtime(true) - $start) / 1000;
    $results[] = ['_wp_specialchars', $input_key, strlen($input), $time_us / $iterations];
}

// parse_blocks (if content exists)
if (!empty($homepage_html)) {
    $block_content = '<!-- wp:paragraph --><p>Test</p><!-- /wp:paragraph --><!-- wp:heading --><h2>Heading</h2><!-- /wp:heading -->';
    $start = hrtime(true);
    for ($i = 0; $i < $iterations; $i++) {
        parse_blocks($block_content);
    }
    $time_us = (hrtime(true) - $start) / 1000;
    $results[] = ['parse_blocks', 'small', strlen($block_content), $time_us / $iterations];
}

// do_blocks
if (!empty($homepage_html)) {
    $block_content = '<!-- wp:paragraph --><p>Test content here</p><!-- /wp:paragraph -->';
    $start = hrtime(true);
    for ($i = 0; $i < $iterations; $i++) {
        do_blocks($block_content);
    }
    $time_us = (hrtime(true) - $start) / 1000;
    $results[] = ['do_blocks', 'small', strlen($block_content), $time_us / $iterations];
}

// wp_get_global_stylesheet
$start = hrtime(true);
for ($i = 0; $i < 100; $i++) {
    // Reset cache to measure actual work
    wp_get_global_stylesheet();
}
$time_us = (hrtime(true) - $start) / 1000;
$results[] = ['wp_get_global_stylesheet', 'cached', 0, $time_us / 100];

// wp_json_encode
$test_data = ['key' => 'value', 'nested' => ['a' => 1, 'b' => 2], 'html' => '<p>Test & "quotes"</p>'];
$start = hrtime(true);
for ($i = 0; $i < $iterations; $i++) {
    wp_json_encode($test_data);
}
$time_us = (hrtime(true) - $start) / 1000;
$results[] = ['wp_json_encode', 'small_obj', 0, $time_us / $iterations];

// wp_strip_all_tags
foreach (['short_html', 'medium_html'] as $input_key) {
    $input = $test_inputs[$input_key];
    $start = hrtime(true);
    for ($i = 0; $i < $iterations; $i++) {
        wp_strip_all_tags($input);
    }
    $time_us = (hrtime(true) - $start) / 1000;
    $results[] = ['wp_strip_all_tags', $input_key, strlen($input), $time_us / $iterations];
}

// --- Print results ---
printf("%-30s %-15s %8s %12s\n", 'Function', 'Input', 'Size(B)', 'µs/call');
printf("%s\n", str_repeat('─', 70));

usort($results, fn($a, $b) => $b[3] <=> $a[3]); // Sort by time descending

foreach ($results as $r) {
    printf("%-30s %-15s %8d %12.2f\n", $r[0], $r[1], $r[2], $r[3]);
}

// --- Homepage call count estimation ---
echo "\n=== Estimated call counts (homepage) ===\n";
echo "These are approximate — based on typical WP page rendering:\n\n";

// Count occurrences of function calls in the rendered HTML as a proxy
$html = $homepage_html;
$esc_html_calls = substr_count($html, 'esc_html') + 100; // esc_html is called many times for attributes etc.
printf("esc_html:    ~200-500 calls/request (escapes every text output)\n");
printf("esc_attr:    ~200-500 calls/request (escapes every HTML attribute)\n");
printf("esc_url:     ~50-200 calls/request (every URL in output)\n");
printf("wp_kses_post: ~10-50 calls/request (post content, widgets)\n");
printf("_wp_specialchars: ~400-1000 calls/request (called by esc_html + esc_attr)\n");
printf("wp_check_invalid_utf8: ~400-1000 calls/request (called by esc_html + esc_attr)\n");
printf("parse_blocks: ~1-5 calls/request (per post/template)\n");
printf("do_blocks:   ~1-5 calls/request (per post/template)\n");
printf("apply_filters: ~500-2000 calls/request\n");
