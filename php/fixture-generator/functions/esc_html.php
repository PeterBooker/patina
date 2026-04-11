<?php

return [
    'name' => 'esc_html',
    'callable' => 'esc_html',
    'inputs' => array_merge(
        corpus_strings(),
        corpus_html(),
        [
            // esc_html-specific edge cases
            '&amp;amp; double reference',
            '&#38; numeric ampersand',
            '&#x26; hex ampersand',
            '&nosemicolon',
            '&invalid;entity',
            '& bare ampersand at end',
            'text & more & text',
            '<div class="foo">bar</div>',
            "line1\nline2\nline3",
            "tab\there",
            str_repeat('&amp;', 1000),
            str_repeat('<', 1000),
        ]
    ),
];
