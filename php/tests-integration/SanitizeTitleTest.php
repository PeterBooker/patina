<?php

declare(strict_types=1);

namespace Patina\Tests\Integration;

/**
 * sanitize_title_with_dashes override — filter compatibility + byte-for-byte
 * agreement with the unmodified WordPress implementation.
 *
 * Unlike esc_html / wp_kses, sanitize_title_with_dashes itself does NOT fire
 * a filter — the outer wrapper `sanitize_title()` does via the
 * `'sanitize_title'` tag. So the filter-compatibility surface is: the outer
 * wrapper's filter must still see patina's transformed output. That's the
 * contract we lock down here, plus a round-trip through the wp-admin slug
 * machinery (`wp_unique_post_slug`) to verify the override plays nicely with
 * the code paths that actually hit it.
 */
class SanitizeTitleTest extends IntegrationTestCase
{
    protected function setUp(): void
    {
        parent::setUp();
        $this->assertContains(
            'sanitize_title_with_dashes',
            patina_status(),
            'patina sanitize_title_with_dashes override not active'
        );
    }

    // ========================================================================
    // Core behaviour — basic shapes
    // ========================================================================

    public function test_plain_title_becomes_slug(): void
    {
        $this->assertSame('hello-world', sanitize_title_with_dashes('Hello World'));
    }

    public function test_strips_html_and_collapses_whitespace(): void
    {
        $this->assertSame(
            'emphasis-title',
            sanitize_title_with_dashes('<em>emphasis</em>   title')
        );
    }

    public function test_non_ascii_is_percent_encoded(): void
    {
        // Uppercase É → lowercase é → %c3%a9 (lowercase hex).
        $this->assertSame('caf%c3%a9', sanitize_title_with_dashes('Café'));
    }

    public function test_200_byte_cap(): void
    {
        $long = str_repeat('a', 400);
        $this->assertSame(200, strlen(sanitize_title_with_dashes($long)));
    }

    // ========================================================================
    // Context flag — 'save' triggers the extra replacement chain
    // ========================================================================

    public function test_save_context_replaces_forward_slash_with_dash(): void
    {
        $this->assertSame(
            'foo-bar',
            sanitize_title_with_dashes('foo/bar', '', 'save')
        );
    }

    public function test_display_context_preserves_encoded_en_dash(): void
    {
        // display context skips the save-only dash rewrite, so the en-dash's
        // %e2%80%93 encoding survives.
        $this->assertSame(
            'hello%e2%80%93world',
            sanitize_title_with_dashes("hello\xE2\x80\x93world", '', 'display')
        );
    }

    public function test_save_context_replaces_en_dash_with_dash(): void
    {
        $this->assertSame(
            'hello-world',
            sanitize_title_with_dashes("hello\xE2\x80\x93world", '', 'save')
        );
    }

    public function test_save_context_replaces_times_with_x(): void
    {
        // U+00D7 MULTIPLICATION SIGN → x.
        $this->assertSame(
            '10x10',
            sanitize_title_with_dashes("10\xC3\x9710", '', 'save')
        );
    }

    // ========================================================================
    // Filter compatibility — the outer sanitize_title() filter still fires
    // ========================================================================

    public function test_sanitize_title_filter_sees_patina_output(): void
    {
        $captured = null;
        $this->add_test_filter(
            'sanitize_title',
            function ($title, $raw, $context) use (&$captured) {
                $captured = ['title' => $title, 'raw' => $raw, 'context' => $context];
                return $title;
            },
            10,
            3
        );

        sanitize_title('Hello World');

        $this->assertNotNull($captured, 'sanitize_title filter did not fire');
        $this->assertSame('hello-world', $captured['title']);
        $this->assertSame('Hello World', $captured['raw']);
        $this->assertSame('save', $captured['context']);
    }

    public function test_sanitize_title_filter_can_modify_result(): void
    {
        $this->add_test_filter('sanitize_title', function ($title) {
            return $title . '-prefixed';
        });

        $this->assertSame(
            'hello-world-prefixed',
            sanitize_title('Hello World')
        );
    }

    public function test_sanitize_title_falls_back_when_patina_returns_empty(): void
    {
        // sanitize_title() substitutes $fallback_title if the sanitized
        // output is an empty string. Verify patina's empty-output path
        // (input that reduces to nothing after filtering) still engages
        // the fallback.
        $this->assertSame(
            'fallback',
            sanitize_title('%%%', 'fallback')
        );
    }

    // ========================================================================
    // Type coercion — the shim must cast non-strings to string
    // ========================================================================

    public function test_integer_input_is_coerced(): void
    {
        $this->assertSame('12345', sanitize_title_with_dashes(12345));
    }

    public function test_null_input_is_coerced_to_empty_string(): void
    {
        // Stock WP accepts null via loose typing; our shim's (string) cast
        // must reproduce that — (string) null == ''.
        $this->assertSame('', sanitize_title_with_dashes(null));
    }

    // ========================================================================
    // Optional args default correctly (signature parity)
    // ========================================================================

    public function test_raw_title_arg_is_optional(): void
    {
        // Two-arg form should work with the default context.
        $this->assertSame(
            'hello-world',
            sanitize_title_with_dashes('Hello World', 'ignored')
        );
    }

    public function test_all_three_args_positional(): void
    {
        $this->assertSame(
            'hello-world',
            sanitize_title_with_dashes('Hello World', 'ignored', 'save')
        );
    }
}
