<?php

/**
 * Reference implementations of WordPress pluggable functions.
 * Copied verbatim from wp-includes/pluggable.php.
 */

/**
 * Reference wp_sanitize_redirect — from wp-includes/pluggable.php
 */
function reference_wp_sanitize_redirect(string $location): string
{
    // Encode spaces.
    $location = str_replace(' ', '%20', $location);

    $regex = '/
        (
            (?: [\xC2-\xDF][\x80-\xBF]        # double-byte sequences   110xxxxx 10xxxxxx
            |   \xE0[\xA0-\xBF][\x80-\xBF]     # triple-byte sequences   1110xxxx 10xxxxxx * 2
            |   [\xE1-\xEC][\x80-\xBF]{2}
            |   \xED[\x80-\x9F][\x80-\xBF]
            |   [\xEE-\xEF][\x80-\xBF]{2}
            |   \xF0[\x90-\xBF][\x80-\xBF]{2}  # four-byte sequences    11110xxx 10xxxxxx * 3
            |   [\xF1-\xF3][\x80-\xBF]{3}
            |   \xF4[\x80-\x8F][\x80-\xBF]{2}
        ){1,40}                              # ...one or more times
        )/x';
    $location = preg_replace_callback($regex, 'reference__wp_sanitize_utf8_in_redirect', $location);
    $location = preg_replace('|[^a-z0-9-~+_.?#=&;,/:%!*\[\]()@]|i', '', $location);

    $location = reference_wp_kses_no_null($location);

    return $location;
}

function reference__wp_sanitize_utf8_in_redirect(array $matches): string
{
    return urlencode($matches[0]);
}

function reference_wp_kses_no_null(string $content, array $options = []): string
{
    $content = preg_replace('/[\x00-\x08\x0B\x0C\x0E-\x1F]/', '', $content);

    if (empty($options['slash_zero'])) {
        $content = preg_replace('/\\\\+0+/', '', $content);
    }

    return $content;
}
