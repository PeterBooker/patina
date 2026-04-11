<?php

/**
 * Reference implementations of WordPress escaping functions.
 * Copied verbatim from WordPress core for benchmark comparison.
 *
 * These are the PHP implementations that our Rust code replaces.
 */

/**
 * Reference esc_html — from wp-includes/formatting.php
 * Simplified: calls _wp_specialchars directly (no filter hook).
 */
function reference_esc_html(string $text): string
{
    return reference__wp_specialchars($text, ENT_QUOTES, 'UTF-8', true);
}

/**
 * Reference esc_attr — identical encoding to esc_html.
 */
function reference_esc_attr(string $text): string
{
    return reference__wp_specialchars($text, ENT_QUOTES, 'UTF-8', true);
}

/**
 * Reference _wp_specialchars — from wp-includes/formatting.php
 *
 * WordPress's version of htmlspecialchars that avoids double-encoding.
 */
function reference__wp_specialchars(
    string $text,
    int $quote_style = ENT_NOQUOTES,
    string $charset = 'UTF-8',
    bool $double_encode = false
): string {
    $text = (string) $text;

    if (0 === strlen($text)) {
        return '';
    }

    // Don't bother if there are no specialchars - saves some processing.
    if (!preg_match('/[&<>"\']/', $text)) {
        return $text;
    }

    // Account for the previous behavior of the function when the $quote_style is not an accepted value.
    if (empty($quote_style)) {
        $quote_style = ENT_NOQUOTES;
    } elseif (ENT_XML1 === $quote_style) {
        $quote_style = ENT_QUOTES | ENT_XML1;
    } elseif (!in_array($quote_style, array(ENT_NOQUOTES, ENT_COMPAT, ENT_QUOTES, 'single', 'double'), true)) {
        $quote_style = ENT_QUOTES;
    }

    $charset = 'UTF-8';

    $_quote_style = $quote_style;

    if ('double' === $quote_style) {
        $quote_style  = ENT_NOQUOTES;
        $_quote_style = ENT_COMPAT;
    } elseif ('single' === $quote_style) {
        $quote_style = ENT_NOQUOTES;
    }

    $text = htmlspecialchars($text, $quote_style | ENT_SUBSTITUTE, $charset, $double_encode);

    // Back-compat.
    if ('single' === $_quote_style) {
        $text = str_replace("'", '&#039;', $text);
    }

    return $text;
}
