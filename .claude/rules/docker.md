---
paths:
  - "docker/**"
  - "profiling/**"
  - "Makefile"
  - ".dockerignore"
---

# Docker & Infrastructure Rules

## Dockerfiles

- All PHP containers use `PHP_VERSION` build arg, defaulting to 8.3.
- WP-CLI needs `memory_limit=512M` — PHP's default 128M is not enough for extraction.
- The build Dockerfile has a verification step: `php -d extension=... -r "echo patina_version();"`.

## Makefile

- All targets use `$(DEV)` which runs inside the Docker dev container.
- The `DEV` variable is `docker compose -f docker/docker-compose.dev.yml run --rm dev`.
- `test-php` depends on `build` (needs the .so to exist).
- Cargo and Composer caches are Docker volumes — persistent between runs.

## Profiling stack

- The repo is mounted read-only at `/app` in the profiling container, but `fixtures/` is mounted read-write for fixture generation.
- WordPress data lives in Docker volumes (`wordpress`, `dbdata`). Use `docker compose down -v` to reset.
- Extension must be built for PHP 8.3 (matching the container), not the local PHP version.
