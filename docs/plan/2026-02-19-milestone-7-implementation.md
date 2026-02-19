# TypeScript Analyzer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a TypeScript/Svelte analyzer to `crates/analyzer/`, proving the multi-language architecture works and dog-fooding on the `web/` frontend.

**Architecture:** Extends the existing analyzer with a TypeScript language driver (`languages/typescript.rs`), a Svelte script extractor (`languages/svelte.rs`), and TypeScript package discovery (`discovery.rs`). Reuses the existing `LanguageAnalyzer` trait, `map_to_graph()` pipeline, and `qualified_name_to_canonical()` mapping unchanged. Both Rust and TypeScript analysis feed into a single analysis snapshot.

**Tech Stack:** tree-sitter-typescript 0.23 (compatible with existing tree-sitter 0.24), lightweight Svelte `<script>` block extraction (no grammar crate).

---

### Task 1: Add tree-sitter-typescript Dependency

**Files:**
- Modify: `crates/analyzer/Cargo.toml`

**Step 1: Add the dependency**

Add `tree-sitter-typescript = "0.23"` to `[dependencies]` in `crates/analyzer/Cargo.toml`, after the `tree-sitter-rust` line:

```toml
tree-sitter-typescript = "0.23"
```

**Step 2: Verify it compiles**

Run: `cargo check -p svt-analyzer`
Expected: Compiles successfully with no errors.

**Step 3: Commit**

```bash
git add crates/analyzer/Cargo.toml Cargo.lock
git commit -m "chore(analyzer): add tree-sitter-typescript dependency"
```

---

### Task 2: Svelte Script Block Extraction

**Files:**
- Create: `crates/analyzer/src/languages/svelte.rs`
- Modify: `crates/analyzer/src/languages/mod.rs`

**Step 1: Write the failing tests**

Create `crates/analyzer/src/languages/svelte.rs` with the module doc comment, types, a stub `extract_script_blocks` function that returns an empty vec, and tests:

```rust
//! Svelte script block extraction.
//!
//! Extracts `<script>` block content from `.svelte` files so it can be
//! parsed as TypeScript by tree-sitter-typescript.

/// A script block extracted from a Svelte file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptBlock {
    /// The TypeScript/JavaScript content of the script block.
    pub content: String,
    /// The line offset of the `<script>` tag in the original `.svelte` file.
    /// Used to adjust source_ref line numbers.
    pub line_offset: usize,
    /// Whether this is a `<script context="module">` block.
    pub is_module: bool,
}

/// Extract `<script>` blocks from a Svelte source file.
///
/// Finds `<script>` or `<script lang="ts">` tags, extracts the content
/// between the opening and closing tags, and records the line offset for
/// correct source_ref generation.
pub fn extract_script_blocks(source: &str) -> Vec<ScriptBlock> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_simple_script_block() {
        let source = r#"<script lang="ts">
export let name: string = "world";
</script>

<h1>Hello {name}!</h1>"#;
        let blocks = extract_script_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].content.trim(), r#"export let name: string = "world";"#);
        assert_eq!(blocks[0].line_offset, 0);
        assert!(!blocks[0].is_module);
    }

    #[test]
    fn extracts_module_script_block() {
        let source = r#"<script context="module" lang="ts">
export function helper() { return 42; }
</script>

<script lang="ts">
let count = 0;
</script>"#;
        let blocks = extract_script_blocks(source);
        assert_eq!(blocks.len(), 2);
        assert!(blocks.iter().any(|b| b.is_module), "should find module script");
        assert!(blocks.iter().any(|b| !b.is_module), "should find instance script");
    }

    #[test]
    fn extracts_script_without_lang_attribute() {
        let source = r#"<script>
let x = 1;
</script>"#;
        let blocks = extract_script_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].content.trim(), "let x = 1;");
    }

    #[test]
    fn line_offset_is_correct() {
        let source = "<p>hello</p>\n<p>world</p>\n<script lang=\"ts\">\nlet x = 1;\n</script>";
        let blocks = extract_script_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].line_offset, 2, "script tag is on line 2 (0-indexed)");
    }

    #[test]
    fn no_script_block_returns_empty() {
        let source = "<h1>Hello world</h1>\n<p>No script here</p>";
        let blocks = extract_script_blocks(source);
        assert!(blocks.is_empty());
    }

    #[test]
    fn malformed_unclosed_script_returns_empty() {
        let source = "<script lang=\"ts\">\nlet x = 1;";
        let blocks = extract_script_blocks(source);
        assert!(blocks.is_empty(), "unclosed script tag should be skipped");
    }
}
```

**Step 2: Wire up the module**

In `crates/analyzer/src/languages/mod.rs`, add after `pub mod rust;`:

```rust
pub mod svelte;
```

**Step 3: Run tests to verify they fail**

Run: `cargo test -p svt-analyzer svelte -- --nocapture`
Expected: 5 tests fail (stub returns empty vec), 1 passes (no_script_block_returns_empty).

**Step 4: Implement extract_script_blocks**

Replace the stub function body in `svelte.rs`:

```rust
pub fn extract_script_blocks(source: &str) -> Vec<ScriptBlock> {
    let mut blocks = Vec::new();
    let mut search_from = 0;

    while let Some(open_start) = source[search_from..].find("<script") {
        let abs_open_start = search_from + open_start;

        // Find the end of the opening tag
        let Some(open_end) = source[abs_open_start..].find('>') else {
            break;
        };
        let abs_open_end = abs_open_start + open_end + 1;

        // Extract the opening tag text to check attributes
        let tag_text = &source[abs_open_start..abs_open_end];

        // Check for context="module"
        let is_module = tag_text.contains("context=\"module\"")
            || tag_text.contains("context='module'");

        // Find the closing </script> tag
        let Some(close_start) = source[abs_open_end..].find("</script>") else {
            break; // Unclosed tag — skip
        };
        let abs_close_start = abs_open_end + close_start;

        // Extract content between tags
        let content = source[abs_open_end..abs_close_start].to_string();

        // Calculate line offset (count newlines before the opening tag)
        let line_offset = source[..abs_open_start].matches('\n').count();

        blocks.push(ScriptBlock {
            content,
            line_offset,
            is_module,
        });

        search_from = abs_close_start + "</script>".len();
    }

    blocks
}
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p svt-analyzer svelte`
Expected: All 6 tests pass.

**Step 6: Commit**

```bash
git add crates/analyzer/src/languages/svelte.rs crates/analyzer/src/languages/mod.rs
git commit -m "feat(analyzer): add Svelte script block extraction"
```

---

### Task 3: TypeScript Package Discovery

**Files:**
- Modify: `crates/analyzer/src/discovery.rs`
- Modify: `crates/analyzer/src/types.rs`

**Step 1: Add TsPackageInfo type to types.rs**

Add after the `ProjectLayout` struct in `crates/analyzer/src/types.rs`:

```rust
/// Information about a TypeScript/JavaScript package.
#[derive(Debug, Clone)]
pub struct TsPackageInfo {
    /// Package name (from package.json "name" field).
    pub name: String,
    /// Root directory of the package (where package.json lives).
    pub root: PathBuf,
    /// Source root directory (typically root/src/).
    pub source_root: PathBuf,
    /// All .ts, .tsx, .svelte source files under the source root.
    pub source_files: Vec<PathBuf>,
}
```

**Step 2: Write the failing tests in discovery.rs**

Add these tests to the existing `mod tests` in `discovery.rs`:

```rust
    #[test]
    fn discovers_ts_package_from_package_json() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name": "my-app"}"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/index.ts"), "export const x = 1;").unwrap();

        let packages = discover_ts_packages(dir.path()).unwrap();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "my-app");
        assert!(!packages[0].source_files.is_empty());
    }

    #[test]
    fn ts_discovery_skips_node_modules() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name": "my-app"}"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/app.ts"), "export const x = 1;").unwrap();
        std::fs::create_dir_all(dir.path().join("node_modules/dep")).unwrap();
        std::fs::write(
            dir.path().join("node_modules/dep/package.json"),
            r#"{"name": "dep"}"#,
        )
        .unwrap();

        let packages = discover_ts_packages(dir.path()).unwrap();
        assert_eq!(packages.len(), 1, "should only find root package, not node_modules");
    }

    #[test]
    fn ts_discovery_collects_svelte_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name": "svelte-app"}"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("src/components")).unwrap();
        std::fs::write(dir.path().join("src/main.ts"), "import App from './App.svelte';").unwrap();
        std::fs::write(
            dir.path().join("src/components/App.svelte"),
            "<script lang=\"ts\">\nlet x = 1;\n</script>",
        )
        .unwrap();

        let packages = discover_ts_packages(dir.path()).unwrap();
        assert_eq!(packages.len(), 1);
        let svelte_files: Vec<_> = packages[0]
            .source_files
            .iter()
            .filter(|f| f.extension().is_some_and(|e| e == "svelte"))
            .collect();
        assert!(!svelte_files.is_empty(), "should collect .svelte files");
    }

    #[test]
    fn ts_discovery_skips_test_and_declaration_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name": "my-app"}"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/app.ts"), "export const x = 1;").unwrap();
        std::fs::write(dir.path().join("src/app.test.ts"), "test('x', () => {});").unwrap();
        std::fs::write(dir.path().join("src/app.spec.ts"), "test('x', () => {});").unwrap();
        std::fs::write(dir.path().join("src/types.d.ts"), "declare module 'x';").unwrap();

        let packages = discover_ts_packages(dir.path()).unwrap();
        assert_eq!(packages[0].source_files.len(), 1, "should only include app.ts");
    }

    #[test]
    fn ts_discovery_falls_back_to_dir_name_when_no_name_field() {
        let dir = TempDir::new().unwrap();
        let pkg_dir = dir.path().join("my-project");
        std::fs::create_dir_all(pkg_dir.join("src")).unwrap();
        std::fs::write(pkg_dir.join("package.json"), r#"{}"#).unwrap();
        std::fs::write(pkg_dir.join("src/index.ts"), "export const x = 1;").unwrap();

        let packages = discover_ts_packages(dir.path()).unwrap();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "my-project");
    }

    #[test]
    fn ts_discovery_returns_empty_when_no_package_json() {
        let dir = TempDir::new().unwrap();
        let packages = discover_ts_packages(dir.path()).unwrap();
        assert!(packages.is_empty());
    }
```

**Step 3: Add stub discover_ts_packages function**

Add to `discovery.rs` after the `walk_rs_files` function:

```rust
/// Discover TypeScript/JavaScript packages in a project tree.
///
/// Walks the directory tree looking for `package.json` files (skipping
/// `node_modules/`, `dist/`, `build/`, `.svt/`, `target/`). For each
/// package found, collects `.ts`, `.tsx`, and `.svelte` source files.
pub fn discover_ts_packages(project_root: &Path) -> Result<Vec<TsPackageInfo>, DiscoveryError> {
    Ok(Vec::new())
}
```

Add `use crate::types::TsPackageInfo;` to the imports.

**Step 4: Run tests to verify they fail**

Run: `cargo test -p svt-analyzer discover_ts`
Expected: 5 tests fail, 1 passes (returns_empty_when_no_package_json).

**Step 5: Implement discover_ts_packages**

Replace the stub:

```rust
pub fn discover_ts_packages(project_root: &Path) -> Result<Vec<TsPackageInfo>, DiscoveryError> {
    let skip_dirs = ["node_modules", "dist", "build", ".svt", "target"];
    let mut packages = Vec::new();

    for entry in WalkDir::new(project_root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            !skip_dirs.contains(&name)
        })
        .filter_map(|e| e.ok())
    {
        if entry.file_name() != "package.json" || !entry.file_type().is_file() {
            continue;
        }

        let pkg_dir = entry.path().parent().unwrap_or(project_root);
        let content = std::fs::read_to_string(entry.path())?;
        let json: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue, // Skip malformed package.json
        };

        let name = json
            .get("name")
            .and_then(|v| v.as_str())
            .map(|n| {
                // Strip npm scope prefix (e.g., @scope/name -> name)
                n.rsplit('/').next().unwrap_or(n).to_string()
            })
            .unwrap_or_else(|| {
                pkg_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            });

        let source_root = if pkg_dir.join("src").is_dir() {
            pkg_dir.join("src")
        } else {
            pkg_dir.to_path_buf()
        };

        let source_files = walk_ts_files(&source_root);

        if !source_files.is_empty() {
            packages.push(TsPackageInfo {
                name,
                root: pkg_dir.to_path_buf(),
                source_root,
                source_files,
            });
        }
    }

    Ok(packages)
}

/// Recursively walk a directory and collect all `.ts`, `.tsx`, and `.svelte` files.
///
/// Skips test files (`*.test.ts`, `*.spec.ts`) and declaration files (`*.d.ts`).
fn walk_ts_files(dir: &Path) -> Vec<PathBuf> {
    if !dir.exists() {
        return Vec::new();
    }
    WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            name != "node_modules" && name != "__tests__"
        })
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            match ext {
                "ts" | "tsx" => {
                    !filename.ends_with(".test.ts")
                        && !filename.ends_with(".spec.ts")
                        && !filename.ends_with(".test.tsx")
                        && !filename.ends_with(".spec.tsx")
                        && !filename.ends_with(".d.ts")
                }
                "svelte" => true,
                _ => false,
            }
        })
        .map(|e| e.into_path())
        .collect()
}
```

Add `serde_json` to the imports at the top of `discovery.rs` — but wait, we need to check if serde_json is already a dependency. If not, we'll need to add it.

Actually, `svt-core` depends on `serde_json` and re-exports it, but the analyzer crate needs its own dependency. Add to `crates/analyzer/Cargo.toml`:

```toml
serde_json = "1"
```

**Step 6: Run tests to verify they pass**

Run: `cargo test -p svt-analyzer discover_ts`
Expected: All 6 tests pass.

**Step 7: Run all tests**

Run: `cargo test -p svt-analyzer`
Expected: All existing tests still pass, plus 6 new discovery tests.

**Step 8: Commit**

```bash
git add crates/analyzer/Cargo.toml crates/analyzer/src/discovery.rs crates/analyzer/src/types.rs Cargo.lock
git commit -m "feat(analyzer): add TypeScript package discovery"
```

---

### Task 4: TypeScript Analyzer — Exported Items

**Files:**
- Create: `crates/analyzer/src/languages/typescript.rs`
- Modify: `crates/analyzer/src/languages/mod.rs`

**Step 1: Write failing tests for exported item extraction**

Create `crates/analyzer/src/languages/typescript.rs`:

```rust
//! TypeScript language analyzer using tree-sitter-typescript.
//!
//! Extracts exported structural elements (classes, functions, interfaces,
//! type aliases) and import relationships from TypeScript source files.
//! Also handles Svelte files by extracting their `<script>` blocks first.

use std::path::Path;

use svt_core::model::{EdgeKind, NodeKind};

use crate::languages::svelte;
use crate::types::{AnalysisItem, AnalysisRelation, AnalysisWarning};

use super::{LanguageAnalyzer, ParseResult};

/// TypeScript source code analyzer using tree-sitter-typescript.
#[derive(Debug)]
pub struct TypeScriptAnalyzer {
    _private: (),
}

impl TypeScriptAnalyzer {
    /// Create a new `TypeScriptAnalyzer`.
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for TypeScriptAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAnalyzer for TypeScriptAnalyzer {
    fn analyze_crate(&self, package_name: &str, files: &[&Path]) -> ParseResult {
        ParseResult::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn parse_ts_source(package_name: &str, source: &str) -> ParseResult {
        let mut file = NamedTempFile::with_suffix(".ts").unwrap();
        write!(file, "{}", source).unwrap();
        let analyzer = TypeScriptAnalyzer::new();
        analyzer.analyze_crate(package_name, &[file.path()])
    }

    #[test]
    fn extracts_exported_function() {
        let result = parse_ts_source(
            "my-app",
            "export function fetchData(): void {}",
        );
        let fns: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "function")
            .collect();
        assert!(
            fns.iter().any(|f| f.qualified_name == "my-app::fetchData"),
            "should extract exported function, got: {:?}",
            fns
        );
    }

    #[test]
    fn extracts_exported_class() {
        let result = parse_ts_source("my-app", "export class UserService {}");
        let classes: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "class")
            .collect();
        assert!(
            classes.iter().any(|c| c.qualified_name == "my-app::UserService"),
            "should extract exported class, got: {:?}",
            classes
        );
    }

    #[test]
    fn extracts_exported_interface() {
        let result = parse_ts_source("my-app", "export interface ApiNode { id: string; }");
        let interfaces: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "interface")
            .collect();
        assert!(
            interfaces.iter().any(|i| i.qualified_name == "my-app::ApiNode"),
            "should extract exported interface, got: {:?}",
            interfaces
        );
    }

    #[test]
    fn extracts_exported_type_alias() {
        let result = parse_ts_source(
            "my-app",
            "export type NodeKind = 'system' | 'service';",
        );
        let types: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "type-alias")
            .collect();
        assert!(
            types.iter().any(|t| t.qualified_name == "my-app::NodeKind"),
            "should extract exported type alias, got: {:?}",
            types
        );
    }

    #[test]
    fn extracts_export_default_function() {
        let result = parse_ts_source(
            "my-app",
            "export default function main() {}",
        );
        let fns: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "function")
            .collect();
        assert!(
            fns.iter().any(|f| f.qualified_name == "my-app::main"),
            "should extract default exported function, got: {:?}",
            fns
        );
    }

    #[test]
    fn ignores_non_exported_declarations() {
        let result = parse_ts_source(
            "my-app",
            r#"
function privateHelper() {}
const localVar = 42;
class InternalClass {}
export function publicFn() {}
"#,
        );
        assert_eq!(
            result.items.len(),
            1,
            "should only extract the exported function, got: {:?}",
            result.items
        );
        assert_eq!(result.items[0].qualified_name, "my-app::publicFn");
    }

    #[test]
    fn empty_file_produces_no_items() {
        let result = parse_ts_source("my-app", "");
        assert!(result.items.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn language_is_typescript() {
        let result = parse_ts_source(
            "my-app",
            "export function hello() {}",
        );
        assert_eq!(result.items[0].language, "typescript");
    }

    #[test]
    fn source_ref_contains_line_number() {
        let result = parse_ts_source(
            "my-app",
            "export function hello() {}",
        );
        assert!(
            result.items[0].source_ref.contains(':'),
            "source_ref should contain file:line, got: {}",
            result.items[0].source_ref
        );
    }

    #[test]
    fn parent_qualified_name_is_package() {
        let result = parse_ts_source(
            "my-app",
            "export function hello() {}",
        );
        assert_eq!(
            result.items[0].parent_qualified_name,
            Some("my-app".to_string()),
        );
    }
}
```

**Step 2: Wire up the module**

In `crates/analyzer/src/languages/mod.rs`, add:

```rust
pub mod typescript;
```

**Step 3: Run tests to verify they fail**

Run: `cargo test -p svt-analyzer typescript -- --nocapture`
Expected: Most tests fail (stub returns empty ParseResult).

**Step 4: Implement the TypeScript analyzer**

Replace the `LanguageAnalyzer` impl in `typescript.rs`:

```rust
impl LanguageAnalyzer for TypeScriptAnalyzer {
    fn analyze_crate(&self, package_name: &str, files: &[&Path]) -> ParseResult {
        let mut result = ParseResult::default();

        let mut parser = tree_sitter::Parser::new();
        if parser
            .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            .is_err()
        {
            result.warnings.push(AnalysisWarning {
                source_ref: String::new(),
                message: "failed to load tree-sitter-typescript grammar".to_string(),
            });
            return result;
        }

        for file in files {
            let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");

            match ext {
                "svelte" => {
                    match std::fs::read_to_string(file) {
                        Ok(source) => {
                            let blocks = svelte::extract_script_blocks(&source);
                            for block in &blocks {
                                parse_typescript_source(
                                    &mut parser,
                                    package_name,
                                    file,
                                    &block.content,
                                    block.line_offset,
                                    &mut result.items,
                                    &mut result.relations,
                                    &mut result.warnings,
                                );
                            }
                        }
                        Err(err) => {
                            result.warnings.push(AnalysisWarning {
                                source_ref: file.display().to_string(),
                                message: format!("failed to read file: {err}"),
                            });
                        }
                    }
                }
                _ => {
                    // .ts or .tsx files
                    match std::fs::read_to_string(file) {
                        Ok(source) => {
                            parse_typescript_source(
                                &mut parser,
                                package_name,
                                file,
                                &source,
                                0,
                                &mut result.items,
                                &mut result.relations,
                                &mut result.warnings,
                            );
                        }
                        Err(err) => {
                            result.warnings.push(AnalysisWarning {
                                source_ref: file.display().to_string(),
                                message: format!("failed to read file: {err}"),
                            });
                        }
                    }
                }
            }
        }

        result
    }
}

/// Parse TypeScript source and extract exported items and import relations.
fn parse_typescript_source(
    parser: &mut tree_sitter::Parser,
    package_name: &str,
    file_path: &Path,
    source: &str,
    line_offset: usize,
    items: &mut Vec<AnalysisItem>,
    relations: &mut Vec<AnalysisRelation>,
    warnings: &mut Vec<AnalysisWarning>,
) {
    let Some(tree) = parser.parse(source, None) else {
        warnings.push(AnalysisWarning {
            source_ref: file_path.display().to_string(),
            message: "tree-sitter failed to parse TypeScript".to_string(),
        });
        return;
    };

    let source_bytes = source.as_bytes();
    let root = tree.root_node();

    // The module context for this file is just the package name
    // (directory-based module context is handled by the orchestrator)
    let module_context = package_name.to_string();

    for i in 0..root.named_child_count() {
        let Some(child) = root.named_child(i) else {
            continue;
        };

        match child.kind() {
            "export_statement" => {
                extract_export(
                    child,
                    source_bytes,
                    file_path,
                    &module_context,
                    line_offset,
                    items,
                );
            }
            "import_statement" => {
                extract_import(
                    child,
                    source_bytes,
                    file_path,
                    &module_context,
                    relations,
                );
            }
            _ => {}
        }
    }
}

/// Extract an exported declaration from an `export_statement` node.
fn extract_export(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &Path,
    module_context: &str,
    line_offset: usize,
    items: &mut Vec<AnalysisItem>,
) {
    // export_statement can contain: function_declaration, class_declaration,
    // interface_declaration, type_alias_declaration, or lexical_declaration.
    for i in 0..node.named_child_count() {
        let Some(child) = node.named_child(i) else {
            continue;
        };

        let (kind, sub_kind) = match child.kind() {
            "function_declaration" => (NodeKind::Unit, "function"),
            "class_declaration" => (NodeKind::Unit, "class"),
            "interface_declaration" => (NodeKind::Unit, "interface"),
            "type_alias_declaration" => (NodeKind::Unit, "type-alias"),
            _ => continue,
        };

        let Some(name) = child
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(source).ok())
        else {
            continue;
        };

        let line = child.start_position().row + 1 + line_offset;
        let source_ref = format!("{}:{line}", file_path.display());

        items.push(AnalysisItem {
            qualified_name: format!("{module_context}::{name}"),
            kind,
            sub_kind: sub_kind.to_string(),
            parent_qualified_name: Some(module_context.to_string()),
            source_ref,
            language: "typescript".to_string(),
        });
    }
}

/// Extract an import statement and emit a Depends relation.
fn extract_import(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    _file_path: &Path,
    module_context: &str,
    relations: &mut Vec<AnalysisRelation>,
) {
    // import_statement has a "source" field which is a string node.
    let Some(source_node) = node.child_by_field_name("source") else {
        return;
    };

    let Ok(import_path) = source_node.utf8_text(source) else {
        return;
    };

    // Strip quotes from the import path
    let import_path = import_path.trim_matches(|c| c == '\'' || c == '"');

    if import_path.is_empty() {
        return;
    }

    // Only track relative imports (starting with ./ or ../)
    // External package imports are not resolvable within our graph
    if import_path.starts_with("./") || import_path.starts_with("../") {
        // Convert the relative path to a qualified-name-like form.
        // ./lib/api -> module context determined by orchestrator
        // For now, emit raw import path as target — the orchestrator
        // will resolve it when it builds the full qualified name map.
        relations.push(AnalysisRelation {
            source_qualified_name: module_context.to_string(),
            target_qualified_name: import_path.to_string(),
            kind: EdgeKind::Depends,
        });
    }
}
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p svt-analyzer typescript`
Expected: All 10 tests pass.

**Step 6: Commit**

```bash
git add crates/analyzer/src/languages/typescript.rs crates/analyzer/src/languages/mod.rs
git commit -m "feat(analyzer): add TypeScript analyzer with export extraction"
```

---

### Task 5: TypeScript Analyzer — Import Edges and Svelte Components

**Files:**
- Modify: `crates/analyzer/src/languages/typescript.rs`

**Step 1: Write failing tests for import extraction and Svelte handling**

Add these tests to the existing `mod tests` in `typescript.rs`:

```rust
    #[test]
    fn extracts_relative_import_as_depends_edge() {
        let result = parse_ts_source(
            "my-app",
            "import { fetchData } from './lib/api';",
        );
        let depends: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        assert!(
            !depends.is_empty(),
            "relative import should generate Depends relation"
        );
        assert_eq!(depends[0].source_qualified_name, "my-app");
        assert_eq!(depends[0].target_qualified_name, "./lib/api");
    }

    #[test]
    fn ignores_external_package_imports() {
        let result = parse_ts_source(
            "my-app",
            "import { writable } from 'svelte/store';",
        );
        let depends: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        assert!(
            depends.is_empty(),
            "external package imports should not generate edges"
        );
    }

    #[test]
    fn extracts_from_svelte_script_block() {
        let mut file = NamedTempFile::with_suffix(".svelte").unwrap();
        write!(
            file,
            r#"<script lang="ts">
export function handleClick(): void {{}}
</script>

<button on:click={{handleClick}}>Click me</button>"#,
        )
        .unwrap();
        let analyzer = TypeScriptAnalyzer::new();
        let result = analyzer.analyze_crate("my-app", &[file.path()]);
        let fns: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "function")
            .collect();
        assert!(
            fns.iter().any(|f| f.qualified_name == "my-app::handleClick"),
            "should extract function from Svelte script block, got: {:?}",
            fns
        );
    }

    #[test]
    fn svelte_source_ref_includes_line_offset() {
        let mut file = NamedTempFile::with_suffix(".svelte").unwrap();
        write!(
            file,
            "<p>hello</p>\n<script lang=\"ts\">\nexport function test(): void {{}}\n</script>",
        )
        .unwrap();
        let analyzer = TypeScriptAnalyzer::new();
        let result = analyzer.analyze_crate("my-app", &[file.path()]);
        // The function is on line 3 of the .svelte file (1-indexed)
        // line_offset=1 (script on line 1 zero-indexed) + row 1 within script + 1 = line 3
        let item = &result.items[0];
        let line_num: usize = item
            .source_ref
            .rsplit(':')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            line_num >= 3,
            "source_ref line should account for offset, got: {}",
            item.source_ref
        );
    }

    #[test]
    fn multiple_imports_generate_multiple_edges() {
        let result = parse_ts_source(
            "my-app",
            r#"
import { type ApiNode } from './lib/types';
import { fetchNodes } from './lib/api';
"#,
        );
        let depends: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        assert_eq!(depends.len(), 2, "should generate 2 Depends relations");
    }
```

**Step 2: Run tests to verify they pass (or fail)**

Run: `cargo test -p svt-analyzer typescript`
Expected: All tests pass (import extraction and Svelte handling were implemented in Task 4). If any fail, fix them.

**Step 3: Commit (if any fixes were needed)**

```bash
git add crates/analyzer/src/languages/typescript.rs
git commit -m "test(analyzer): add import and Svelte extraction tests for TypeScript analyzer"
```

---

### Task 6: Integrate TypeScript Analysis into Orchestrator

**Files:**
- Modify: `crates/analyzer/src/lib.rs`
- Modify: `crates/analyzer/src/types.rs`

**Step 1: Update AnalysisSummary in types.rs**

Add a `ts_packages_analyzed` field to `AnalysisSummary`:

```rust
pub struct AnalysisSummary {
    /// Version number of the created analysis snapshot.
    pub version: svt_core::model::Version,
    /// Number of Rust crates analyzed.
    pub crates_analyzed: usize,
    /// Number of TypeScript packages analyzed.
    pub ts_packages_analyzed: usize,
    /// Number of source files parsed.
    pub files_analyzed: usize,
    /// Number of nodes created in the store.
    pub nodes_created: usize,
    /// Number of edges created in the store.
    pub edges_created: usize,
    /// Warnings produced during analysis.
    pub warnings: Vec<AnalysisWarning>,
}
```

**Step 2: Fix existing code that constructs AnalysisSummary**

In `lib.rs`, update the `AnalysisSummary` construction to include `ts_packages_analyzed: 0`.

**Step 3: Add TypeScript phase to analyze_project**

In `lib.rs`, add the TypeScript analysis phase after the Rust phase (after the `for crate_info in &layout.crates` loop, before `map_to_graph`):

Add to imports at top of `lib.rs`:

```rust
use crate::discovery::discover_ts_packages;
use crate::languages::typescript::TypeScriptAnalyzer;
```

Add after the Rust analysis loop:

```rust
    // Phase 2: TypeScript/Svelte analysis
    let ts_packages = discover_ts_packages(project_root).unwrap_or_default();
    let ts_analyzer = TypeScriptAnalyzer::new();
    let mut ts_packages_analyzed = 0;

    for package in &ts_packages {
        // Emit package-level item
        all_items.push(AnalysisItem {
            qualified_name: package.name.clone(),
            kind: NodeKind::Service,
            sub_kind: "package".to_string(),
            parent_qualified_name: None,
            source_ref: package.root.join("package.json").display().to_string(),
            language: "typescript".to_string(),
        });

        // Emit directory-level module items and file-level items
        emit_ts_module_items(&package.source_root, &package.name, &package.source_files, &mut all_items);

        let file_refs: Vec<&Path> = package
            .source_files
            .iter()
            .map(|p| p.as_path())
            .collect();
        files_analyzed += file_refs.len();

        let parse_result = ts_analyzer.analyze_crate(&package.name, &file_refs);

        // Reparent items: the TS analyzer sets parent to package_name,
        // but we need to set it to the file's module qualified name
        for mut item in parse_result.items {
            let file_module_qn = file_to_module_qn(
                &package.source_root,
                // Find which file this item came from via source_ref
                &item.source_ref,
                &package.name,
                &package.source_files,
            );
            if let Some(ref module_qn) = file_module_qn {
                item.parent_qualified_name = Some(module_qn.clone());
                // Update qualified name to include module path
                let item_name = item.qualified_name.rsplit("::").next().unwrap_or("").to_string();
                item.qualified_name = format!("{module_qn}::{item_name}");
            }
            all_items.push(item);
        }

        // Resolve relative import paths to qualified names
        for mut rel in parse_result.relations {
            if rel.target_qualified_name.starts_with("./")
                || rel.target_qualified_name.starts_with("../")
            {
                if let Some(resolved) = resolve_ts_import(
                    &package.source_root,
                    &rel.source_qualified_name,
                    &rel.target_qualified_name,
                    &package.name,
                ) {
                    rel.target_qualified_name = resolved;
                    all_relations.push(rel);
                }
            } else {
                all_relations.push(rel);
            }
        }

        all_warnings.extend(parse_result.warnings);
        ts_packages_analyzed += 1;
    }
```

Add these helper functions before the `analyze_project` function:

```rust
/// Emit module items for directories and files in a TypeScript package.
fn emit_ts_module_items(
    source_root: &Path,
    package_name: &str,
    source_files: &[PathBuf],
    items: &mut Vec<AnalysisItem>,
) {
    use std::collections::HashSet;

    let mut emitted_modules: HashSet<String> = HashSet::new();

    for file in source_files {
        let rel = match file.strip_prefix(source_root) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Emit directory modules
        let mut current_qn = package_name.to_string();
        for component in rel.parent().iter().flat_map(|p| p.components()) {
            if let std::path::Component::Normal(name) = component {
                let name_str = name.to_str().unwrap_or("");
                current_qn = format!("{current_qn}::{name_str}");
                if emitted_modules.insert(current_qn.clone()) {
                    items.push(AnalysisItem {
                        qualified_name: current_qn.clone(),
                        kind: NodeKind::Component,
                        sub_kind: "module".to_string(),
                        parent_qualified_name: Some(
                            current_qn.rsplit_once("::").map(|(p, _)| p.to_string()).unwrap_or_else(|| package_name.to_string())
                        ),
                        source_ref: file.parent().unwrap_or(source_root).display().to_string(),
                        language: "typescript".to_string(),
                    });
                }
            }
        }

        // Emit file-level module (for .ts files) or component (for .svelte files)
        let file_stem = rel.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let ext = rel.extension().and_then(|e| e.to_str()).unwrap_or("");

        // Skip index files as modules (they represent their parent directory)
        if file_stem == "index" || file_stem == "main" {
            continue;
        }

        let file_qn = if current_qn == package_name {
            format!("{package_name}::{file_stem}")
        } else {
            let dir_qn = rel.parent()
                .and_then(|p| p.to_str())
                .map(|s| s.replace(std::path::MAIN_SEPARATOR, "::"))
                .map(|s| format!("{package_name}::{s}"))
                .unwrap_or_else(|| package_name.to_string());
            format!("{dir_qn}::{file_stem}")
        };

        let (kind, sub_kind) = if ext == "svelte" {
            (NodeKind::Unit, "component")
        } else {
            (NodeKind::Component, "module")
        };

        if emitted_modules.insert(file_qn.clone()) {
            items.push(AnalysisItem {
                qualified_name: file_qn,
                kind,
                sub_kind: sub_kind.to_string(),
                parent_qualified_name: Some(current_qn),
                source_ref: file.display().to_string(),
                language: if ext == "svelte" { "svelte" } else { "typescript" }.to_string(),
            });
        }
    }
}

/// Map a file path to its module qualified name.
fn file_to_module_qn(
    source_root: &Path,
    source_ref: &str,
    package_name: &str,
    source_files: &[PathBuf],
) -> Option<String> {
    // source_ref is "path:line" — extract the path part
    let file_path_str = source_ref.rsplit_once(':').map(|(p, _)| p).unwrap_or(source_ref);
    let file_path = Path::new(file_path_str);

    // Find the matching source file
    let matched_file = source_files.iter().find(|f| f.as_path() == file_path)?;
    let rel = matched_file.strip_prefix(source_root).ok()?;

    let stem = rel.file_stem().and_then(|s| s.to_str())?;
    let parent_components: Vec<&str> = rel
        .parent()
        .iter()
        .flat_map(|p| p.components())
        .filter_map(|c| {
            if let std::path::Component::Normal(name) = c {
                name.to_str()
            } else {
                None
            }
        })
        .collect();

    let mut qn = package_name.to_string();
    for comp in &parent_components {
        qn = format!("{qn}::{comp}");
    }

    if stem != "index" && stem != "main" {
        qn = format!("{qn}::{stem}");
    }

    Some(qn)
}

/// Resolve a relative TypeScript import path to a qualified name.
fn resolve_ts_import(
    _source_root: &Path,
    source_qn: &str,
    import_path: &str,
    package_name: &str,
) -> Option<String> {
    // Strip ./ or ../ prefix and file extension
    let clean = import_path
        .trim_start_matches("./")
        .trim_start_matches("../")
        .trim_end_matches(".ts")
        .trim_end_matches(".tsx")
        .trim_end_matches(".svelte")
        .trim_end_matches(".js");

    if clean.is_empty() {
        return None;
    }

    // Convert path separators to :: separators
    let target_qn = format!("{package_name}::{}", clean.replace('/', "::"));

    Some(target_qn)
}
```

**Step 4: Update the AnalysisSummary construction**

```rust
    Ok(AnalysisSummary {
        version,
        crates_analyzed: layout.crates.len(),
        ts_packages_analyzed,
        files_analyzed,
        nodes_created: nodes.len(),
        edges_created: edges.len(),
        warnings: all_warnings,
    })
```

**Step 5: Run all tests**

Run: `cargo test`
Expected: All tests pass. Existing dog-food tests should still work (they now also discover the web/ package).

**Step 6: Commit**

```bash
git add crates/analyzer/src/lib.rs crates/analyzer/src/types.rs
git commit -m "feat(analyzer): integrate TypeScript analysis into orchestrator"
```

---

### Task 7: Update CLI Output for TypeScript

**Files:**
- Modify: `crates/cli/src/main.rs`

**Step 1: Update run_analyze to show TypeScript stats**

In the `run_analyze` function, update the output to include TypeScript package count:

```rust
    println!("Analyzed {}\\n", args.path.display());
    println!("  Created analysis snapshot v{}", summary.version);
    println!(
        "    {} crates, {} TypeScript packages, {} files analyzed",
        summary.crates_analyzed, summary.ts_packages_analyzed, summary.files_analyzed
    );
    println!(
        "    {} nodes, {} edges",
        summary.nodes_created, summary.edges_created
    );
```

**Step 2: Run tests**

Run: `cargo test -p svt-cli`
Expected: All CLI tests pass.

**Step 3: Commit**

```bash
git add crates/cli/src/main.rs
git commit -m "feat(cli): show TypeScript package count in analyze output"
```

---

### Task 8: Dog-Food Tests

**Files:**
- Modify: `crates/analyzer/tests/dogfood.rs`

**Step 1: Read existing dogfood tests**

Read `crates/analyzer/tests/dogfood.rs` to understand the current structure.

**Step 2: Add TypeScript-specific assertions**

Add a new test:

```rust
#[test]
fn dogfood_analysis_includes_typescript_nodes() {
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let mut store = CozoStore::new_in_memory().unwrap();
    let summary = svt_analyzer::analyze_project(&mut store, &project_root, None).unwrap();

    assert!(
        summary.ts_packages_analyzed >= 1,
        "should find at least 1 TypeScript package (web/), got {}",
        summary.ts_packages_analyzed
    );

    // Check that TypeScript nodes exist in the store
    let nodes = store
        .get_all_nodes(summary.version)
        .unwrap();

    let ts_nodes: Vec<_> = nodes
        .iter()
        .filter(|n| n.language.as_deref() == Some("typescript") || n.language.as_deref() == Some("svelte"))
        .collect();

    assert!(
        !ts_nodes.is_empty(),
        "should have TypeScript/Svelte nodes in the analysis snapshot"
    );

    // Should have the web package
    let web_package = nodes
        .iter()
        .find(|n| n.canonical_path.starts_with("/software-visualizer-web"));
    assert!(
        web_package.is_some(),
        "should find web package node, got paths: {:?}",
        ts_nodes.iter().map(|n| &n.canonical_path).take(10).collect::<Vec<_>>()
    );
}
```

Also update the existing `dogfood_analyze_produces_meaningful_results` test to account for TypeScript packages:

Update the assertion `summary.crates_analyzed >= 4` — this stays the same since `crates_analyzed` only counts Rust crates.

**Step 3: Run dogfood tests**

Run: `cargo test -p svt-analyzer --test dogfood`
Expected: All tests pass.

**Step 4: Commit**

```bash
git add crates/analyzer/tests/dogfood.rs
git commit -m "test(analyzer): add dog-food test for TypeScript analysis"
```

---

### Task 9: Integration Test with Fixture

**Files:**
- Create: `crates/analyzer/tests/fixtures/ts-project/package.json`
- Create: `crates/analyzer/tests/fixtures/ts-project/src/index.ts`
- Create: `crates/analyzer/tests/fixtures/ts-project/src/lib/utils.ts`
- Create: `crates/analyzer/tests/fixtures/ts-project/src/components/App.svelte`
- Create: `crates/analyzer/tests/integration.rs` (add test to existing file)

**Step 1: Create fixture files**

`crates/analyzer/tests/fixtures/ts-project/package.json`:
```json
{
  "name": "test-project",
  "version": "1.0.0"
}
```

`crates/analyzer/tests/fixtures/ts-project/src/index.ts`:
```typescript
import { formatName } from './lib/utils';

export function main(): void {
    console.log(formatName("world"));
}
```

`crates/analyzer/tests/fixtures/ts-project/src/lib/utils.ts`:
```typescript
export function formatName(name: string): string {
    return `Hello, ${name}!`;
}

export interface Config {
    name: string;
    debug: boolean;
}
```

`crates/analyzer/tests/fixtures/ts-project/src/components/App.svelte`:
```svelte
<script lang="ts">
export let title: string = "App";
export function greet(): string {
    return `Hello from ${title}`;
}
</script>

<h1>{title}</h1>
```

**Step 2: Add integration test**

Add to `crates/analyzer/tests/integration.rs`:

```rust
#[test]
fn typescript_fixture_project_produces_nodes_and_edges() {
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ts-project");

    let packages = svt_analyzer::discovery::discover_ts_packages(&fixture_root).unwrap();
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].name, "test-project");
    assert!(
        packages[0].source_files.len() >= 3,
        "should find index.ts, utils.ts, and App.svelte, got {}",
        packages[0].source_files.len()
    );

    // Full pipeline test
    let mut store = CozoStore::new_in_memory().unwrap();
    let summary = svt_analyzer::analyze_project(&mut store, &fixture_root, None).unwrap();

    assert!(summary.ts_packages_analyzed >= 1);
    assert!(summary.nodes_created > 0);

    let nodes = store.get_all_nodes(summary.version).unwrap();
    let ts_nodes: Vec<_> = nodes
        .iter()
        .filter(|n| {
            n.language.as_deref() == Some("typescript")
                || n.language.as_deref() == Some("svelte")
        })
        .collect();
    assert!(
        !ts_nodes.is_empty(),
        "should have TypeScript nodes"
    );

    // Check specific expected nodes
    let paths: Vec<&str> = ts_nodes.iter().map(|n| n.canonical_path.as_str()).collect();
    assert!(
        paths.iter().any(|p| p.contains("test-project")),
        "should have test-project package node, got: {:?}",
        paths
    );
}
```

**Step 3: Run integration tests**

Run: `cargo test -p svt-analyzer --test integration`
Expected: All tests pass.

**Step 4: Commit**

```bash
git add crates/analyzer/tests/fixtures/ crates/analyzer/tests/integration.rs
git commit -m "test(analyzer): add TypeScript fixture and integration test"
```

---

### Task 10: Update PROGRESS.md

**Files:**
- Modify: `docs/plan/PROGRESS.md`

**Step 1: Update the progress document**

- Add Milestone 7 row to the completed milestones table
- Update the "Current state" test count
- Remove TypeScript analyzer from "Not Yet Built"
- Update "What's Working Now" section

**Step 2: Run full test suite to get final count**

Run: `cargo test 2>&1 | grep "test result" | tail -1`
Expected: Note the total test count.

**Step 3: Commit**

```bash
git add docs/plan/PROGRESS.md
git commit -m "docs: mark milestone 7 as complete with progress summary"
```

---

### Task 11: Final Verification

**Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

**Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings.

**Step 3: Run fmt check**

Run: `cargo fmt --check`
Expected: No formatting issues.

**Step 4: Dog-food the analyzer on the whole project**

Run: `cargo run -p svt-cli -- --store /tmp/svt-test-store analyze .`
Expected: Output shows both Rust crates and TypeScript packages analyzed, with node/edge counts.

**Step 5: Verify the web package appears**

Run: `cargo run -p svt-cli -- --store /tmp/svt-test-store export --format json | grep -c "typescript"`
Expected: Multiple TypeScript nodes in the export.
