# Plugin Authoring Guide

This guide explains how to create, build, and install plugins for the Software Visualizer Tool (svt).

## Overview

Plugins extend svt with additional capabilities:

- **Constraint evaluators** — custom conformance rules (e.g. naming conventions, dependency limits)
- **Export formats** — additional output formats beyond the built-in Mermaid, JSON, DOT, SVG, PNG
- **Language parsers** — analyzers for additional programming languages

Plugins are compiled as Rust shared libraries (`.dylib`/`.so`/`.dll`), loaded at runtime via the `--plugin` flag or from convention directories.

## Quick Start

1. Create a new Rust library crate:
   ```bash
   cargo new --lib svt-plugin-example
   cd svt-plugin-example
   ```

2. Set up `Cargo.toml`:
   ```toml
   [package]
   name = "svt-plugin-example"
   version = "0.1.0"
   edition = "2021"

   [lib]
   crate-type = ["cdylib"]

   [dependencies]
   svt-core = { path = "../path/to/svt/crates/core" }
   ```

3. Implement the `SvtPlugin` trait in `src/lib.rs`:
   ```rust
   use svt_core::plugin::{SvtPlugin, SVT_PLUGIN_API_VERSION};

   #[derive(Default)]
   struct ExamplePlugin;

   impl SvtPlugin for ExamplePlugin {
       fn name(&self) -> &str { "svt-plugin-example" }
       fn version(&self) -> &str { "0.1.0" }
       fn api_version(&self) -> u32 { SVT_PLUGIN_API_VERSION }
   }

   svt_core::declare_plugin!(ExamplePlugin);
   ```

4. Build the plugin:
   ```bash
   cargo build --release
   ```

5. Create `svt-plugin.toml`:
   ```toml
   [plugin]
   name = "svt-plugin-example"
   version = "0.1.0"
   description = "An example svt plugin"
   api_version = 1
   ```

6. Install the plugin:
   ```bash
   svt plugin install .
   ```

## Cargo.toml Setup

Your plugin crate must produce a C-compatible dynamic library:

```toml
[lib]
crate-type = ["cdylib"]
```

The only required dependency is `svt-core`, which provides the `SvtPlugin` trait and related types:

```toml
[dependencies]
svt-core = { path = "../path/to/svt/crates/core" }
```

> **Important:** Your plugin must be compiled with the **same Rust compiler version** and **same `svt-core` version** as the host `svt` binary. Mismatched versions will cause undefined behavior due to ABI incompatibility.

## Implementing `SvtPlugin`

The `SvtPlugin` trait is defined in `svt_core::plugin`:

```rust
pub trait SvtPlugin: Send + Sync {
    /// Human-readable name of the plugin.
    fn name(&self) -> &str;

    /// Semantic version string (e.g. "0.1.0").
    fn version(&self) -> &str;

    /// Plugin API version. Must match SVT_PLUGIN_API_VERSION.
    fn api_version(&self) -> u32;

    /// Constraint evaluators contributed by this plugin (default: empty).
    fn constraint_evaluators(&self) -> Vec<Box<dyn ConstraintEvaluator>> {
        Vec::new()
    }

    /// Export formats contributed by this plugin (default: empty).
    fn export_formats(&self) -> Vec<Box<dyn ExportFormat>> {
        Vec::new()
    }

    /// Language parsers contributed by this plugin (default: empty).
    fn language_parsers(&self) -> Vec<(LanguageDescriptor, Box<dyn LanguageParser>)> {
        Vec::new()
    }
}
```

Your plugin struct must implement `Default` (used by `declare_plugin!`) and `SvtPlugin`:

```rust
#[derive(Default)]
struct MyPlugin;

impl SvtPlugin for MyPlugin {
    fn name(&self) -> &str { "my-plugin" }
    fn version(&self) -> &str { "0.1.0" }
    fn api_version(&self) -> u32 { SVT_PLUGIN_API_VERSION }
}
```

The `declare_plugin!` macro generates the required `extern "C"` entry point:

```rust
svt_core::declare_plugin!(MyPlugin);
```

## Contributing Constraint Evaluators

Implement `svt_core::conformance::ConstraintEvaluator`:

```rust
use svt_core::conformance::{ConstraintEvaluator, ConstraintResult, ConstraintStatus};
use svt_core::model::{Constraint, Severity, Version};
use svt_core::store::{GraphStore, Result as StoreResult};

#[derive(Debug)]
struct NamingConventionEvaluator;

impl ConstraintEvaluator for NamingConventionEvaluator {
    fn kind(&self) -> &str {
        "naming_convention"
    }

    fn evaluate(
        &self,
        store: &dyn GraphStore,
        constraint: &Constraint,
        version: Version,
    ) -> StoreResult<ConstraintResult> {
        // Your evaluation logic here
        Ok(ConstraintResult {
            constraint_name: constraint.name.clone(),
            constraint_kind: "naming_convention".to_string(),
            status: ConstraintStatus::Pass,
            severity: Severity::Warning,
            message: "All names follow convention".to_string(),
            violations: vec![],
        })
    }
}
```

Return it from `constraint_evaluators()`:

```rust
fn constraint_evaluators(&self) -> Vec<Box<dyn ConstraintEvaluator>> {
    vec![Box::new(NamingConventionEvaluator)]
}
```

## Contributing Export Formats

Implement `svt_core::export::ExportFormat`:

```rust
use svt_core::export::ExportFormat;
use svt_core::model::Version;
use svt_core::store::{GraphStore, Result as StoreResult};

#[derive(Debug)]
struct CsvExporter;

impl ExportFormat for CsvExporter {
    fn name(&self) -> &str {
        "csv"
    }

    fn export(&self, store: &dyn GraphStore, version: Version) -> StoreResult<String> {
        let nodes = store.get_all_nodes(version)?;
        let mut output = String::from("path,kind,sub_kind\n");
        for node in &nodes {
            output.push_str(&format!(
                "{},{:?},{}\n",
                node.canonical_path, node.kind, node.sub_kind
            ));
        }
        Ok(output)
    }
}
```

Return it from `export_formats()`:

```rust
fn export_formats(&self) -> Vec<Box<dyn ExportFormat>> {
    vec![Box::new(CsvExporter)]
}
```

## Contributing Language Parsers

Language parsers allow svt to analyze additional programming languages. You need two things:

1. A `LanguageDescriptor` describing how to discover project units
2. A `LanguageParser` implementation that parses source files

```rust
use svt_core::analysis::{
    LanguageDescriptor, LanguageParser, ParseResult, AnalysisItem, AnalysisRelation,
};
use svt_core::model::NodeKind;

struct JavaParser;

impl LanguageParser for JavaParser {
    fn parse(&self, source: &str, file_path: &str, unit_prefix: &str) -> ParseResult {
        let mut items = Vec::new();
        let mut relations = Vec::new();
        let warnings = Vec::new();

        // Your tree-sitter or custom parsing logic here
        // Extract classes, methods, fields, imports, etc.

        ParseResult { items, relations, warnings }
    }
}
```

Create a `LanguageDescriptor`:

```rust
fn language_parsers(&self) -> Vec<(LanguageDescriptor, Box<dyn LanguageParser>)> {
    let descriptor = LanguageDescriptor {
        language_id: "java".to_string(),
        manifest_files: vec!["pom.xml".to_string(), "build.gradle".to_string()],
        source_extensions: vec![".java".to_string()],
        source_dirs: vec!["src".to_string()],
        exclude_dirs: vec!["target".to_string(), "build".to_string()],
    };
    vec![(descriptor, Box::new(JavaParser))]
}
```

The `LanguageDescriptor` fields:

| Field | Description |
|-------|-------------|
| `language_id` | Unique identifier (e.g. `"java"`) |
| `manifest_files` | Project manifest filenames (e.g. `["pom.xml"]`) |
| `source_extensions` | Source file extensions (e.g. `[".java"]`) |
| `source_dirs` | Directories to scan for source files |
| `exclude_dirs` | Directories to skip during scanning |

## Creating `svt-plugin.toml`

The manifest describes your plugin's metadata and contributions. Place it in your crate root.

### Minimal Example

```toml
[plugin]
name = "svt-plugin-java"
version = "0.1.0"
api_version = 1
```

### Complete Example

```toml
[plugin]
name = "svt-plugin-java"
version = "0.1.0"
description = "Java language analyzer for SVT"
authors = ["Jane Doe <jane@example.com>"]
license = "MIT"
api_version = 1
library = "svt_plugin_java"  # optional, derived from name if omitted

[contributions]
constraint_kinds = []
export_formats = []
language_ids = ["java"]
```

### Field Reference

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Plugin name (conventionally `svt-plugin-<name>`) |
| `version` | Yes | Semantic version string |
| `api_version` | Yes | Must match the host's `SVT_PLUGIN_API_VERSION` (currently `1`) |
| `description` | No | Human-readable description |
| `authors` | No | List of author strings |
| `license` | No | SPDX license identifier |
| `library` | No | Library stem name; defaults to plugin name with hyphens replaced by underscores |
| `contributions.constraint_kinds` | No | List of constraint kind strings |
| `contributions.export_formats` | No | List of export format names |
| `contributions.language_ids` | No | List of language IDs |

## Building and Installing

### Build

```bash
cargo build --release
```

The compiled library will be at `target/release/lib<name>.<ext>` (e.g. `libsvt_plugin_java.dylib` on macOS).

### Install

Copy your manifest and library to a directory, then install:

```bash
# Install to project-local plugins (.svt/plugins/)
svt plugin install /path/to/plugin/directory

# Install to user-global plugins (~/.svt/plugins/)
svt plugin install /path/to/plugin/directory --global

# Overwrite an existing plugin
svt plugin install /path/to/plugin/directory --force
```

### Verify

```bash
# Check plugin metadata
svt plugin info /path/to/plugin/directory

# List all loaded plugins
svt plugin list
```

### Remove

```bash
# Remove from project-local plugins
svt plugin remove svt-plugin-java

# Remove from user-global plugins
svt plugin remove svt-plugin-java --global
```

## Testing Your Plugin

### Manual Testing

Load your plugin directly with the `--plugin` flag:

```bash
# List plugin and its contributions
svt --plugin target/release/libsvt_plugin_java.dylib plugin list

# Run analysis with plugin-contributed language parser
svt --plugin target/release/libsvt_plugin_java.dylib analyze /path/to/java/project

# Run conformance with plugin-contributed constraint evaluator
svt --plugin target/release/libsvt_plugin_java.dylib check
```

### Unit Testing

Test your parser and evaluator implementations in isolation:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_extracts_java_class() {
        let parser = JavaParser;
        let source = r#"
            package com.example;
            public class Foo {
                public void bar() {}
            }
        "#;
        let result = parser.parse(source, "Foo.java", "/com/example");
        assert!(!result.items.is_empty(), "should extract at least one item");
    }
}
```

## API Reference

### Traits

| Trait | Module | Purpose |
|-------|--------|---------|
| `SvtPlugin` | `svt_core::plugin` | Main plugin trait |
| `ConstraintEvaluator` | `svt_core::conformance` | Custom conformance rules |
| `ExportFormat` | `svt_core::export` | Custom output formats |
| `LanguageParser` | `svt_core::analysis` | Source code analysis |

### Key Types

| Type | Module | Purpose |
|------|--------|---------|
| `SVT_PLUGIN_API_VERSION` | `svt_core::plugin` | Current API version constant |
| `LanguageDescriptor` | `svt_core::analysis` | Language discovery configuration |
| `ParseResult` | `svt_core::analysis` | Parser output (items + relations + warnings) |
| `AnalysisItem` | `svt_core::analysis` | Extracted code element (node) |
| `AnalysisRelation` | `svt_core::analysis` | Extracted relationship (edge) |
| `ConstraintResult` | `svt_core::conformance` | Evaluator output |
| `GraphStore` | `svt_core::store` | Read-only access to the graph |

### Macros

| Macro | Module | Purpose |
|-------|--------|---------|
| `declare_plugin!` | `svt_core` | Generates the `extern "C"` entry point |

## Compatibility Notes

- **Same Rust compiler:** Plugin and host must be compiled with the same `rustc` version. The `dyn SvtPlugin` vtable layout is not stable across compiler versions.
- **Same svt-core version:** Plugin must link against the same `svt-core` crate version as the host.
- **API versioning:** `SVT_PLUGIN_API_VERSION` (currently `1`) is checked at load time. If the plugin returns a different version, it will not be loaded.
- **No hot-reloading:** Plugins are loaded once at startup. Changes require restarting svt.
- **In-process:** Plugins run in the same process as svt with full trust. There is no sandboxing.

## Troubleshooting

### Plugin not loaded

```
WARN  failed to load plugin at 'libfoo.dylib': <reason>
```

- Verify the file exists and is a valid shared library
- Check that it was compiled for the correct platform and architecture
- Ensure `rustc` versions match between plugin and host

### Symbol not found

```
WARN  symbol 'svt_plugin_create' not found in 'libfoo.dylib'
```

- Ensure you called `svt_core::declare_plugin!(YourPluginType);`
- Verify `crate-type = ["cdylib"]` in your `Cargo.toml`

### API version mismatch

```
WARN  API version mismatch for plugin 'foo': expected 1, got 2
```

- Update your plugin to return `SVT_PLUGIN_API_VERSION` from `api_version()`
- Rebuild the plugin against the same `svt-core` version as the host

### Crash or undefined behavior

- Ensure plugin and host use the **exact same** `rustc` version
- Ensure plugin links against the **exact same** `svt-core` version
- Verify no `unsafe` code in the plugin introduces memory safety issues

### Library file not found during install

```
Library file 'libfoo.dylib' not found in '/path/to/dir'
```

- Run `cargo build --release` first
- Copy the library from `target/release/` to the same directory as `svt-plugin.toml`
- Or set the `library` field in the manifest to match your library stem name
