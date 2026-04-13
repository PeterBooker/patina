<?php

declare(strict_types=1);

namespace Patina\Tests\Integration;

/**
 * Verify the per-override skip list wired up in Phase 3 of the benchmark
 * plan actually disables individual overrides.
 *
 * These tests tear down patina's existing activation, re-activate with a
 * skip list, assert patina_status() reflects the partial activation, and
 * then restore full activation in tearDown so later tests aren't affected.
 *
 * We do NOT drive the env-var / constant path end-to-end here: the bridge
 * reads those at mu-plugin load time, which happens once per php-fpm
 * request. Inside a long-running phpunit process we can't unload and reload
 * mu-plugins, so the behavioral test is on `patina_activate($skip)` itself.
 * The bridge's skip-list computation is straightforward enough that its
 * correctness is covered by eyeballing `patina-bridge.php`.
 */
class OverrideTogglesTest extends IntegrationTestCase
{
    protected function tearDown(): void
    {
        // Restore full activation so later tests in this process see the
        // same state they expect from the bridge mu-plugin.
        patina_deactivate();
        patina_activate([]);
        parent::tearDown();
    }

    public function test_baseline_full_activation(): void
    {
        patina_deactivate();
        patina_activate([]);
        $active = patina_status();

        $this->assertContains('esc_html', $active);
        $this->assertContains('esc_attr', $active);
        $this->assertContains('wp_kses', $active);
        $this->assertContains('parse_blocks', $active);
    }

    public function test_skip_esc_disables_only_esc_pair(): void
    {
        patina_deactivate();
        patina_activate(['esc_html', 'esc_attr']);
        $active = patina_status();

        $this->assertNotContains('esc_html', $active);
        $this->assertNotContains('esc_attr', $active);
        $this->assertContains('wp_kses', $active);
        $this->assertContains('parse_blocks', $active);
    }

    public function test_skip_kses_keeps_other_overrides(): void
    {
        patina_deactivate();
        patina_activate(['wp_kses']);
        $active = patina_status();

        $this->assertNotContains('wp_kses', $active);
        $this->assertContains('esc_html', $active);
        $this->assertContains('parse_blocks', $active);
    }

    public function test_skip_parse_blocks_keeps_other_overrides(): void
    {
        patina_deactivate();
        patina_activate(['parse_blocks']);
        $active = patina_status();

        $this->assertNotContains('parse_blocks', $active);
        $this->assertContains('esc_html', $active);
        $this->assertContains('wp_kses', $active);
    }

    public function test_skip_all_disables_everything(): void
    {
        patina_deactivate();
        patina_activate(['esc_html', 'esc_attr', 'wp_kses', 'parse_blocks']);
        $this->assertSame([], patina_status());
    }

    public function test_unknown_skip_names_are_ignored(): void
    {
        patina_deactivate();
        patina_activate(['nonexistent_function', 'also_not_real']);
        $active = patina_status();

        $this->assertContains('esc_html', $active);
        $this->assertContains('wp_kses', $active);
        $this->assertContains('parse_blocks', $active);
    }
}
