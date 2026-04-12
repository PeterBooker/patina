<?php

declare(strict_types=1);

namespace Patina\Tests\Integration;

use PHPUnit\Framework\TestCase;

/**
 * Base class for integration tests that register WordPress filters.
 *
 * Tracks every filter added via `add_test_filter()` and removes them
 * in tearDown, so test A's filters never leak into test B — and more
 * importantly, WP's own default filters (wp_pre_kses_less_than on
 * pre_kses, etc.) are never touched.
 */
abstract class IntegrationTestCase extends TestCase
{
    /** @var list<array{string, callable, int}> */
    private array $added_filters = [];

    /**
     * Add a filter for the duration of this test. Automatically removed
     * in tearDown. Use this instead of `add_filter()` directly.
     *
     * Returns the callback so the test can capture it for assertions.
     */
    protected function add_test_filter(
        string $tag,
        callable $callback,
        int $priority = 10,
        int $accepted_args = 1
    ): callable {
        add_filter($tag, $callback, $priority, $accepted_args);
        $this->added_filters[] = [$tag, $callback, $priority];
        return $callback;
    }

    protected function tearDown(): void
    {
        foreach ($this->added_filters as [$tag, $callback, $priority]) {
            remove_filter($tag, $callback, $priority);
        }
        $this->added_filters = [];
        parent::tearDown();
    }

    protected function assertPatinaActive(): void
    {
        $this->assertContains('wp_kses', patina_status(), 'patina wp_kses override not active');
    }
}
