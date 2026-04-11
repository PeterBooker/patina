PHP_VERSION ?= 8.3
DEV = docker compose -f docker/docker-compose.dev.yml run --rm dev
PROF = docker compose -f profiling/docker-compose.yml

.PHONY: help build test test-rust test-php bench bench-wp bench-jit bench-rust check clean fixtures shell

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'

build: ## Build the extension (release)
	$(DEV) cargo build --release -p patina-ext

test: test-rust test-php ## Run all tests

test-rust: ## Run Rust tests
	$(DEV) cargo test --workspace

test-php: build ## Run PHP tests
	$(DEV) sh -c '\
		cd php && \
		composer update --no-interaction --quiet && \
		php -d extension=/app/target/release/libpatina.so vendor/bin/phpunit'

bench: build ## Run PHP benchmarks (escaping + sanitize_redirect)
	$(DEV) php -d extension=/app/target/release/libpatina.so php/benchmarks/run.php 50000

bench-wp: ## Run all PHP benchmarks including kses (requires WordPress)
	@echo "=== Building extension for PHP 8.3 ==="
	docker build --build-arg PHP_VERSION=8.3 -f docker/Dockerfile.build --target builder -t patina-build-8.3 . > /dev/null
	docker run --rm patina-build-8.3 cat /src/target/release/libpatina.so > /tmp/patina-8.3.so
	@echo "=== Starting WordPress stack ==="
	$(PROF) up -d
	@EXT_DIR=$$($(PROF) exec php-fpm php -r "echo ini_get('extension_dir');") && \
		$(PROF) cp /tmp/patina-8.3.so php-fpm:$$EXT_DIR/patina.so && \
		$(PROF) exec php-fpm bash -c 'echo "extension=patina.so" > /usr/local/etc/php/conf.d/patina.ini'
	$(PROF) restart php-fpm
	@sleep 2
	@echo "=== Running benchmarks ==="
	$(PROF) exec php-fpm php -d memory_limit=512M -r '\
		require "/var/www/html/wp-load.php"; \
		require "/app/php/benchmarks/Runner.php"; \
		use Patina\Benchmarks\Runner; \
		$$bench = new Runner(10000); \
		$$inputs = [ \
			"small_76B" => "<p>Simple <b>paragraph</b> with <a href=\"http://example.com\">link</a>.</p>", \
			"medium_740B" => str_repeat("<p>Paragraph <b>bold</b> and <a href=\"http://example.com\">link</a>.</p>", 10), \
			"with_script" => str_repeat("<p>Safe <script>alert(\"xss\")</script> text</p>", 5), \
			"large_3KB" => str_repeat("<div class=\"container\"><p>Content with <strong>formatting</strong>, <a href=\"http://example.com\" title=\"Link\">links</a>, and &amp; entities.</p></div>", 20), \
		]; \
		foreach ($$inputs as $$label => $$input) { \
			$$bench->run("wp_kses_post", $$label, "wp_kses_post", "patina_wp_kses_post", [$$input]); \
		} \
		$$bench->report();'

bench-jit: build ## Run PHP benchmarks with JIT enabled
	$(DEV) php \
		-d extension=/app/target/release/libpatina.so \
		-d opcache.enable_cli=1 \
		-d opcache.jit_buffer_size=128M \
		-d opcache.jit=1255 \
		php/benchmarks/run.php 50000

bench-rust: ## Run Criterion benchmarks
	$(DEV) cargo bench -p patina-bench

check: ## Run all checks (test + clippy + fmt)
	$(DEV) sh -c '\
		cargo fmt --all --check && \
		cargo clippy --workspace -- -D warnings && \
		cargo test --workspace && \
		cd php && \
		composer update --no-interaction --quiet && \
		php -d extension=/app/target/release/libpatina.so vendor/bin/phpunit'

fmt: ## Format Rust code
	$(DEV) cargo fmt --all

clippy: ## Run clippy lints
	$(DEV) cargo clippy --workspace -- -D warnings

verify: build ## Verify the extension loads and print info
	$(DEV) php -d extension=/app/target/release/libpatina.so -r "\
		echo 'Extension: ' . (extension_loaded('patina-ext') ? 'loaded' : 'NOT loaded') . PHP_EOL; \
		echo 'Version: ' . patina_version() . PHP_EOL; \
		echo 'Functions: ' . implode(', ', get_extension_funcs('patina-ext')) . PHP_EOL;"

fixtures: ## Generate test fixtures from WordPress (requires profiling stack)
	$(PROF) up -d
	@sleep 5
	$(PROF) exec php-fpm php -d memory_limit=512M \
		/app/php/fixture-generator/generate.php --all --output=/app/fixtures/

shell: ## Open a shell in the dev container
	$(DEV) bash

clean: ## Remove build artifacts
	$(DEV) cargo clean
	docker compose -f docker/docker-compose.dev.yml down -v 2>/dev/null || true
