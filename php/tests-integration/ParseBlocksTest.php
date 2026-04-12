<?php

declare(strict_types=1);

namespace Patina\Tests\Integration;

/**
 * Verify the parse_blocks override matches stock WordPress output and
 * honors the block_parser_class filter so plugins can still swap in
 * their own parser.
 */
class ParseBlocksTest extends IntegrationTestCase
{
    protected function setUp(): void
    {
        parent::setUp();
        $this->assertContains(
            'parse_blocks',
            patina_status(),
            'parse_blocks override not active'
        );
    }

    // ========================================================================
    // Output fidelity — patina matches stock for every shape
    // ========================================================================

    public function test_empty_input(): void
    {
        $this->assertSame([], parse_blocks(''));
    }

    public function test_plain_text_becomes_freeform_block(): void
    {
        $result = parse_blocks('hello world');
        $this->assertCount(1, $result);
        $this->assertNull($result[0]['blockName']);
        $this->assertSame('hello world', $result[0]['innerHTML']);
        $this->assertSame(['hello world'], $result[0]['innerContent']);
    }

    public function test_single_void_block(): void
    {
        $result = parse_blocks('<!-- wp:separator /-->');
        $this->assertCount(1, $result);
        $this->assertSame('core/separator', $result[0]['blockName']);
        $this->assertSame([], $result[0]['innerBlocks']);
        $this->assertSame('', $result[0]['innerHTML']);
    }

    public function test_single_wrapped_block(): void
    {
        $result = parse_blocks('<!-- wp:paragraph --><p>Hello</p><!-- /wp:paragraph -->');
        $this->assertCount(1, $result);
        $this->assertSame('core/paragraph', $result[0]['blockName']);
        $this->assertSame('<p>Hello</p>', $result[0]['innerHTML']);
    }

    public function test_block_with_json_attrs(): void
    {
        $result = parse_blocks('<!-- wp:heading {"level":3} --><h3>Hi</h3><!-- /wp:heading -->');
        $this->assertCount(1, $result);
        $this->assertSame(['level' => 3], $result[0]['attrs']);
    }

    public function test_no_attrs_is_empty_array(): void
    {
        $result = parse_blocks('<!-- wp:paragraph --><p>x</p><!-- /wp:paragraph -->');
        // PHP represents no-attrs as an empty array. Our override must match.
        $this->assertSame([], $result[0]['attrs']);
    }

    public function test_nested_blocks(): void
    {
        $result = parse_blocks(
            '<!-- wp:group --><div><!-- wp:paragraph --><p>Inner</p><!-- /wp:paragraph --></div><!-- /wp:group -->'
        );
        $this->assertCount(1, $result);
        $this->assertSame('core/group', $result[0]['blockName']);
        $this->assertCount(1, $result[0]['innerBlocks']);
        $this->assertSame('core/paragraph', $result[0]['innerBlocks'][0]['blockName']);
        $this->assertSame('<p>Inner</p>', $result[0]['innerBlocks'][0]['innerHTML']);
    }

    public function test_nested_inner_content_layout(): void
    {
        $result = parse_blocks(
            '<!-- wp:group --><div><!-- wp:paragraph --><p>x</p><!-- /wp:paragraph --></div><!-- /wp:group -->'
        );
        $group = $result[0];
        // innerContent must interleave: [before-html, null, after-html]
        $this->assertCount(3, $group['innerContent']);
        $this->assertSame('<div>', $group['innerContent'][0]);
        $this->assertNull($group['innerContent'][1]);
        $this->assertSame('</div>', $group['innerContent'][2]);
    }

    public function test_namespaced_block(): void
    {
        $result = parse_blocks('<!-- wp:my-plugin/custom /-->');
        $this->assertSame('my-plugin/custom', $result[0]['blockName']);
    }

    public function test_leading_freeform_before_block(): void
    {
        $result = parse_blocks('prefix<!-- wp:separator /-->');
        $this->assertCount(2, $result);
        $this->assertNull($result[0]['blockName']);
        $this->assertSame('prefix', $result[0]['innerHTML']);
        $this->assertSame('core/separator', $result[1]['blockName']);
    }

    // ========================================================================
    // Exact match against stock PHP parser
    //
    // For arbitrary inputs we should produce byte-identical output to
    // WP_Block_Parser. We verify this by using ReflectionClass to bypass
    // our override and parse with the original class directly, then
    // comparing.
    // ========================================================================

    #[\PHPUnit\Framework\Attributes\DataProvider('exact_match_inputs')]
    public function test_output_matches_stock_parser(string $input): void
    {
        $patina_result = parse_blocks($input);

        // Call stock WP parser directly, bypassing our override.
        $parser = new \WP_Block_Parser();
        $stock_result = $parser->parse($input);

        $this->assertSame(
            $stock_result,
            $patina_result,
            "Patina parse_blocks differs from stock WP_Block_Parser for input: " . substr($input, 0, 100)
        );
    }

    public static function exact_match_inputs(): array
    {
        return [
            'empty' => [''],
            'plain_text' => ['just some text'],
            'single_void' => ['<!-- wp:separator /-->'],
            'wrapped_paragraph' => ['<!-- wp:paragraph --><p>Hi</p><!-- /wp:paragraph -->'],
            'nested_group' => [
                '<!-- wp:group --><div><!-- wp:paragraph --><p>a</p><!-- /wp:paragraph --></div><!-- /wp:group -->'
            ],
            'with_attrs' => ['<!-- wp:heading {"level":2} --><h2>x</h2><!-- /wp:heading -->'],
            'complex_attrs' => [
                '<!-- wp:group {"style":{"spacing":{"padding":{"top":"10px","bottom":"20px"}}}} --><div>x</div><!-- /wp:group -->'
            ],
            'multi_block' => [
                '<!-- wp:paragraph --><p>1</p><!-- /wp:paragraph --><!-- wp:paragraph --><p>2</p><!-- /wp:paragraph -->'
            ],
            'freeform_and_blocks' => [
                'before<!-- wp:separator /-->middle<!-- wp:separator /-->after'
            ],
            'dangling_opener' => ['<!-- wp:paragraph --><p>dangling'],
            'orphan_closer' => ['<p>x</p><!-- /wp:paragraph -->'],
            'namespaced' => ['<!-- wp:myns/mybl /-->'],
            'fake_comment' => ['<!-- not a block --><!-- wp:paragraph --><p>real</p><!-- /wp:paragraph -->'],
        ];
    }

    // ========================================================================
    // block_parser_class filter — plugin compatibility
    // ========================================================================

    public function test_block_parser_class_filter_fallback(): void
    {
        // Register a custom parser class via the filter. The shim should
        // detect this and fall back to the PHP implementation, bypassing
        // our Rust parser entirely. The custom parser below wraps every
        // block name in a marker so we can verify it actually ran.
        $this->add_test_filter('block_parser_class', function () {
            return 'Patina\Tests\Integration\MarkerParser';
        });

        $result = parse_blocks('<!-- wp:paragraph --><p>x</p><!-- /wp:paragraph -->');

        $this->assertCount(1, $result);
        $this->assertSame('MARKED:core/paragraph', $result[0]['blockName']);
    }

    public function test_block_parser_class_filter_is_fired(): void
    {
        // Filter is fired even when it returns the default value.
        $fired = false;
        $this->add_test_filter('block_parser_class', function ($class) use (&$fired) {
            $fired = true;
            return $class;
        });

        parse_blocks('<!-- wp:separator /-->');

        $this->assertTrue($fired, 'block_parser_class filter was not fired');
    }
}

/**
 * Custom WP_Block_Parser subclass used to verify the shim's filter-fallback
 * path. Wraps each block name in "MARKED:" so the test can detect it.
 */
class MarkerParser extends \WP_Block_Parser
{
    public function parse($document)
    {
        $blocks = parent::parse($document);
        foreach ($blocks as &$block) {
            if ($block['blockName'] !== null) {
                $block['blockName'] = 'MARKED:' . $block['blockName'];
            }
        }
        return $blocks;
    }
}
