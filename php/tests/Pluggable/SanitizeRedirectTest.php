<?php

declare(strict_types=1);

namespace Patina\Tests\Pluggable;

use Patina\Tests\FixtureTestCase;

class SanitizeRedirectTest extends FixtureTestCase
{
    public static function fixtureProvider(): array
    {
        return static::fixturesAsProvider('wp_sanitize_redirect');
    }

    /**
     * @dataProvider fixtureProvider
     */
    public function test_matches_wordpress_output(array $input, mixed $expected): void
    {
        $result = wp_sanitize_redirect($input[0]);
        $this->assertSame($expected, $result);
    }

    public function test_simple_url_passthrough(): void
    {
        $this->assertSame(
            'http://example.com/page',
            wp_sanitize_redirect('http://example.com/page')
        );
    }

    public function test_spaces_to_percent20(): void
    {
        $result = wp_sanitize_redirect('http://example.com/my page');
        $this->assertSame('http://example.com/my%20page', $result);
    }

    public function test_multibyte_percent_encoded(): void
    {
        $result = wp_sanitize_redirect('http://example.com/日本');
        $this->assertStringNotContainsString('日', $result);
        $this->assertStringContainsString('%', $result);
    }

    public function test_strips_html_tags(): void
    {
        $result = wp_sanitize_redirect('http://example.com/<script>alert(1)</script>');
        $this->assertStringNotContainsString('<', $result);
        $this->assertStringNotContainsString('>', $result);
    }

    public function test_empty_string(): void
    {
        $this->assertSame('', wp_sanitize_redirect(''));
    }

    public function test_preserves_query_and_fragment(): void
    {
        $this->assertSame(
            'http://example.com/path?key=value&other=1#section',
            wp_sanitize_redirect('http://example.com/path?key=value&other=1#section')
        );
    }

    public function test_fuzz_no_crash(): void
    {
        // Generate random valid UTF-8 strings — ext-php-rs requires valid &str
        $chars = 'abcdefghijklmnopqrstuvwxyz0123456789';
        $chars .= 'http://example.com/ ?&=#%<>';
        $chars .= '日本語テスト';
        $charArray = mb_str_split($chars);

        for ($i = 0; $i < 1000; $i++) {
            $len = random_int(0, 200);
            $input = '';
            for ($j = 0; $j < $len; $j++) {
                $input .= $charArray[array_rand($charArray)];
            }
            $result = wp_sanitize_redirect($input);
            $this->assertIsString($result, "Non-string return for fuzz input $i");
        }
    }

    public function test_large_input(): void
    {
        $input = 'http://example.com/' . str_repeat('a', 100000);
        $result = wp_sanitize_redirect($input);
        $this->assertIsString($result);
    }
}
