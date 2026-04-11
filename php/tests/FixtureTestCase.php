<?php

declare(strict_types=1);

namespace Patina\Tests;

use PHPUnit\Framework\TestCase;

/**
 * Base test case that loads JSON fixtures and provides common test patterns.
 */
abstract class FixtureTestCase extends TestCase
{
    protected static string $fixturesDir = __DIR__ . '/../../fixtures';

    /**
     * Load fixtures for a given function name.
     *
     * @return array<int, array{input: list<mixed>, output: mixed}>
     */
    protected static function loadFixtures(string $function): array
    {
        $path = static::$fixturesDir . "/{$function}.json";
        if (!file_exists($path)) {
            static::markTestSkipped("Fixture file not found: {$path}. Run generate-fixtures.sh first.");
        }

        $data = json_decode(file_get_contents($path), true, 512, JSON_THROW_ON_ERROR);
        assert(is_array($data), "Fixture file must contain a JSON array: {$path}");
        return $data;
    }

    /**
     * Convert fixtures into PHPUnit data provider format.
     *
     * @return array<string, array{0: list<mixed>, 1: mixed}>
     */
    protected static function fixturesAsProvider(string $function): array
    {
        $cases = [];
        foreach (static::loadFixtures($function) as $i => $fixture) {
            $label = substr(json_encode($fixture['input'][0] ?? ''), 0, 60);
            $cases["fixture_{$i}_{$label}"] = [$fixture['input'], $fixture['output']];
        }
        return $cases;
    }
}
