# Canonical Path Mapping Rules

This document defines the rules for mapping language-specific qualified names to and from canonical paths. See [DATA_MODEL.md](./DATA_MODEL.md) for the canonical path schema definition.

## Core Transformation

Every analyzer applies three transformations to produce a canonical path:

1. **Separator replacement** — language separator → `/`
2. **Case normalization** — language convention → `kebab-case`
3. **Prefix stripping** — remove boilerplate prefixes (configurable)

## Case Normalization

The core provides a `to_kebab_case()` utility that handles all source conventions:

| Input convention | Example | Output |
|---|---|---|
| `snake_case` | `create_order` | `create-order` |
| `PascalCase` | `CreateOrder` | `create-order` |
| `camelCase` | `createOrder` | `create-order` |
| `SCREAMING_SNAKE` | `MAX_RETRIES` | `max-retries` |
| `kebab-case` | `create-order` | `create-order` (no-op) |

### Acronym Handling

Consecutive capitals are treated as a single word until a lowercase letter signals a new word boundary:

| Input | Output | Rationale |
|---|---|---|
| `HTMLParser` | `html-parser` | `HTML` is one word, `Parser` is another |
| `IOStream` | `io-stream` | `IO` is one word |
| `Base64Encoder` | `base64-encoder` | `Base64` is one word |
| `getHTTPResponse` | `get-http-response` | Three words |
| `XMLToJSON` | `xml-to-json` | Three words |

The boundary rule: insert a word break before a capital letter that is followed by a lowercase letter, or between a lowercase/digit and an uppercase letter. This is a well-established algorithm (used by serde `rename_all`, the Heck crate, etc.).

## Per-Language Rules

### Rust

| Aspect | Rule |
|---|---|
| Separator | `::` → `/` |
| Case | `snake_case` modules/functions → `kebab-case`; `PascalCase` types → `kebab-case` |
| Crate root | Crate name becomes the first segment |
| Crate name normalization | Cargo treats `-` and `_` as equivalent; normalize to kebab-case (`my_crate` → `my-crate`) |
| `pub use` re-exports | Follow the re-export — canonical path uses the public API path, not the internal module path |
| `impl` blocks | Not a node themselves; methods inside become children of the type they implement |

```
// Code
payments_service::handlers::CreateOrder::execute

// Canonical
/payments-service/handlers/create-order/execute
```

### Java

| Aspect | Rule |
|---|---|
| Separator | `.` → `/` |
| Case | `lowercase` packages → `kebab-case` (usually no-op); `PascalCase` classes → `kebab-case` |
| Prefix stripping | **Configurable** — strip organizational prefix (e.g., `com.example.myorg.`) |
| Maven module | Module name becomes the service-level segment, not the package path |
| Inner classes | `Outer.Inner` or `Outer$Inner` → `/outer/inner` (inner class is a child node) |
| Method overloads | Disambiguate with parameter types in metadata, not in the canonical path — the path uses the method name only |

```
// Code
com.example.payments.handlers.CreateOrder.execute

// With prefix strip: "com.example."
// Canonical
/payments/handlers/create-order/execute
```

Prefix stripping is critical for Java — without it, `com.example.myorg` becomes meaningless noise in every path. The analyzer configuration specifies which prefix to strip:

```yaml
analyzer:
  java:
    strip_prefix: "com.example."
```

### Python

| Aspect | Rule |
|---|---|
| Separator | `.` → `/` |
| Case | `snake_case` modules/functions → `kebab-case`; `PascalCase` classes → `kebab-case` |
| `__init__.py` | The package directory is the node, `__init__.py` is not a separate segment |
| Private convention | `_private_module` → `private-module` (leading underscore stripped — it's visibility metadata, not part of the name) |
| Dunder methods | `__init__`, `__str__` → `init`, `str` (strip dunders, store original in metadata) |

```
// Code
payments_service.handlers.CreateOrder.execute

// Canonical
/payments-service/handlers/create-order/execute
```

### C# (.NET)

| Aspect | Rule |
|---|---|
| Separator | `.` → `/` |
| Case | `PascalCase` everything → `kebab-case` |
| Prefix stripping | **Configurable** — strip company/product namespace prefix |
| Assembly vs namespace | Assembly name defines the service-level segment; namespace path defines components |
| Partial classes | Single node — the analyzer merges partial class definitions |
| Extension methods | Attributed to the static class that defines them, not the type they extend |

```
// Code
Payments.Handlers.CreateOrder.Execute

// Canonical
/payments/handlers/create-order/execute
```

### TypeScript / JavaScript

| Aspect | Rule |
|---|---|
| Separator | `/` (file paths, already canonical) |
| Case | `camelCase` functions → `kebab-case`; `PascalCase` classes → `kebab-case`; file names may already be `kebab-case` |
| npm scope | `@org/package-name` → strip `@org/`, package name becomes service segment |
| Barrel exports | Follow the public API path (`index.ts` re-exports), not the internal file path |
| Path aliases | Resolve aliases (`@/components/Button` → actual path) before canonicalization |
| File = module | Each file is typically a component; default export name or file name is used |

```
// Code (file path based)
@payments/handlers/createOrder

// Canonical
/payments/handlers/create-order
```

## Configurable Overrides

The core convention handles most cases, but projects need escape hatches.

### Analyzer Configuration (per-project)

```yaml
canonical_mapping:
  # Global overrides — exact renames
  overrides:
    /com-example-payments: /payments
    /src-main-java: ""              # strip build directory segments

  # Per-language settings
  rust:
    crate_prefix: ""                # no prefix stripping needed
  java:
    strip_prefix: "com.example."
  csharp:
    strip_prefix: "MyCompany.MyProduct."
  python:
    strip_prefix: ""
  typescript:
    strip_scope: "@myorg"
```

The `from_canonical()` reverse mapping uses the same configuration to reconstruct the language-specific name when navigating from the graph back to source code.

## Collision Handling

Two different language constructs could map to the same canonical path. For example, a Python function `create_order` and a class `CreateOrder` in the same module both map to `create-order`.

Resolution:

1. The `sub_kind` field differentiates them in the graph — they are distinct nodes with the same canonical path.
2. Path queries that return multiple nodes are filtered by the caller using `kind` or `sub_kind`.
3. The canonical path is not mutated for disambiguation — node identity is `(canonical_path, kind)` in practice.

In most languages this is rare — naming conventions typically prevent a function and a class from having equivalent names in the same scope.
