<?php

/**
 * Reference implementation of WordPress sanitize_title_with_dashes().
 *
 * Verbatim port of wp-includes/formatting.php::sanitize_title_with_dashes()
 * from WordPress 6.9.4, with its helpers inlined so the file can be loaded
 * outside a WordPress runtime (the extension-only dev container).
 *
 * Deliberately *not* a filter-free simplification — this is the routine
 * patina_sanitize_title_with_dashes_internal replaces, so the PHP vs Rust
 * comparison here reflects the actual algorithmic work the override saves.
 */

function reference_sanitize_title_with_dashes(string $title, string $raw_title = '', string $context = 'display'): string
{
    $title = strip_tags($title);
    // Preserve escaped octets.
    $title = preg_replace('|%([a-fA-F0-9][a-fA-F0-9])|', '---$1---', $title);
    // Remove percent signs that are not part of an octet.
    $title = str_replace('%', '', $title);
    // Restore octets.
    $title = preg_replace('|---([a-fA-F0-9][a-fA-F0-9])---|', '%$1', $title);

    if (mb_check_encoding($title, 'UTF-8')) {
        if (function_exists('mb_strtolower')) {
            $title = mb_strtolower($title, 'UTF-8');
        }
        $title = reference_utf8_uri_encode($title, 200);
    }

    $title = strtolower($title);

    if ('save' === $context) {
        $title = str_replace(
            array('%c2%a0', '%e2%80%91', '%e2%80%93', '%e2%80%94'),
            '-',
            $title
        );
        $title = str_replace(
            array('&nbsp;', '&#8209;', '&#160;', '&ndash;', '&#8211;', '&mdash;', '&#8212;'),
            '-',
            $title
        );
        $title = str_replace('/', '-', $title);

        $title = str_replace(
            array(
                '%c2%ad',
                '%c2%a1', '%c2%bf',
                '%c2%ab', '%c2%bb', '%e2%80%b9', '%e2%80%ba',
                '%e2%80%98', '%e2%80%99', '%e2%80%9c', '%e2%80%9d',
                '%e2%80%9a', '%e2%80%9b', '%e2%80%9e', '%e2%80%9f',
                '%e2%80%a2',
                '%c2%a9', '%c2%ae', '%c2%b0', '%e2%80%a6', '%e2%84%a2',
                '%c2%b4', '%cb%8a', '%cc%81', '%cd%81',
                '%cc%80', '%cc%84', '%cc%8c',
                '%e2%80%8b', '%e2%80%8c', '%e2%80%8d', '%e2%80%8e', '%e2%80%8f',
                '%e2%80%aa', '%e2%80%ab', '%e2%80%ac', '%e2%80%ad', '%e2%80%ae',
                '%ef%bb%bf', '%ef%bf%bc',
            ),
            '',
            $title
        );

        $title = str_replace(
            array(
                '%e2%80%80', '%e2%80%81', '%e2%80%82', '%e2%80%83',
                '%e2%80%84', '%e2%80%85', '%e2%80%86', '%e2%80%87',
                '%e2%80%88', '%e2%80%89', '%e2%80%8a',
                '%e2%80%a8', '%e2%80%a9', '%e2%80%af',
            ),
            '-',
            $title
        );

        $title = str_replace('%c3%97', 'x', $title);
    }

    $title = preg_replace('/&.+?;/', '', $title);
    $title = str_replace('.', '-', $title);

    $title = preg_replace('/[^%a-z0-9 _-]/', '', $title);
    $title = preg_replace('/\s+/', '-', $title);
    $title = preg_replace('|-+|', '-', $title);
    $title = trim($title, '-');

    return $title;
}

/**
 * Inlined verbatim from wp-includes/formatting.php so the reference is
 * self-contained and callable outside a WordPress runtime.
 */
function reference_utf8_uri_encode(string $utf8_string, int $length = 0, bool $encode_ascii_characters = false): string
{
    $unicode        = '';
    $values         = array();
    $num_octets     = 1;
    $unicode_length = 0;

    $string_length = strlen($utf8_string);

    for ($i = 0; $i < $string_length; $i++) {
        $value = ord($utf8_string[$i]);

        if ($value < 128) {
            $char                = chr($value);
            $encoded_char        = $encode_ascii_characters ? rawurlencode($char) : $char;
            $encoded_char_length = strlen($encoded_char);
            if ($length && ($unicode_length + $encoded_char_length) > $length) {
                break;
            }
            $unicode        .= $encoded_char;
            $unicode_length += $encoded_char_length;
        } else {
            if (0 === count($values)) {
                if ($value < 224) {
                    $num_octets = 2;
                } elseif ($value < 240) {
                    $num_octets = 3;
                } else {
                    $num_octets = 4;
                }
            }

            $values[] = $value;

            if ($length && ($unicode_length + ($num_octets * 3)) > $length) {
                break;
            }
            if (count($values) === $num_octets) {
                for ($j = 0; $j < $num_octets; $j++) {
                    $unicode .= '%' . dechex($values[$j]);
                }
                $unicode_length += $num_octets * 3;

                $values     = array();
                $num_octets = 1;
            }
        }
    }

    return $unicode;
}
