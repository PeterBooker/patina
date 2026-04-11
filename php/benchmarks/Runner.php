<?php

declare(strict_types=1);

namespace Patina\Benchmarks;

class Runner
{
    private int $iterations;
    /** @var list<array{function: string, input_label: string, php_ms: float, rust_ms: float, speedup: string, iterations: int}> */
    private array $results = [];

    public function __construct(int $iterations = 10_000)
    {
        $this->iterations = $iterations;
    }

    /**
     * Benchmark a PHP reference implementation against the Rust extension version.
     *
     * @param string $name Human-readable function name
     * @param string $inputLabel Description of the input
     * @param callable $phpFn Reference PHP implementation
     * @param callable $rustFn Rust extension function
     * @param array $args Arguments to pass to both functions
     */
    public function run(string $name, string $inputLabel, callable $phpFn, callable $rustFn, array $args): void
    {
        // Warmup
        for ($i = 0; $i < min(100, $this->iterations); $i++) {
            $phpFn(...$args);
            $rustFn(...$args);
        }

        // Benchmark PHP
        $start = hrtime(true);
        for ($i = 0; $i < $this->iterations; $i++) {
            $phpFn(...$args);
        }
        $phpNs = hrtime(true) - $start;

        // Benchmark Rust
        $start = hrtime(true);
        for ($i = 0; $i < $this->iterations; $i++) {
            $rustFn(...$args);
        }
        $rustNs = hrtime(true) - $start;

        $this->results[] = [
            'function' => $name,
            'input_label' => $inputLabel,
            'php_ms' => $phpNs / 1_000_000,
            'rust_ms' => $rustNs / 1_000_000,
            'speedup' => sprintf('%.1fx', $phpNs / max($rustNs, 1)),
            'iterations' => $this->iterations,
        ];
    }

    public function report(): void
    {
        printf("\n%-35s %-15s %12s %12s %10s\n",
            'Function', 'Input', 'PHP (ms)', 'Rust (ms)', 'Speedup');
        printf("%s\n", str_repeat('─', 88));

        foreach ($this->results as $r) {
            printf("%-35s %-15s %12.2f %12.2f %10s\n",
                $r['function'],
                $r['input_label'],
                $r['php_ms'],
                $r['rust_ms'],
                $r['speedup']);
        }

        printf("\nIterations per benchmark: %s\n", number_format($this->results[0]['iterations'] ?? 0));
    }

    /** @return list<array{function: string, input_label: string, php_ms: float, rust_ms: float, speedup: string, iterations: int}> */
    public function getResults(): array
    {
        return $this->results;
    }
}
