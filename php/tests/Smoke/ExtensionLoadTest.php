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
            'wp_sanitize_redirect',
        ];

        $registered = get_extension_funcs('patina-ext');
        foreach ($expected as $func) {
            $this->assertContains($func, $registered, "Function {$func} not registered");
        }
    }
}
