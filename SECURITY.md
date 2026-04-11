# Security Policy

## Scope

Patina replaces security-sensitive WordPress functions (escaping, sanitization). A behavioral mismatch could introduce XSS or injection vulnerabilities. We take correctness seriously.

## Reporting a Vulnerability

If you discover a security issue, please **do not** open a public GitHub issue.

Instead, email **peter@peterbooker.com** with:
- Which function is affected
- Input that produces incorrect output
- Expected vs actual output
- WordPress version tested against

You should receive a response within 72 hours.

## Supported Versions

| Version | Supported |
|---|---|
| Latest release | Yes |
| Older releases | No — upgrade to latest |

## Security Measures

- Every function is validated against WordPress fixtures (byte-identical output)
- Fuzz testing runs in CI (millions of random inputs)
- `catch_unwind` prevents panics from crashing PHP-FPM workers
- No `unsafe` code in `patina-core`
