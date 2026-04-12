<?php

return [
    'name' => 'parse_blocks',
    'callable' => 'parse_blocks',
    'inputs' => [
        // Empty / trivial
        '',
        'plain text without any blocks',
        '   \n\t   ',

        // Single void blocks
        '<!-- wp:separator /-->',
        '<!-- wp:spacer /-->',
        '<!-- wp:nextpage /-->',

        // Void blocks with attrs
        '<!-- wp:image {"id":42,"sizeSlug":"large","linkDestination":"none"} /-->',
        '<!-- wp:spacer {"height":"50px"} /-->',

        // Single wrapped blocks (opener + content + closer)
        '<!-- wp:paragraph --><p>Hello, world.</p><!-- /wp:paragraph -->',
        '<!-- wp:heading --><h2>A heading</h2><!-- /wp:heading -->',
        '<!-- wp:heading {"level":3} --><h3>Level 3 heading</h3><!-- /wp:heading -->',

        // Multiple top-level blocks
        '<!-- wp:paragraph --><p>One</p><!-- /wp:paragraph --><!-- wp:paragraph --><p>Two</p><!-- /wp:paragraph -->',

        // Freeform text before, between, and after blocks
        'prefix<!-- wp:separator /-->',
        '<!-- wp:separator /-->suffix',
        'before<!-- wp:separator /-->middle<!-- wp:separator /-->after',
        'just text before<!-- wp:paragraph --><p>block</p><!-- /wp:paragraph -->just text after',

        // Nested blocks: group wrapping paragraph
        '<!-- wp:group --><div class="wp-block-group"><!-- wp:paragraph --><p>Nested paragraph</p><!-- /wp:paragraph --></div><!-- /wp:group -->',

        // Nested blocks: columns wrapping two columns each wrapping a paragraph
        '<!-- wp:columns --><div class="wp-block-columns"><!-- wp:column --><div class="wp-block-column"><!-- wp:paragraph --><p>Col 1</p><!-- /wp:paragraph --></div><!-- /wp:column --><!-- wp:column --><div class="wp-block-column"><!-- wp:paragraph --><p>Col 2</p><!-- /wp:paragraph --></div><!-- /wp:column --></div><!-- /wp:columns -->',

        // Deeply nested (3 levels)
        '<!-- wp:group --><div><!-- wp:group --><div><!-- wp:paragraph --><p>Deep</p><!-- /wp:paragraph --></div><!-- /wp:group --></div><!-- /wp:group -->',

        // Namespaced blocks
        '<!-- wp:my-plugin/custom-block /-->',
        '<!-- wp:my-plugin/custom-block {"foo":"bar"} /-->',
        '<!-- wp:acme/widget --><div class="acme-widget">content</div><!-- /wp:acme/widget -->',

        // Complex JSON attrs (nested objects, arrays, strings with special chars)
        '<!-- wp:group {"style":{"spacing":{"padding":{"top":"10px","bottom":"20px"}},"color":{"background":"#ffffff"}}} --><div>styled group</div><!-- /wp:group -->',
        '<!-- wp:image {"id":1,"sizeSlug":"full","linkDestination":"none","align":"wide","className":"is-style-rounded"} /-->',
        '<!-- wp:list {"ordered":true,"values":"<li>one</li><li>two</li>"} -->x<!-- /wp:list -->',

        // Blocks with numeric and boolean attrs
        '<!-- wp:separator {"opacity":"css","className":"is-style-wide"} /-->',
        '<!-- wp:embed {"url":"https://example.com/video","type":"video","providerNameSlug":"vimeo","responsive":true} /-->',

        // Real-world-ish post content mix
        '<!-- wp:paragraph --><p>Introduction paragraph.</p><!-- /wp:paragraph --><!-- wp:heading --><h2>Section 1</h2><!-- /wp:heading --><!-- wp:paragraph --><p>Body text goes here.</p><!-- /wp:paragraph --><!-- wp:separator /--><!-- wp:paragraph --><p>After the separator.</p><!-- /wp:paragraph -->',

        // Block with inner HTML containing angle brackets
        '<!-- wp:code --><pre class="wp-block-code"><code>if (x &lt; y) { ... }</code></pre><!-- /wp:code -->',

        // Multiple adjacent void blocks with no freeform between
        '<!-- wp:separator /--><!-- wp:spacer /--><!-- wp:separator /-->',

        // Block comment-like text that is NOT a block delimiter (should be freeform)
        '<!-- this is just a regular HTML comment --><!-- wp:paragraph --><p>real block</p><!-- /wp:paragraph -->',
        'text with <!-- wp:not-quite /> not a real close',

        // Malformed: unclosed opener at end of document
        '<!-- wp:paragraph --><p>Dangling with no closer',

        // Malformed: closer without opener
        '<p>Orphan content</p><!-- /wp:paragraph -->',
    ],
];
