---
name: profile-wp-functions
description: Profile WordPress to identify functions suitable for Rust replacement.
disable-model-invocation: true
---

# Profile WordPress Functions

## 1. Start profiling stack

```bash
cd profiling && docker compose up -d
./profiling/setup-wordpress.sh   # if not already installed
```

## 2. Profile with SPX

SPX UI: `http://localhost:8080/?SPX_UI_URI=/&SPX_KEY=dev`

```bash
curl -H "Cookie: SPX_ENABLED=1; SPX_KEY=dev" http://localhost:8080/
curl -H "Cookie: SPX_ENABLED=1; SPX_KEY=dev" http://localhost:8080/patina-benchmark/
curl -H "Cookie: SPX_ENABLED=1; SPX_KEY=dev" http://localhost:8080/?s=lorem
```

## 3. Score candidates

For each function using >0.5% wall time or >50 calls/request, score on:
- **Time %** (30%) — cumulative wall time
- **Call count** (20%) — high frequency = high savings
- **API surface** (20%) — string→string is ideal, complex objects = skip
- **Determinism** (15%) — pure function?
- **WP test coverage** (15%)

Score >= 70: implement. Also note if pluggable, if it calls `apply_filters`, if it depends on DB/state (skip those).

## 4. Clean up

```bash
cd profiling && docker compose down
```
