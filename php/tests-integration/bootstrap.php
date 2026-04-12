<?php

declare(strict_types=1);

// Integration-test bootstrap: runs inside the profiling stack's php-fpm
// container where WordPress is fully installed at /var/www/html and the
// patina-bridge mu-plugin has been copied into wp-content/mu-plugins/.
//
// require'ing wp-load.php triggers the normal WP boot sequence, including
// mu-plugin load → patina_activate() → function table swap + shim install.

if (!extension_loaded("patina-ext")) {
    fwrite(
        STDERR,
        "FATAL: patina extension not loaded. Cannot run integration tests.\n",
    );
    exit(1);
}

if (!file_exists("/var/www/html/wp-load.php")) {
    fwrite(
        STDERR,
        "FATAL: WordPress not found at /var/www/html/wp-load.php. " .
            "Is the profiling stack running?\n",
    );
    exit(1);
}

// Silence the "ABSPATH not defined, PHP sessions, etc." noise WP emits on CLI.
define("WP_USE_THEMES", false);

require_once "/var/www/html/wp-load.php";

// Confirm the bridge ran — integration tests assume wp_kses is overridden.
if (!in_array("wp_kses", patina_status(), true)) {
    fwrite(
        STDERR,
        "FATAL: wp_kses override not active. Is the bridge mu-plugin installed?\n",
    );
    exit(1);
}

// Composer autoloader (for PHPUnit itself).
require_once __DIR__ . "/../vendor/autoload.php";

// Our integration tests live outside the psr-4 tests/ namespace configured
// in composer.json, so load the base class explicitly.
require_once __DIR__ . "/IntegrationTestCase.php";
