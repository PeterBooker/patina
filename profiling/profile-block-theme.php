<?php
/**
 * Profile block theme and theme.json hot paths.
 */

require '/var/www/html/wp-load.php';

$iterations = 100;
$results = [];

// --- Block template rendering ---
echo "=== Block Theme Rendering ===\n\n";

// Get the block template content
$template = get_block_template(get_stylesheet() . '//index', 'wp_template');
$template_content = $template->content ?? '';
printf("Template size: %d bytes\n", strlen($template_content));

// Get block-heavy post content
$post = get_page_by_path('block-heavy-test', OBJECT, 'post');
$post_content = $post ? $post->post_content : '';
printf("Post content size: %d bytes\n\n", strlen($post_content));

// parse_blocks on template
$start = hrtime(true);
for ($i = 0; $i < $iterations; $i++) {
    parse_blocks($template_content);
}
$time = (hrtime(true) - $start) / 1000;
$results[] = ['parse_blocks', 'template', strlen($template_content), $time / $iterations];

// parse_blocks on post content
if ($post_content) {
    $start = hrtime(true);
    for ($i = 0; $i < $iterations; $i++) {
        parse_blocks($post_content);
    }
    $time = (hrtime(true) - $start) / 1000;
    $results[] = ['parse_blocks', 'post_content', strlen($post_content), $time / $iterations];
}

// do_blocks on template
$start = hrtime(true);
for ($i = 0; $i < $iterations; $i++) {
    do_blocks($template_content);
}
$time = (hrtime(true) - $start) / 1000;
$results[] = ['do_blocks', 'template', strlen($template_content), $time / $iterations];

// render the full template like WP would
$start = hrtime(true);
for ($i = 0; $i < 10; $i++) {
    ob_start();
    echo do_blocks($template_content);
    ob_get_clean();
}
$time = (hrtime(true) - $start) / 1000;
$results[] = ['full_template_render', '10 iterations', strlen($template_content), $time / 10];

// --- theme.json / Global Styles ---
echo "=== theme.json / Global Styles ===\n\n";

// wp_get_global_settings
$start = hrtime(true);
for ($i = 0; $i < $iterations; $i++) {
    wp_get_global_settings();
}
$time = (hrtime(true) - $start) / 1000;
$results[] = ['wp_get_global_settings', 'full', 0, $time / $iterations];

// wp_get_global_styles
$start = hrtime(true);
for ($i = 0; $i < $iterations; $i++) {
    wp_get_global_styles();
}
$time = (hrtime(true) - $start) / 1000;
$results[] = ['wp_get_global_styles', 'full', 0, $time / $iterations];

// wp_get_global_stylesheet
$start = hrtime(true);
for ($i = 0; $i < $iterations; $i++) {
    wp_get_global_stylesheet();
}
$time = (hrtime(true) - $start) / 1000;
$results[] = ['wp_get_global_stylesheet', 'full', 0, $time / $iterations];

// WP_Theme_JSON resolution
if (class_exists('WP_Theme_JSON_Resolver')) {
    $start = hrtime(true);
    for ($i = 0; $i < $iterations; $i++) {
        WP_Theme_JSON_Resolver::get_merged_data();
    }
    $time = (hrtime(true) - $start) / 1000;
    $results[] = ['WP_Theme_JSON_Resolver::get_merged_data', 'full', 0, $time / $iterations];
}

// --- Escaping functions with real block output ---
echo "=== Escaping on block-rendered content ===\n\n";

$rendered = do_blocks($template_content);
$rendered_size = strlen($rendered);
printf("Rendered template: %d bytes\n\n", $rendered_size);

$start = hrtime(true);
for ($i = 0; $i < $iterations; $i++) {
    esc_html($rendered);
}
$time = (hrtime(true) - $start) / 1000;
$results[] = ['esc_html', 'rendered_template', $rendered_size, $time / $iterations];

$start = hrtime(true);
for ($i = 0; $i < $iterations; $i++) {
    wp_kses_post($rendered);
}
$time = (hrtime(true) - $start) / 1000;
$results[] = ['wp_kses_post', 'rendered_template', $rendered_size, $time / $iterations];

// --- WP_HTML_Tag_Processor (block theme's main HTML processor) ---
if (class_exists('WP_HTML_Tag_Processor')) {
    $start = hrtime(true);
    for ($i = 0; $i < $iterations; $i++) {
        $proc = new WP_HTML_Tag_Processor($rendered);
        while ($proc->next_tag()) {
            $proc->get_tag();
        }
    }
    $time = (hrtime(true) - $start) / 1000;
    $results[] = ['WP_HTML_Tag_Processor', 'rendered_template', $rendered_size, $time / $iterations];
}

// --- Print results ---
printf("\n%-45s %-20s %8s %12s\n", 'Function', 'Input', 'Size(B)', 'µs/call');
printf("%s\n", str_repeat('─', 90));

usort($results, fn($a, $b) => $b[3] <=> $a[3]);

foreach ($results as $r) {
    printf("%-45s %-20s %8d %12.1f\n", $r[0], $r[1], $r[2], $r[3]);
}
