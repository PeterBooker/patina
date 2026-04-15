<?php

declare(strict_types=1);

namespace Patina\Tests\Integration;

/**
 * Exercises the worker-level activation cache — a Rust-side AtomicBool
 * plus the `patina_is_activated()` probe the bridge mu-plugin uses to
 * short-circuit every request after the first one. See the commentary
 * on `ACTIVATED` in `crates/patina-ext/src/lib.rs` and the early-return
 * block in `php/bridge/patina-bridge.php` for the full rationale.
 */
class ActivationCacheTest extends IntegrationTestCase
{
    protected function tearDown(): void
    {
        patina_deactivate();
        patina_activate([]);
        parent::tearDown();
    }

    public function test_is_activated_reports_bridge_state(): void
    {
        $this->assertTrue(
            patina_is_activated(),
            'bridge mu-plugin should have activated by the time phpunit boots'
        );

        patina_deactivate();
        $this->assertFalse(
            patina_is_activated(),
            'patina_deactivate() must clear the activation flag'
        );

        patina_activate([]);
        $this->assertTrue(
            patina_is_activated(),
            'patina_activate() must set the activation flag'
        );
    }

    public function test_second_activate_is_noop(): void
    {
        // Baseline: fully activated from the mu-plugin boot.
        $initial_status = patina_status();
        $this->assertContains('wp_kses', $initial_status);

        // A second activate call should short-circuit and leave the
        // already-installed swap set alone — no double-swap, no extra
        // ORIGINALS entries, no re-eval of the shim PHP. We can't probe
        // ORIGINALS directly from PHP but patina_status() reports one
        // entry per saved-original slot, so its count is a proxy.
        $count_before = count($initial_status);
        patina_activate([]);
        $count_after = count(patina_status());

        $this->assertSame(
            $count_before,
            $count_after,
            'second activate must not grow the ORIGINALS slot list'
        );
    }

    public function test_second_activate_ignores_new_skip_list(): void
    {
        // First-call short-circuit semantics: once activated, a
        // follow-up patina_activate() with a DIFFERENT skip list does
        // NOT re-install anything — workers need a full restart to
        // pick up a new config. This is what the bench runner's
        // docker-restart-per-config path relies on.
        $this->assertTrue(patina_is_activated());
        $before = patina_status();

        patina_activate(['wp_kses']); // ask for kses skipped, expect no-op
        $after = patina_status();

        $this->assertSame(
            $before,
            $after,
            'cached activation must ignore subsequent skip-list arguments'
        );
        $this->assertContains(
            'wp_kses',
            $after,
            'wp_kses should remain overridden because the cached activation '
                . 'wins over the new skip list'
        );
    }

    public function test_esc_html_still_works_after_redundant_activate(): void
    {
        // Regression guard: the cached-path return must not interfere
        // with the installed function-table swap. If it did, calling
        // esc_html after a second activate would either crash or return
        // a raw/unchanged string.
        patina_activate([]);
        $this->assertSame(
            '&lt;script&gt;alert(1)&lt;/script&gt;',
            esc_html('<script>alert(1)</script>')
        );
    }
}
