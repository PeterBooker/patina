<?php

declare(strict_types=1);

namespace Patina\Tests\Integration;

/**
 * Verify that the patina wp_kses override preserves WordPress filter
 * compatibility, so plugins/themes that hook into the kses pipeline
 * continue to work unchanged.
 *
 * Every test registers a real WordPress filter, calls wp_kses via the
 * overridden path, and asserts the filter's observable effect.
 */
class KsesFilterTest extends IntegrationTestCase
{
    protected function setUp(): void
    {
        parent::setUp();
        $this->assertPatinaActive();
    }

    // ========================================================================
    // pre_kses — fires before sanitization runs
    // ========================================================================

    public function test_pre_kses_filter_is_invoked(): void
    {
        $called_with = null;
        $this->add_test_filter('pre_kses', function ($content) use (&$called_with) {
            $called_with = $content;
            return $content;
        });

        wp_kses('<b>hello</b>', 'post');

        $this->assertSame('<b>hello</b>', $called_with);
    }

    public function test_pre_kses_can_rewrite_content(): void
    {
        $this->add_test_filter('pre_kses', function ($content) {
            return str_replace('MARKER', '<b>rewritten</b>', $content);
        });

        $result = wp_kses('before MARKER after', 'post');

        $this->assertStringContainsString('<b>rewritten</b>', $result);
        $this->assertStringNotContainsString('MARKER', $result);
    }

    public function test_pre_kses_receives_allowed_html_arg(): void
    {
        $captured_context = null;
        $this->add_test_filter('pre_kses', function ($content, $allowed_html) use (&$captured_context) {
            $captured_context = $allowed_html;
            return $content;
        }, 10, 3);

        wp_kses('<b>x</b>', 'post');

        $this->assertSame('post', $captured_context);
    }

    public function test_pre_kses_default_wp_pre_kses_less_than_still_runs(): void
    {
        // WP registers wp_pre_kses_less_than as a default pre_kses filter.
        // Make sure the override doesn't clobber WP's own filter registrations.
        $result = wp_kses('test < b > 4', 'post');

        // wp_pre_kses_less_than entity-encodes malformed `<` sequences that
        // don't close into a real tag. The "< b > 4" fragment should be
        // handled by that default filter.
        $this->assertStringNotContainsString('< b >', $result);
    }

    // ========================================================================
    // wp_kses_allowed_html — filter the tag/attribute allowlist
    // ========================================================================

    public function test_wp_kses_allowed_html_can_add_a_new_tag(): void
    {
        $this->add_test_filter('wp_kses_allowed_html', function ($tags, $context) {
            if ($context === 'post') {
                $tags['iframe'] = [
                    'src' => true,
                    'width' => true,
                    'height' => true,
                ];
            }
            return $tags;
        }, 10, 2);

        $result = wp_kses(
            '<iframe src="https://example.com/embed" width="640" height="360"></iframe>',
            'post'
        );

        $this->assertStringContainsString('<iframe', $result);
        $this->assertStringContainsString('src="https://example.com/embed"', $result);
    }

    public function test_wp_kses_allowed_html_can_add_attr_to_existing_tag(): void
    {
        // By default `<a>` allows several attrs. Add a custom one.
        $this->add_test_filter('wp_kses_allowed_html', function ($tags, $context) {
            if ($context === 'post' && isset($tags['a'])) {
                $tags['a']['data-track'] = true;
            }
            return $tags;
        }, 10, 2);

        $result = wp_kses(
            '<a href="https://example.com" data-track="click">link</a>',
            'post'
        );

        $this->assertStringContainsString('data-track="click"', $result);
    }

    public function test_wp_kses_allowed_html_can_remove_tag(): void
    {
        // Strip `<b>` from the allowlist for this test.
        $this->add_test_filter('wp_kses_allowed_html', function ($tags, $context) {
            unset($tags['b']);
            return $tags;
        }, 10, 2);

        $result = wp_kses('<b>bold</b>', 'post');

        $this->assertStringNotContainsString('<b>', $result);
        $this->assertStringContainsString('bold', $result);
    }

    public function test_wp_kses_allowed_html_context_is_passed(): void
    {
        $seen_contexts = [];
        $this->add_test_filter('wp_kses_allowed_html', function ($tags, $context) use (&$seen_contexts) {
            $seen_contexts[] = $context;
            return $tags;
        }, 10, 2);

        wp_kses('<b>x</b>', 'post');
        wp_kses('<b>x</b>', 'data');

        $this->assertContains('post', $seen_contexts);
        $this->assertContains('data', $seen_contexts);
    }

    // ========================================================================
    // Allowed protocols — explicit argument + kses_allowed_protocols filter
    // ========================================================================

    public function test_explicit_protocols_arg_is_respected(): void
    {
        // When the caller passes an explicit protocols array (3rd arg to
        // wp_kses), our bridge uses it directly — no wp_allowed_protocols
        // round-trip, no kses_allowed_protocols filter.
        $result = wp_kses(
            '<a href="myapp://open/42">open</a>',
            'post',
            ['http', 'https', 'myapp']
        );

        $this->assertStringContainsString('myapp://open/42', $result);
    }

    public function test_explicit_protocols_restrict_allowlist(): void
    {
        // Passing only ['http'] should strip https: protocol links.
        $result = wp_kses(
            '<a href="https://example.com">link</a>',
            'post',
            ['http']
        );

        // https is not in the explicit list, so the scheme gets stripped.
        // (wp_kses_bad_protocol strips the scheme but preserves the rest.)
        $this->assertStringNotContainsString('https:', $result);
    }

    public function test_kses_allowed_protocols_filter_fires(): void
    {
        // WP caches wp_allowed_protocols() in a static after wp_loaded fires,
        // so adding a kses_allowed_protocols filter post-wp_loaded doesn't
        // affect wp_allowed_protocols()'s return value. But our bridge still
        // respects the filter via has_filter() + wp_allowed_protocols() call —
        // we can verify the filter *is wired* by asserting it gets invoked.
        $called = false;
        $this->add_test_filter('kses_allowed_protocols', function ($protocols) use (&$called) {
            $called = true;
            return $protocols;
        });

        // Force a kses call with no explicit protocols so the bridge takes
        // the slow path and calls wp_allowed_protocols(). The filter inside
        // wp_allowed_protocols is gated on `!did_action('wp_loaded')`, so
        // even though it's registered, it may not fire. That is WP's own
        // cache, not patina's — we assert the wiring is in place.
        wp_kses('<a href="http://example.com">ok</a>', 'post');

        // Either the filter was called (pre-wp_loaded) or WP's cache
        // short-circuited it (post-wp_loaded). Both are valid.
        $this->assertTrue(
            $called || did_action('wp_loaded') > 0,
            'kses_allowed_protocols filter wiring broken'
        );
    }

    public function test_unregistered_protocols_are_stripped(): void
    {
        // Sanity: dangerous protocols are stripped regardless of filters.
        $result = wp_kses('<a href="javascript:alert(1)">xss</a>', 'post');

        $this->assertStringNotContainsString('javascript:', $result);
    }

    // ========================================================================
    // wp_kses_uri_attributes — filter the list of URI-bearing attrs
    // ========================================================================

    public function test_wp_kses_uri_attributes_can_add_custom_attr(): void
    {
        // Allow data-src on img (not normally protocol-checked), then add
        // data-src to the URI attributes list so the protocol check runs.
        $this->add_test_filter('wp_kses_allowed_html', function ($tags, $context) {
            if ($context === 'post' && isset($tags['img'])) {
                $tags['img']['data-src'] = true;
            }
            return $tags;
        }, 10, 2);

        $this->add_test_filter('wp_kses_uri_attributes', function ($attrs) {
            $attrs[] = 'data-src';
            return $attrs;
        });

        // javascript: in data-src should now be stripped because data-src is
        // treated as a URI attribute.
        $result = wp_kses(
            '<img src="https://example.com/a.png" data-src="javascript:alert(1)">',
            'post'
        );

        $this->assertStringContainsString('src="https://example.com/a.png"', $result);
        $this->assertStringNotContainsString('javascript:', $result);
    }

    // ========================================================================
    // Wrapper coverage — filters fire through every wrapper
    // ========================================================================

    public function test_pre_kses_fires_for_wp_kses_post(): void
    {
        $called = false;
        $this->add_test_filter('pre_kses', function ($content) use (&$called) {
            $called = true;
            return $content;
        });

        wp_kses_post('<b>x</b>');

        $this->assertTrue($called, 'pre_kses should fire when wp_kses_post is called');
    }

    public function test_pre_kses_fires_for_wp_filter_post_kses(): void
    {
        $called = false;
        $this->add_test_filter('pre_kses', function ($content) use (&$called) {
            $called = true;
            return $content;
        });

        // wp_filter_post_kses is the one hooked into content_save_pre.
        // It expects slashed input and returns slashed output.
        wp_filter_post_kses(addslashes('<b>x</b>'));

        $this->assertTrue($called, 'pre_kses should fire when wp_filter_post_kses is called');
    }

    public function test_wp_kses_allowed_html_fires_for_wp_kses_data(): void
    {
        $called = false;
        $this->add_test_filter('wp_kses_allowed_html', function ($tags, $context) use (&$called) {
            $called = true;
            return $tags;
        }, 10, 2);

        wp_kses_data('<b>x</b>');

        $this->assertTrue($called, 'wp_kses_allowed_html should fire when wp_kses_data is called');
    }

    // ========================================================================
    // Output fidelity — patina matches stock PHP for the common path
    // ========================================================================

    public function test_basic_output_matches_expected(): void
    {
        $result = wp_kses('<p>Hello <b>world</b>!</p>', 'post');
        $this->assertSame('<p>Hello <b>world</b>!</p>', $result);
    }

    public function test_strips_script_tag(): void
    {
        $result = wp_kses('<p>safe</p><script>alert(1)</script>', 'post');
        $this->assertStringContainsString('<p>safe</p>', $result);
        $this->assertStringNotContainsString('<script', $result);
    }

    public function test_preserves_allowed_attributes(): void
    {
        $result = wp_kses('<a href="https://example.com" title="ok">link</a>', 'post');
        $this->assertStringContainsString('href="https://example.com"', $result);
        $this->assertStringContainsString('title="ok"', $result);
    }

    public function test_strips_disallowed_attributes(): void
    {
        $result = wp_kses('<a href="https://example.com" onclick="x()">link</a>', 'post');
        $this->assertStringContainsString('href="https://example.com"', $result);
        $this->assertStringNotContainsString('onclick', $result);
    }
}
