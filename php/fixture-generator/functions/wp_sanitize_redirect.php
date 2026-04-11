<?php
/**
 * Fixture definition for wp_sanitize_redirect().
 */

return [
    'name' => 'wp_sanitize_redirect',
    'callable' => 'wp_sanitize_redirect',
    'inputs' => array_merge(
        corpus_strings(),
        corpus_urls(),
        [
            // Function-specific edge cases
            'http://example.com/path?a=1&b=2',
            'http://example.com/%E6%97%A5%E6%9C%AC', // pre-encoded unicode
            "http://example.com/\\0null",
            'http://example.com/path with "quotes"',
            'HTTP://EXAMPLE.COM/UPPERCASE',
            'http://example.com/tilde~underscore_dash-dot.ext',
            'http://example.com/brackets[0]',
            'http://example.com/parens(1)',
            'http://example.com/at@sign',
            'http://example.com/exclaim!',
            'http://example.com/star*glob',
            'http://example.com/semicolon;param',
            'http://example.com/comma,separated',
        ]
    ),
];
