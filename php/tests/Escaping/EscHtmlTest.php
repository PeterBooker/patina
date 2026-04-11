<?php

declare(strict_types=1);

namespace Patina\Tests\Escaping;

use Patina\Tests\FixtureTestCase;

class EscHtmlTest extends FixtureTestCase
{
    public static function fixtureProvider(): array
    {
        return static::fixturesAsProvider('esc_html');
    }

    /**
     * @dataProvider fixtureProvider
     */
    public function test_matches_wordpress_output(array $input, mixed $expected): void
    {
        $result = patina_esc_html($input[0]);
        $this->assertSame($expected, $result);
    }

    public function test_script_tag_escaped(): void
    {
        $this->assertSame(
            '&lt;script&gt;alert(1)&lt;/script&gt;',
            patina_esc_html('<script>alert(1)</script>')
        );
    }

    public function test_no_double_encoding(): void
    {
        $this->assertSame('&amp;', patina_esc_html('&amp;'));
        $this->assertSame('&lt;', patina_esc_html('&lt;'));
        $this->assertSame('&#039;', patina_esc_html('&#039;'));
        $this->assertSame('&#x41;', patina_esc_html('&#x41;'));
    }

    public function test_bare_special_chars_encoded(): void
    {
        $this->assertSame('&amp;', patina_esc_html('&'));
        $this->assertSame('&lt;', patina_esc_html('<'));
        $this->assertSame('&gt;', patina_esc_html('>'));
        $this->assertSame('&quot;', patina_esc_html('"'));
        $this->assertSame('&#039;', patina_esc_html("'"));
    }

    public function test_multibyte_preserved(): void
    {
        $this->assertSame(
            '日本語 &lt;b&gt;太字&lt;/b&gt;',
            patina_esc_html('日本語 <b>太字</b>')
        );
    }

    public function test_empty_string(): void
    {
        $this->assertSame('', patina_esc_html(''));
    }

    public function test_plain_text_passthrough(): void
    {
        $input = 'Just plain text with no special characters.';
        $this->assertSame($input, patina_esc_html($input));
    }

    public function test_fuzz_no_crash(): void
    {
        // Generate random valid UTF-8 strings — ext-php-rs requires valid &str
        $chars = 'abcdefghijklmnopqrstuvwxyz<>&"\' ';
        $chars .= '日本語テスト中文العربية';
        $chars .= '&amp;&lt;&gt;&#039;&#x41;&invalid;';
        $charArray = mb_str_split($chars);

        for ($i = 0; $i < 1000; $i++) {
            $len = random_int(0, 200);
            $input = '';
            for ($j = 0; $j < $len; $j++) {
                $input .= $charArray[array_rand($charArray)];
            }
            $result = patina_esc_html($input);
            $this->assertIsString($result, "Non-string return for fuzz input $i");
        }
    }

    public function test_large_input(): void
    {
        $input = str_repeat('<b>hello</b> & "world" ', 10000);
        $result = patina_esc_html($input);
        $this->assertIsString($result);
        $this->assertGreaterThan(strlen($input), strlen($result));
    }
}
