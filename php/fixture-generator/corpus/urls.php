<?php
/**
 * URL test inputs for escaping/sanitization fixtures.
 */

function corpus_urls(): array {
    return [
        'http://example.com',
        'http://example.com/',
        'http://example.com/page',
        'http://example.com/path?key=value&other=1',
        'http://example.com/path?key=value&other=1#section',
        'http://example.com/path with spaces',
        'http://example.com/日本語/ページ',
        'http://example.com/<script>alert(1)</script>',
        'https://example.com:8080/path',
        'http://user:pass@example.com/path',
        '//example.com/protocol-relative',
        '/relative/path',
        'example.com/no-protocol',
        'javascript:alert(1)',
        'data:text/html,<h1>test</h1>',
        '',
        'ftp://files.example.com/file.txt',
        'http://example.com/' . str_repeat('a', 10000),
        "http://example.com/\x00page",
        "http://example.com/%00page",
    ];
}
