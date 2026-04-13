PHP_VERSION ?= 8.3
DEV = docker compose -f docker/docker-compose.dev.yml run --rm dev
PROF = docker compose -f profiling/docker-compose.yml
export DOCKER_BUILDKIT = 1

.PHONY: help build test test-rust test-php test-integration bench bench-wp bench-jit bench-rust bench-http bench-full bench-compare bench-baseline check clean fixtures shell

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

test-integration: ## Run WordPress integration tests (spins up profiling stack)
	@echo "=== Building extension for PHP 8.3 ==="
	docker build --build-arg PHP_VERSION=8.3 -f docker/Dockerfile.build --target builder -t patina-build-8.3 . > /dev/null
	docker run --rm patina-build-8.3 cat /src/target/release/libpatina.so > /tmp/patina-8.3.so
	@echo "=== Ensuring PHPUnit is installed ==="
	$(DEV) sh -c 'cd php && composer update --no-interaction --quiet'
	@echo "=== Starting WordPress stack ==="
	$(PROF) up -d
	@EXT_DIR=$$($(PROF) exec php-fpm php -r "echo ini_get('extension_dir');") && \
		$(PROF) cp /tmp/patina-8.3.so php-fpm:$$EXT_DIR/patina.so && \
		$(PROF) exec php-fpm bash -c 'echo "extension=patina.so" > /usr/local/etc/php/conf.d/patina.ini' && \
		$(PROF) exec php-fpm bash -c 'mkdir -p /var/www/html/wp-content/mu-plugins && cp /app/php/bridge/patina-bridge.php /var/www/html/wp-content/mu-plugins/'
	$(PROF) restart php-fpm
	@sleep 2
	@echo "=== Running integration tests ==="
	$(PROF) exec -T php-fpm php -d memory_limit=512M /app/php/vendor/bin/phpunit -c /app/php/tests-integration/phpunit.xml

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
	$(PROF) exec php-fpm php -d memory_limit=512M /app/php/benchmarks/bench-kses.php

bench-jit: build ## Run PHP benchmarks with JIT enabled
	$(DEV) php \
		-d extension=/app/target/release/libpatina.so \
		-d opcache.enable_cli=1 \
		-d opcache.jit_buffer_size=128M \
		-d opcache.jit=1255 \
		php/benchmarks/run.php 50000

bench-rust: ## Run Criterion benchmarks
	$(DEV) cargo bench -p patina-bench

bench-http: ## Run HTTP-level bench (k6 against the profiling stack)
	@echo "=== Building extension for PHP 8.3 ==="
	docker build --build-arg PHP_VERSION=8.3 -f docker/Dockerfile.build --target builder -t patina-build-8.3 . > /dev/null
	docker run --rm patina-build-8.3 cat /src/target/release/libpatina.so > /tmp/patina-8.3.so
	@echo "=== Starting profiling stack ==="
	$(PROF) up -d
	@EXT_DIR=$$($(PROF) exec php-fpm php -r "echo ini_get('extension_dir');") && \
		$(PROF) cp /tmp/patina-8.3.so php-fpm:$$EXT_DIR/patina.so && \
		$(PROF) exec php-fpm bash -c 'echo "extension=patina.so" > /usr/local/etc/php/conf.d/patina.ini' && \
		$(PROF) exec php-fpm bash -c 'mkdir -p /var/www/html/wp-content/mu-plugins && cp /app/php/bridge/patina-bridge.php /var/www/html/wp-content/mu-plugins/'
	$(PROF) restart php-fpm
	@sleep 2
	@RUN_DIR=/tmp/patina-bench/$$(date -u +%Y%m%dT%H%M%SZ) && \
		mkdir -p $$RUN_DIR && \
		echo "=== Running k6 workloads ($$RUN_DIR) ===" && \
		$(PROF) exec -T \
			-e BASE_URL=http://nginx \
			-e ITERATIONS=$${ITERATIONS:-100} \
			-e WARMUP=$${WARMUP:-5} \
			php-fpm k6 run \
				--quiet \
				--out json=/tmp/k6-output.json \
				/app/profiling/k6-workloads.js && \
		$(PROF) exec -T php-fpm cat /tmp/k6-output.json > $$RUN_DIR/k6-output.json && \
		echo "Output: $$RUN_DIR/k6-output.json"

bench-full: ## Run the full per-config bench matrix (stock + 4 patina configs)
	@ITERATIONS=$${ITERATIONS:-100} WARMUP=$${WARMUP:-5} \
		CONFIGS=$${CONFIGS:-stock,esc_only,kses_only,parse_blocks_only,full_patina} \
		bash scripts/bench-runner.sh

bench-compare: ## Compare one run (intra) or two runs (cross). Usage: make bench-compare RUN=/tmp/... [TO=/tmp/...]
	@if [ -z "$${RUN:-}" ]; then echo "usage: make bench-compare RUN=/path/to/run [TO=/path/to/other-run]"; exit 2; fi
	@if [ -n "$${TO:-}" ]; then \
		python3 scripts/bench-compare.py "$$RUN" "$$TO" $${BASELINE:+--baseline $$BASELINE} $${FAIL_ON_REGRESS:+--fail-on-regress $$FAIL_ON_REGRESS}; \
	else \
		python3 scripts/bench-compare.py "$$RUN" $${BASELINE:+--baseline $$BASELINE} $${FAIL_ON_REGRESS:+--fail-on-regress $$FAIL_ON_REGRESS}; \
	fi

bench-baseline: ## Run bench-full and commit the result under fixtures/baselines/
	@if [ -z "$${NAME:-}" ]; then echo "usage: make bench-baseline NAME=phase6-initial"; exit 2; fi
	@RUN_DIR=fixtures/baselines/$${NAME} ITERATIONS=$${ITERATIONS:-100} WARMUP=$${WARMUP:-5} \
		bash scripts/bench-runner.sh
	@echo "Baseline written to fixtures/baselines/$${NAME}"
	@echo "Review and git add fixtures/baselines/$${NAME} when you're happy with it."

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
