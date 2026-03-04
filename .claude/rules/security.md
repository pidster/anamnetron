# Security Standards

## General

- Never execute user-supplied input as code or shell commands
- All file system paths from user input must be canonicalized and validated against an allowed root
- No network calls during normal operation — external access is strictly opt-in and explicit
- Dependencies are audited (`cargo audit`) — new dependencies require justification

## Rust-Specific

- No `unsafe` without documented justification, a safety comment, and explicit review
- Use `Path::canonicalize()` or validated path joins — never string concatenation for paths
- Plugin execution is sandboxed — plugins cannot access the filesystem outside their declared scope
- Serialization/deserialization uses strict schemas — reject unknown fields by default
- Use constant-time comparison for any security-sensitive string comparisons

## Web Frontend

- No `innerHTML` or `@html` with user-controlled content — use text interpolation
- CSP headers configured on the server — no inline scripts in production
- WASM module loaded from same-origin only
- No secrets or credentials in frontend code or build artifacts

## API Server

- All API inputs are validated and bounded (max sizes, allowed characters, depth limits)
- Error responses never leak internal paths, stack traces, or implementation details
- Rate limiting on resource-intensive endpoints (analysis, export)
- CORS is restrictive by default — only same-origin unless explicitly configured
