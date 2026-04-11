<?php
/**
 * HTML test inputs for escaping/kses fixtures.
 */

function corpus_html(): array {
    return [
        // Tags
        '<script>alert("xss")</script>',
        '<div class="foo"><span>nested</span></div>',
        '<img src="x" onerror="alert(1)">',
        '<a href="http://example.com">link</a>',
        '<p>Simple paragraph</p>',
        '<br /><hr />',
        '<!-- comment -->',
        '<b>bold</b> and <i>italic</i>',

        // Entities - already encoded (must NOT double-encode)
        '&amp; is an ampersand',
        '&lt;not a tag&gt;',
        '&quot;quoted&quot;',
        '&#039;single quotes&#039;',
        '&#x41; is A',
        '&amp;amp; double-encoded already',

        // Bare special characters
        'foo & bar',
        'a < b > c',
        'say "hello"',
        "it's fine",
        'mix & match <b>bold</b> & "quotes"',

        // Multibyte with HTML
        '日本語 <b>太字</b>',
        'Ångström <b>bold</b>',
        '🎉 <script>alert("emoji")</script>',

        // Deeply nested
        '<div><div><div><div><div>deep</div></div></div></div></div>',

        // Malformed
        '<div class="unclosed',
        '< not really a tag >',
        '<>',
        '</closing-only>',
        '<div attr=\'single\' attr2="double">text</div>',
    ];
}
