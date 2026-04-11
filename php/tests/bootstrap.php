<?php

declare(strict_types=1);

if (!extension_loaded('patina-ext')) {
    fwrite(STDERR, "FATAL: patina extension not loaded. Cannot run tests.\n");
    fwrite(STDERR, "Load it with: php -d extension=/path/to/libpatina.so\n");
    exit(1);
}

// Autoload test dependencies
require_once __DIR__ . '/../vendor/autoload.php';
