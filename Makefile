PHP_VERSION ?= 8.3
DEV = docker compose -f docker/docker-compose.dev.yml run --rm dev

.PHONY: help build test test-rust test-php bench check clean fixtures shell

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

bench: build ## Run PHP benchmarks
	$(DEV) php -d extension=/app/target/release/libpatina.so php/benchmarks/run.php 50000

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
	cd profiling && docker compose up -d && \
	sleep 5 && \
	docker compose exec php-fpm php -d memory_limit=512M \
		/app/php/fixture-generator/generate.php --all --output=/app/fixtures/ && \
	docker compose down

shell: ## Open a shell in the dev container
	$(DEV) bash

clean: ## Remove build artifacts
	$(DEV) cargo clean
	docker compose -f docker/docker-compose.dev.yml down -v 2>/dev/null || true
