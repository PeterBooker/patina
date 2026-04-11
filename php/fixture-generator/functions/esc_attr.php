<?php

return [
    'name' => 'esc_attr',
    'callable' => 'esc_attr',
    'inputs' => array_merge(
        corpus_strings(),
        corpus_html(),
        [
            // esc_attr-specific edge cases
            '" onclick="alert(1)',
            "' onfocus='alert(1)",
            'value" class="injected',
            "mixed 'quotes\" and <tags>",
        ]
    ),
];
