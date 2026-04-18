<?php

declare(strict_types=1);

namespace Patina\Tests\Integration;

/**
 * Verify the patina esc_html / esc_attr overrides preserve WordPress
 * filter compatibility. These overrides went through one refactor where
 * `apply_filters('esc_html', ...)` fired from Rust via `call_user_func`;
 * the current form moves the filter call back into the PHP shim because
 * Rust→PHP round-trips cost ~2-5 µs per invocation vs near-zero for
 * PHP→PHP dispatch when no filter is registered.
 *
 * The functional contract is the same either way, and these tests hold
 * us to it regardless of where in the stack apply_filters actually
 * fires.
 *
 * Every test registers a real WordPress filter and asserts the filter's
 * observable effect on the overridden function's output. We don't probe
 * internal state; if the filter receives the right args and can shape
 * the result, we're byte-for-byte compatible with stock WP.
 */
class EscFilterTest extends IntegrationTestCase
{
    protected function setUp(): void
    {
        parent::setUp();
        $this->assertPatinaActive();
    }

    // ========================================================================
    // esc_html — filter receives the safe text + original value, can rewrite
    // ========================================================================

    public function test_esc_html_filter_is_invoked(): void
    {
        $called = false;
        $this->add_test_filter('esc_html', function ($safe_text) use (&$called) {
            $called = true;
            return $safe_text;
        });

        esc_html('<b>hi</b>');

        $this->assertTrue($called, 'esc_html filter did not fire');
    }

    public function test_esc_html_filter_receives_safe_and_original(): void
    {
        $captured_safe = null;
        $captured_raw = null;
        $this->add_test_filter(
            'esc_html',
            function ($safe_text, $raw_text) use (&$captured_safe, &$captured_raw) {
                $captured_safe = $safe_text;
                $captured_raw = $raw_text;
                return $safe_text;
            },
            10,
            2
        );

        esc_html('<script>');

        $this->assertSame('&lt;script&gt;', $captured_safe);
        $this->assertSame('<script>', $captured_raw);
    }

    public function test_esc_html_filter_can_modify_result(): void
    {
        $this->add_test_filter('esc_html', function ($safe_text) {
            return $safe_text . '-suffixed';
        });

        $this->assertSame(
            '&lt;b&gt;hi&lt;/b&gt;-suffixed',
            esc_html('<b>hi</b>')
        );
    }

    public function test_esc_html_filter_third_arg_is_the_original_not_cast(): void
    {
        // Stock WP's esc_html signature is `esc_html($text)` with no type
        // coercion — when a caller passes a non-string, the filter's third
        // argument gets the raw pre-cast value. Confirm we match that: the
        // shim casts for the Rust internal but must pass the original to
        // the filter.
        $captured_raw = null;
        $this->add_test_filter(
            'esc_html',
            function ($safe_text, $raw_text) use (&$captured_raw) {
                $captured_raw = $raw_text;
                return $safe_text;
            },
            10,
            2
        );

        esc_html(42);

        $this->assertSame(42, $captured_raw, 'filter third arg must be the original value');
    }

    // ========================================================================
    // esc_attr — same contract as esc_html
    // ========================================================================

    public function test_esc_attr_filter_is_invoked(): void
    {
        $called = false;
        $this->add_test_filter('esc_attr', function ($safe_text) use (&$called) {
            $called = true;
            return $safe_text;
        });

        esc_attr('"hello"');

        $this->assertTrue($called, 'esc_attr filter did not fire');
    }

    public function test_esc_attr_filter_receives_safe_and_original(): void
    {
        $captured_safe = null;
        $captured_raw = null;
        $this->add_test_filter(
            'esc_attr',
            function ($safe_text, $raw_text) use (&$captured_safe, &$captured_raw) {
                $captured_safe = $safe_text;
                $captured_raw = $raw_text;
                return $safe_text;
            },
            10,
            2
        );

        esc_attr('"quoted"');

        $this->assertSame('&quot;quoted&quot;', $captured_safe);
        $this->assertSame('"quoted"', $captured_raw);
    }

    public function test_esc_attr_filter_can_modify_result(): void
    {
        $this->add_test_filter('esc_attr', function ($safe_text) {
            return strtoupper($safe_text);
        });

        $this->assertSame(
            '&QUOT;HI&QUOT;',
            esc_attr('"hi"')
        );
    }

    // ========================================================================
    // Fast path: no filter registered — result matches sanitization only
    // ========================================================================

    public function test_esc_html_with_no_filter_returns_raw_sanitization(): void
    {
        // Sanity: the tests above register filters; this one relies on
        // the base class tearDown removing them so we see the unfiltered
        // path.
        $this->assertSame('&lt;b&gt;hi&lt;/b&gt;', esc_html('<b>hi</b>'));
    }

    public function test_esc_attr_with_no_filter_returns_raw_sanitization(): void
    {
        $this->assertSame('&quot;hello&quot;', esc_attr('"hello"'));
    }
}
