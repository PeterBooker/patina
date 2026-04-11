<?php
/**
 * General string test inputs, used across multiple function fixture generators.
 */

function corpus_strings(): array {
    return [
        '',
        'hello world',
        'Plain text with no special characters at all.',
        "Multi\nline\ninput",
        "Tabs\there\tand\there",
        str_repeat('a', 1000),
        str_repeat('a', 100000),

        // Control characters
        "\x00\x01\x02\x03\x04\x05\x06\x07\x08",
        "null\x00byte",
        "text%00with%00encoded%00nulls",

        // Unicode / multibyte
        'Ångström measurement',
        '日本語テスト',
        '中文测试文本',
        'العربية',
        '🎉🚀💻🔥',
        "Mixed ASCII and 日本語 text",
        "\xC3\xA9\xC3\xA0\xC3\xBC", // éàü
    ];
}
