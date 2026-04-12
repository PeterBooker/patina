<?php

declare(strict_types=1);

namespace Patina\Tests\Smoke;

use PHPUnit\Framework\TestCase;

class ExtensionLoadTest extends TestCase
{
    public function test_extension_is_loaded(): void
    {
        $this->assertTrue(extension_loaded('patina-ext'));
    }

    public function test_patina_loaded_returns_true(): void
    {
        $this->assertTrue(patina_loaded());
    }

    public function test_patina_version_returns_string(): void
    {
        $version = patina_version();
        $this->assertIsString($version);
        $this->assertMatchesRegularExpression('/^\d+\.\d+\.\d+$/', $version);
    }

    public function test_all_expected_functions_exist(): void
    {
        $expected = [
            'patina_version',
            'patina_loaded',
            'patina_esc_html',
            'patina_esc_attr',
            'patina_wp_kses_post',
            'patina_wp_kses_internal',
            'wp_sanitize_redirect',
        ];

        $registered = get_extension_funcs('patina-ext');
        foreach ($expected as $func) {
            $this->assertContains($func, $registered, "Function {$func} not registered");
        }
    }

    public function test_patina_wp_kses_internal_strips_script(): void
    {
        // Smoke test the override path: no WP functions are loaded here, so the
        // bridge falls through to its hardcoded defaults (post spec, default
        // protocols and URI attributes). Verifies the filter lookup code path
        // doesn't crash when has_filter/apply_filters are absent.
        $result = patina_wp_kses_internal('<b>ok</b><script>alert(1)</script>', 'post', null);
        $this->assertStringContainsString('<b>ok</b>', $result);
        $this->assertStringNotContainsString('<script>', $result);
    }

    public function test_patina_wp_kses_internal_preserves_link(): void
    {
        $result = patina_wp_kses_internal('<a href="http://example.com">link</a>', 'post', null);
        $this->assertStringContainsString('http://example.com', $result);
    }

    public function test_patina_wp_kses_internal_strips_javascript_protocol(): void
    {
        $result = patina_wp_kses_internal('<a href="javascript:alert(1)">xss</a>', 'post', null);
        $this->assertStringNotContainsString('javascript:', $result);
    }
}
