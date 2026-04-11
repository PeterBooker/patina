<?php

declare(strict_types=1);

namespace Patina\Tests\Escaping;

use Patina\Tests\FixtureTestCase;

class EscAttrTest extends FixtureTestCase
{
    public static function fixtureProvider(): array
    {
        return static::fixturesAsProvider('esc_attr');
    }

    /**
     * @dataProvider fixtureProvider
     */
    public function test_matches_wordpress_output(array $input, mixed $expected): void
    {
        $result = patina_esc_attr($input[0]);
        $this->assertSame($expected, $result);
    }

    public function test_attribute_injection(): void
    {
        $this->assertSame(
            '&quot; onclick=&quot;alert(1)',
            patina_esc_attr('" onclick="alert(1)')
        );
    }

    public function test_no_double_encoding(): void
    {
        $this->assertSame('&amp;', patina_esc_attr('&amp;'));
    }

    public function test_empty_string(): void
    {
        $this->assertSame('', patina_esc_attr(''));
    }
}
