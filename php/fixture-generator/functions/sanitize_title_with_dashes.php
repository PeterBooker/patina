<?php
/**
 * Fixture definition for sanitize_title_with_dashes().
 *
 * Inputs are either:
 *   - a plain string — passed as the sole argument, so the function runs
 *     with its defaults ($raw_title='', $context='display'), or
 *   - an array [title, raw_title, context] — exercising the 'save' and
 *     'query' code paths that flip on extra replacement chains.
 */

return [
    'name'     => 'sanitize_title_with_dashes',
    'callable' => 'sanitize_title_with_dashes',
    'inputs'   => array_merge(
        corpus_strings(),
        [
            // Common display-context cases (one-arg calls).
            'Hello World',
            'A Long Title With  Multiple  Spaces',
            '   leading and trailing   ',
            '--dashes--at--edges--',
            'dots.in.the.title',
            'file/path/segments',
            'mix of ALL CASES',
            'CamelCasedTitle',
            'underscores_are_preserved',
            'numbers 12345 inline',
            '50% off sale',
            'a%20b%20c',                  // pre-encoded
            'stray % without octet',
            '%gg not hex %00 is hex',
            'a & b & c',
            'foo&amp;bar&#38;baz',
            '<em>emphasis</em> title',
            '<!-- comment -->visible',
            'unclosed <tag at end',
            'Ångström measurement',
            'Café résumé',
            'ÉLÉPHANT',
            '日本語のタイトル',
            '中文 文章 标题',
            'العربية عنوان',
            '🎉 party time 🎉',
            // Very long — exercises the 200-byte utf8_uri_encode cap.
            str_repeat('a', 400),
            str_repeat('日', 100),
            // Array form: [title, raw_title, context]
            ['Hello World', '', 'save'],
            ['Foo/Bar Baz', '', 'save'],
            ['10\xC3\x9710 = 100', '', 'save'],
            ["en\xE2\x80\x93dash", '', 'save'],
            ["em\xE2\x80\x94dash", '', 'save'],
            ["non\xC2\xA0breaking", '', 'save'],
            ['with &nbsp; entity', '', 'save'],
            ['with &mdash; entity', '', 'save'],
            ['Ångström', '', 'save'],
            // Display-context explicit (default)
            ['Hello World', '', 'display'],
            // Unusual context string — neither 'save' nor 'display'
            ['Hello World', '', 'query'],
            ['ÉLÉPHANT', 'Original', 'query'],
        ]
    ),
];
