<?php

declare(strict_types=1);

namespace Patina\Tests\Integration;

/**
 * Regression coverage for PHP's loose typing on escaping/sanitization
 * functions. Stock WordPress `esc_html()`, `esc_attr()`, and `wp_kses()`
 * accept any scalar (int, float, bool, null) — because PHP is loose-typed
 * without strict_types and `htmlspecialchars()` internally coerces.
 *
 * The Rust overrides must preserve this behavior or wp-admin explodes on
 * pages that call `esc_attr($per_page_integer)` from screen options, etc.
 * This class was added after hitting exactly that in production:
 *
 *   PHP Fatal error: Uncaught Exception: Invalid value given for argument
 *   `text`. in wp-admin/includes/class-wp-screen.php line 1285
 *   patina_esc_attr_filtered()
 *   WP_Screen render_per_page_options()
 *
 * The fix was to change the Rust signature from `text: &str` to
 * `text: &Zval` and call `coerce_to_string()` so scalar types get
 * converted the same way stock PHP would convert them.
 */
class LooseTypingTest extends IntegrationTestCase
{
    // ========================================================================
    // esc_attr — the function that actually crashed
    // ========================================================================

    public function test_esc_attr_accepts_integer(): void
    {
        // This mirrors the exact call that crashed wp-admin/plugins.php:
        //   value="[PHP] echo esc_attr( $per_page ); [/PHP]"
        // where $per_page is an integer (e.g. 20 items per page).
        $this->assertSame('20', esc_attr(20));
    }

    public function test_esc_attr_accepts_float(): void
    {
        $this->assertSame('3.14', esc_attr(3.14));
    }

    public function test_esc_attr_accepts_bool_true(): void
    {
        $this->assertSame('1', esc_attr(true));
    }

    public function test_esc_attr_accepts_bool_false(): void
    {
        // PHP's (string) false is "" — and esc_attr preserves that.
        $this->assertSame('', esc_attr(false));
    }

    public function test_esc_attr_accepts_null(): void
    {
        $this->assertSame('', esc_attr(null));
    }

    public function test_esc_attr_accepts_zero(): void
    {
        $this->assertSame('0', esc_attr(0));
    }

    // ========================================================================
    // esc_html — same story
    // ========================================================================

    public function test_esc_html_accepts_integer(): void
    {
        $this->assertSame('42', esc_html(42));
    }

    public function test_esc_html_accepts_float(): void
    {
        $this->assertSame('3.14', esc_html(3.14));
    }

    public function test_esc_html_accepts_null(): void
    {
        $this->assertSame('', esc_html(null));
    }

    public function test_esc_html_accepts_bool(): void
    {
        $this->assertSame('1', esc_html(true));
        $this->assertSame('', esc_html(false));
    }

    // ========================================================================
    // wp_kses — via the shim → patina_wp_kses_internal
    // ========================================================================

    public function test_wp_kses_accepts_integer_content(): void
    {
        $this->assertSame('42', wp_kses(42, 'post'));
    }

    public function test_wp_kses_accepts_null_content(): void
    {
        $this->assertSame('', wp_kses(null, 'post'));
    }

    public function test_wp_kses_post_accepts_integer(): void
    {
        // wp_kses_post($data) → wp_kses($data, 'post')
        $this->assertSame('99', wp_kses_post(99));
    }

    // ========================================================================
    // Existing string behavior still works (no regression)
    // ========================================================================

    public function test_esc_attr_still_works_for_strings(): void
    {
        $this->assertSame('&quot;hello&quot;', esc_attr('"hello"'));
    }

    public function test_esc_html_still_works_for_strings(): void
    {
        $this->assertSame('&lt;b&gt;hi&lt;/b&gt;', esc_html('<b>hi</b>'));
    }

    public function test_wp_kses_still_strips_script(): void
    {
        // wp_kses strips the <script> tags themselves but preserves their
        // text content — matches stock WordPress behavior.
        $result = wp_kses('<b>ok</b><script>alert(1)</script>', 'post');
        $this->assertStringContainsString('<b>ok</b>', $result);
        $this->assertStringNotContainsString('<script', $result);
        $this->assertStringNotContainsString('</script', $result);
    }

    // ========================================================================
    // wp_sanitize_redirect — pluggable, same coercion issue
    // ========================================================================

    public function test_wp_sanitize_redirect_accepts_null(): void
    {
        // wp_get_referer() can return false/null, which plugins might pass
        // straight to wp_sanitize_redirect without a guard.
        $this->assertSame('', wp_sanitize_redirect(null));
    }

    public function test_wp_sanitize_redirect_accepts_false(): void
    {
        $this->assertSame('', wp_sanitize_redirect(false));
    }

    public function test_wp_sanitize_redirect_still_works_for_strings(): void
    {
        $this->assertSame(
            'https://example.com/page',
            wp_sanitize_redirect('https://example.com/page')
        );
    }

    // ========================================================================
    // wp_validate_redirect — optional fallback_url + coercion
    // ========================================================================

    public function test_wp_validate_redirect_with_only_location(): void
    {
        // PHP signature: function wp_validate_redirect($location, $fallback_url = '')
        // Calling with 1 arg must not throw ArgumentCountError.
        $result = wp_validate_redirect('https://example.com/');
        // Any string result is fine — the point of this test is "does not throw".
        $this->assertIsString($result);
    }

    public function test_wp_validate_redirect_accepts_null_location(): void
    {
        // Should not throw; should fall back to the fallback_url (defaulting to '').
        $result = wp_validate_redirect(null);
        $this->assertIsString($result);
    }

    public function test_wp_validate_redirect_accepts_null_fallback(): void
    {
        $result = wp_validate_redirect('https://evil.example.com/', null);
        $this->assertIsString($result);
    }
}
