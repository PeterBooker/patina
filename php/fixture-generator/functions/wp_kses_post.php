<?php

return [
    'name' => 'wp_kses_post',
    'callable' => 'wp_kses_post',
    'inputs' => array_merge(
        corpus_strings(),
        corpus_html(),
        [
            // Allowed tags with attributes
            '<a href="http://example.com" title="Link">click</a>',
            '<a href="javascript:alert(1)">xss</a>',
            '<a href="http://example.com" onclick="alert(1)">click</a>',
            '<div class="container" id="main"><p>content</p></div>',
            '<img src="http://example.com/img.jpg" alt="Test" />',

            // Disallowed tags
            '<script>alert("xss")</script>',
            '<iframe src="http://evil.com"></iframe>',
            '<form action="http://evil.com"><input type="text"></form>',

            // Mixed allowed and disallowed
            '<p>Safe <script>unsafe</script> safe again</p>',
            '<div class="ok"><script>bad</script><b>bold</b></div>',

            // Attribute filtering
            '<div data-custom="value">custom data attr</div>',
            '<span aria-label="accessible">text</span>',

            // Comments
            '<!-- This is a comment -->',
            '<!-- safe comment text -->',

            // Self-closing and XHTML
            '<br />',
            '<br/>',
            '<hr />',
            '<img src="test.jpg" />',

            // Nested structures
            '<blockquote><p>Quoted <strong>text</strong></p></blockquote>',
            '<ul><li>Item 1</li><li>Item 2</li></ul>',
            '<table><tr><td>Cell</td></tr></table>',

            // Edge cases
            '<>',
            '< >',
            '< div>content</div>',
            '<div  class = "spaced"  >content</div>',
            '<DIV CLASS="upper">content</DIV>',
            '',
            'Plain text with no HTML at all',

            // Large input
            str_repeat('<p>Paragraph <b>bold</b> and <a href="http://example.com">link</a>.</p>', 50),

            // Block-style content
            '<!-- wp:paragraph --><p>Block content with <a href="http://example.com">links</a> and <strong>formatting</strong>.</p><!-- /wp:paragraph -->',

            // Entities
            '<p>&amp; &lt; &gt; &quot; &#039;</p>',
            '<p>&#38; &#60; &#62;</p>',

            // Protocol checking in attributes
            '<a href="http://safe.com">http</a>',
            '<a href="https://safe.com">https</a>',
            '<a href="ftp://files.com">ftp</a>',
            '<a href="javascript:void(0)">js</a>',
            '<a href="vbscript:msgbox">vbs</a>',
            '<a href="mailto:user@example.com">email</a>',
            '<a href="tel:+1234567890">phone</a>',
        ]
    ),
];
