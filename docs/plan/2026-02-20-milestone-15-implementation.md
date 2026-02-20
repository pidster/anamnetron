# Milestone 15: Additional Language Analyzers (Go + Python) — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Go and Python language analyzers through the existing `LanguageAnalyzer` trait and `AnalyzerRegistry`, with project discovery for each language.

**Architecture:** Each analyzer follows the established pattern: a struct implementing `LanguageAnalyzer` with `language_id()` and `analyze_crate()`. Discovery functions scan for `go.mod` / `pyproject.toml` / `setup.py` and collect source files. The `analyze_project()` orchestrator in `lib.rs` gains two new phases (Go + Python). Both analyzers register in `AnalyzerRegistry::with_defaults()`.

**Tech Stack:** tree-sitter 0.24, tree-sitter-go 0.23, tree-sitter-python 0.23, walkdir 2, serde_json 1

---

## Context

**Existing pattern (Rust analyzer):**
1. `discovery.rs` — `discover_project()` runs `cargo metadata`, returns `ProjectLayout` with `Vec<CrateInfo>`
2. `languages/rust.rs` — `RustAnalyzer` implements `LanguageAnalyzer`, creates `tree_sitter::Parser`, sets language to `tree_sitter_rust::LANGUAGE.into()`, walks AST extracting `AnalysisItem` + `AnalysisRelation` + `AnalysisWarning`
3. `lib.rs` — `analyze_project()` calls discovery, loops over crates, calls `rust_analyzer.analyze_crate()`, collects items/relations/warnings, then maps to graph and inserts into store

**Existing pattern (TypeScript analyzer):**
1. `discovery.rs` — `discover_ts_packages()` walks for `package.json`, returns `Vec<TsPackageInfo>`
2. `languages/typescript.rs` — `TypeScriptAnalyzer` implements `LanguageAnalyzer`, uses `tree_sitter_typescript::LANGUAGE_TYPESCRIPT`
3. `lib.rs` — Phase 2 calls `discover_ts_packages()`, loops over packages, calls `ts_analyzer.analyze_crate()`

**Key types:**
- `AnalysisItem { qualified_name, kind: NodeKind, sub_kind, parent_qualified_name, source_ref, language }`
- `AnalysisRelation { source_qualified_name, target_qualified_name, kind: EdgeKind }`
- `ParseResult { items, relations, warnings }`

**Mapping:** `mapping.rs` converts `qualified_name` (e.g., `mypackage::MyFunc`) to canonical path (e.g., `/mypackage/my-func`) using `to_kebab_case` per segment.

**Grammar APIs:**
- `tree_sitter_go::LANGUAGE` — Go grammar constant
- `tree_sitter_python::LANGUAGE` — Python grammar constant
- Both use the same `LanguageFn` pattern as Rust/TypeScript

---

### Task 1: Add tree-sitter-go and tree-sitter-python Dependencies

**Files:**
- Modify: `crates/analyzer/Cargo.toml`

**Step 1: Add dependencies**

Add to `[dependencies]`:
```toml
tree-sitter-go = "0.23"
tree-sitter-python = "0.23"
```

**Step 2: Verify compilation**

Run: `cargo check -p svt-analyzer`
Expected: compiles successfully with new deps

**Step 3: Commit**

```bash
git add crates/analyzer/Cargo.toml Cargo.lock
git commit -m "feat(analyzer): add tree-sitter-go and tree-sitter-python dependencies"
```

---

### Task 2: Go Discovery — `GoPackageInfo` Type + `discover_go_packages()`

**Files:**
- Modify: `crates/analyzer/src/types.rs` — add `GoPackageInfo`
- Modify: `crates/analyzer/src/discovery.rs` — add `discover_go_packages()`, `walk_go_files()`

**Step 1: Write `GoPackageInfo` type**

Add to `types.rs`:
```rust
/// Information about a Go module/package discovered in the project.
#[derive(Debug, Clone)]
pub struct GoPackageInfo {
    /// Module path from go.mod (e.g., "github.com/user/repo").
    pub module_path: String,
    /// Short name derived from the module path (last segment).
    pub name: String,
    /// Root directory of the module (where go.mod lives).
    pub root: PathBuf,
    /// All .go source files (excluding _test.go and vendor/).
    pub source_files: Vec<PathBuf>,
    /// Go package directories discovered (relative to root).
    pub packages: Vec<GoPackage>,
}

/// A single Go package (directory with .go files).
#[derive(Debug, Clone)]
pub struct GoPackage {
    /// Package import path relative to module (e.g., "cmd/server").
    pub import_path: String,
    /// Package name from the `package` declaration.
    pub dir: PathBuf,
    /// .go source files in this package directory.
    pub source_files: Vec<PathBuf>,
}
```

**Step 2: Write failing test for `discover_go_packages()`**

Add to `discovery.rs` tests:
```rust
#[test]
fn discovers_go_module_from_go_mod() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("go.mod"),
        "module github.com/user/myapp\n\ngo 1.21\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("cmd/server")).unwrap();
    std::fs::write(dir.path().join("main.go"), "package main\n\nfunc main() {}\n").unwrap();
    std::fs::write(
        dir.path().join("cmd/server/server.go"),
        "package server\n\nfunc Run() {}\n",
    )
    .unwrap();

    let packages = discover_go_packages(dir.path()).unwrap();
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].module_path, "github.com/user/myapp");
    assert_eq!(packages[0].name, "myapp");
    assert!(!packages[0].source_files.is_empty());
}

#[test]
fn go_discovery_skips_vendor_and_test_files() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("go.mod"), "module example.com/app\n\ngo 1.21\n").unwrap();
    std::fs::write(dir.path().join("main.go"), "package main\n").unwrap();
    std::fs::write(dir.path().join("main_test.go"), "package main\n").unwrap();
    std::fs::create_dir_all(dir.path().join("vendor/dep")).unwrap();
    std::fs::write(dir.path().join("vendor/dep/lib.go"), "package dep\n").unwrap();

    let packages = discover_go_packages(dir.path()).unwrap();
    assert_eq!(packages.len(), 1);
    // Should NOT include _test.go or vendor/ files
    for f in &packages[0].source_files {
        let name = f.file_name().unwrap().to_str().unwrap();
        assert!(!name.ends_with("_test.go"), "should skip test files");
        assert!(
            !f.to_str().unwrap().contains("vendor"),
            "should skip vendor dir"
        );
    }
}

#[test]
fn go_discovery_returns_empty_when_no_go_mod() {
    let dir = TempDir::new().unwrap();
    let packages = discover_go_packages(dir.path()).unwrap();
    assert!(packages.is_empty());
}
```

**Step 3: Run tests to verify they fail**

Run: `cargo test -p svt-analyzer discover_go -- --no-run 2>&1`
Expected: compilation error (function doesn't exist yet)

**Step 4: Implement `discover_go_packages()` and `walk_go_files()`**

Add to `discovery.rs`:
```rust
/// Discover Go modules in a project tree.
///
/// Walks the directory tree looking for `go.mod` files (skipping `vendor/`,
/// `node_modules/`, `target/`). For each module, collects `.go` source files
/// (excluding `_test.go` files and `vendor/` directories).
pub fn discover_go_packages(project_root: &Path) -> Result<Vec<GoPackageInfo>, DiscoveryError> {
    let skip_dirs = ["vendor", "node_modules", "target", ".git", "dist"];
    let mut packages = Vec::new();

    for entry in WalkDir::new(project_root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            !skip_dirs.contains(&name)
        })
        .filter_map(|e| e.ok())
    {
        if entry.file_name() != "go.mod" || !entry.file_type().is_file() {
            continue;
        }

        let mod_dir = entry.path().parent().unwrap_or(project_root);
        let content = std::fs::read_to_string(entry.path())?;

        // Parse module path from first "module" line
        let module_path = content
            .lines()
            .find(|line| line.starts_with("module "))
            .and_then(|line| line.strip_prefix("module "))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        if module_path.is_empty() {
            continue;
        }

        let name = module_path
            .rsplit('/')
            .next()
            .unwrap_or(&module_path)
            .to_string();

        let source_files = walk_go_files(mod_dir);

        // Group files by package directory
        let mut pkg_dirs: std::collections::HashMap<PathBuf, Vec<PathBuf>> =
            std::collections::HashMap::new();
        for file in &source_files {
            let dir = file.parent().unwrap_or(mod_dir).to_path_buf();
            pkg_dirs.entry(dir).or_default().push(file.clone());
        }

        let go_packages: Vec<GoPackage> = pkg_dirs
            .into_iter()
            .map(|(dir, files)| {
                let import_path = dir
                    .strip_prefix(mod_dir)
                    .unwrap_or(Path::new(""))
                    .to_str()
                    .unwrap_or("")
                    .to_string();
                GoPackage {
                    import_path,
                    dir,
                    source_files: files,
                }
            })
            .collect();

        if !source_files.is_empty() {
            packages.push(GoPackageInfo {
                module_path,
                name,
                root: mod_dir.to_path_buf(),
                source_files,
                packages: go_packages,
            });
        }
    }

    Ok(packages)
}

/// Recursively walk a directory and collect all `.go` files.
///
/// Skips `_test.go` files and `vendor/` directories.
fn walk_go_files(dir: &Path) -> Vec<PathBuf> {
    if !dir.exists() {
        return Vec::new();
    }
    WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            name != "vendor" && name != "node_modules" && name != "testdata"
        })
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            ext == "go" && !filename.ends_with("_test.go")
        })
        .map(|e| e.into_path())
        .collect()
}
```

Add import for `GoPackageInfo` and `GoPackage` in `discovery.rs` use statement.

**Step 5: Run tests**

Run: `cargo test -p svt-analyzer discover_go`
Expected: 3 tests pass

**Step 6: Commit**

```bash
git add crates/analyzer/src/types.rs crates/analyzer/src/discovery.rs
git commit -m "feat(analyzer): add Go project discovery (go.mod parsing, .go file walking)"
```

---

### Task 3: Go Analyzer — `GoAnalyzer` Implementing `LanguageAnalyzer`

**Files:**
- Create: `crates/analyzer/src/languages/go.rs`
- Modify: `crates/analyzer/src/languages/mod.rs` — add `pub mod go;`

**Step 1: Write failing tests**

Create `crates/analyzer/src/languages/go.rs` with tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::path::PathBuf;

    fn parse_go_source(source: &str) -> ParseResult {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.go");
        std::fs::write(&file, source).unwrap();
        let analyzer = GoAnalyzer::new();
        analyzer.analyze_crate("myapp", &[file.as_path()])
    }

    #[test]
    fn extracts_package_level_function() {
        let result = parse_go_source("package main\n\nfunc Hello() {}\n");
        let funcs: Vec<_> = result.items.iter().filter(|i| i.sub_kind == "function").collect();
        assert_eq!(funcs.len(), 1);
        assert!(funcs[0].qualified_name.contains("Hello"));
        assert_eq!(funcs[0].kind, NodeKind::Unit);
        assert_eq!(funcs[0].language, "go");
    }

    #[test]
    fn extracts_struct_type() {
        let result = parse_go_source(
            "package main\n\ntype Server struct {\n\tPort int\n}\n",
        );
        let structs: Vec<_> = result.items.iter().filter(|i| i.sub_kind == "struct").collect();
        assert_eq!(structs.len(), 1);
        assert!(structs[0].qualified_name.contains("Server"));
        assert_eq!(structs[0].kind, NodeKind::Unit);
    }

    #[test]
    fn extracts_interface_type() {
        let result = parse_go_source(
            "package main\n\ntype Handler interface {\n\tHandle() error\n}\n",
        );
        let ifaces: Vec<_> = result.items.iter().filter(|i| i.sub_kind == "interface").collect();
        assert_eq!(ifaces.len(), 1);
        assert!(ifaces[0].qualified_name.contains("Handler"));
    }

    #[test]
    fn extracts_method_declaration() {
        let result = parse_go_source(
            "package main\n\ntype Server struct{}\n\nfunc (s *Server) Start() {}\n",
        );
        let methods: Vec<_> = result.items.iter().filter(|i| i.sub_kind == "method").collect();
        assert_eq!(methods.len(), 1);
        assert!(methods[0].qualified_name.contains("Server") && methods[0].qualified_name.contains("Start"));
    }

    #[test]
    fn extracts_import_relations() {
        let result = parse_go_source(
            "package main\n\nimport \"fmt\"\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n",
        );
        let imports: Vec<_> = result.relations.iter().filter(|r| r.kind == EdgeKind::Depends).collect();
        assert!(!imports.is_empty(), "should have at least one import dependency");
    }

    #[test]
    fn handles_empty_file_gracefully() {
        let result = parse_go_source("");
        assert!(result.items.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn language_id_is_go() {
        let analyzer = GoAnalyzer::new();
        assert_eq!(analyzer.language_id(), "go");
    }
}
```

**Step 2: Implement `GoAnalyzer`**

Write the implementation above the tests in `go.rs`:

```rust
//! Go language analyzer using tree-sitter-go.
//!
//! Extracts structural elements (packages, structs, interfaces, functions,
//! methods) and import relationships from Go source files.

use std::path::Path;

use svt_core::model::{EdgeKind, NodeKind};

use crate::types::{AnalysisItem, AnalysisRelation, AnalysisWarning};

use super::{LanguageAnalyzer, ParseResult};

/// Go source code analyzer using tree-sitter-go.
#[derive(Debug)]
pub struct GoAnalyzer {
    _private: (),
}

impl GoAnalyzer {
    /// Create a new `GoAnalyzer`.
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for GoAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAnalyzer for GoAnalyzer {
    fn language_id(&self) -> &str {
        "go"
    }

    fn analyze_crate(&self, module_name: &str, files: &[&Path]) -> ParseResult {
        let mut result = ParseResult::default();

        let mut parser = tree_sitter::Parser::new();
        if parser
            .set_language(&tree_sitter_go::LANGUAGE.into())
            .is_err()
        {
            result.warnings.push(AnalysisWarning {
                source_ref: String::new(),
                message: "failed to load tree-sitter-go grammar".to_string(),
            });
            return result;
        }

        for file in files {
            match std::fs::read_to_string(file) {
                Ok(source) => {
                    parse_go_file(
                        &mut parser,
                        &source,
                        file,
                        module_name,
                        &mut result,
                    );
                }
                Err(e) => {
                    result.warnings.push(AnalysisWarning {
                        source_ref: file.display().to_string(),
                        message: format!("failed to read file: {e}"),
                    });
                }
            }
        }

        result
    }
}

fn parse_go_file(
    parser: &mut tree_sitter::Parser,
    source: &str,
    file: &Path,
    module_name: &str,
    result: &mut ParseResult,
) {
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => {
            result.warnings.push(AnalysisWarning {
                source_ref: file.display().to_string(),
                message: "tree-sitter parse returned None".to_string(),
            });
            return;
        }
    };

    let root = tree.root_node();
    let source_ref_base = file.display().to_string();

    // Detect package name from package_clause
    let _package_name = root
        .children(&mut root.walk())
        .find(|n| n.kind() == "package_clause")
        .and_then(|n| n.child_by_field_name("name"))
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .unwrap_or("main");

    for child in root.children(&mut root.walk()) {
        match child.kind() {
            "function_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                        let line = child.start_position().row + 1;
                        result.items.push(AnalysisItem {
                            qualified_name: format!("{module_name}::{name}"),
                            kind: NodeKind::Unit,
                            sub_kind: "function".to_string(),
                            parent_qualified_name: Some(module_name.to_string()),
                            source_ref: format!("{source_ref_base}:{line}"),
                            language: "go".to_string(),
                        });
                    }
                }
            }
            "method_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                        let receiver_type = extract_receiver_type(&child, source);
                        let line = child.start_position().row + 1;
                        let qn = if let Some(ref recv) = receiver_type {
                            format!("{module_name}::{recv}::{name}")
                        } else {
                            format!("{module_name}::{name}")
                        };
                        result.items.push(AnalysisItem {
                            qualified_name: qn,
                            kind: NodeKind::Unit,
                            sub_kind: "method".to_string(),
                            parent_qualified_name: receiver_type
                                .map(|r| format!("{module_name}::{r}"))
                                .or_else(|| Some(module_name.to_string())),
                            source_ref: format!("{source_ref_base}:{line}"),
                            language: "go".to_string(),
                        });
                    }
                }
            }
            "type_declaration" => {
                // type_declaration contains type_spec children
                for spec in child.children(&mut child.walk()) {
                    if spec.kind() == "type_spec" {
                        extract_type_spec(&spec, source, module_name, &source_ref_base, result);
                    }
                }
            }
            "import_declaration" => {
                extract_imports(&child, source, module_name, result);
            }
            _ => {}
        }
    }
}

fn extract_type_spec(
    spec: &tree_sitter::Node,
    source: &str,
    module_name: &str,
    source_ref_base: &str,
    result: &mut ParseResult,
) {
    let name = match spec.child_by_field_name("name") {
        Some(n) => match n.utf8_text(source.as_bytes()) {
            Ok(s) => s.to_string(),
            Err(_) => return,
        },
        None => return,
    };

    let type_node = spec.child_by_field_name("type");
    let sub_kind = match type_node.map(|n| n.kind()) {
        Some("struct_type") => "struct",
        Some("interface_type") => "interface",
        _ => "type_alias",
    };

    let line = spec.start_position().row + 1;
    result.items.push(AnalysisItem {
        qualified_name: format!("{module_name}::{name}"),
        kind: NodeKind::Unit,
        sub_kind: sub_kind.to_string(),
        parent_qualified_name: Some(module_name.to_string()),
        source_ref: format!("{source_ref_base}:{line}"),
        language: "go".to_string(),
    });
}

fn extract_receiver_type(method: &tree_sitter::Node, source: &str) -> Option<String> {
    let params = method.child_by_field_name("receiver")?;
    // receiver is a parameter_list; find the type inside
    let mut cursor = params.walk();
    for child in params.children(&mut cursor) {
        if child.kind() == "parameter_declaration" {
            if let Some(type_node) = child.child_by_field_name("type") {
                // Handle pointer receivers (*Type)
                let text = type_node.utf8_text(source.as_bytes()).ok()?;
                return Some(text.trim_start_matches('*').to_string());
            }
        }
    }
    None
}

fn extract_imports(
    import_decl: &tree_sitter::Node,
    source: &str,
    module_name: &str,
    result: &mut ParseResult,
) {
    let mut cursor = import_decl.walk();
    for child in import_decl.children(&mut cursor) {
        match child.kind() {
            "import_spec" => {
                if let Some(path_node) = child.child_by_field_name("path") {
                    if let Ok(path) = path_node.utf8_text(source.as_bytes()) {
                        let import_path = path.trim_matches('"');
                        result.relations.push(AnalysisRelation {
                            source_qualified_name: module_name.to_string(),
                            target_qualified_name: import_path.to_string(),
                            kind: EdgeKind::Depends,
                        });
                    }
                }
            }
            "import_spec_list" => {
                let mut inner_cursor = child.walk();
                for spec in child.children(&mut inner_cursor) {
                    if spec.kind() == "import_spec" {
                        if let Some(path_node) = spec.child_by_field_name("path") {
                            if let Ok(path) = path_node.utf8_text(source.as_bytes()) {
                                let import_path = path.trim_matches('"');
                                result.relations.push(AnalysisRelation {
                                    source_qualified_name: module_name.to_string(),
                                    target_qualified_name: import_path.to_string(),
                                    kind: EdgeKind::Depends,
                                });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
```

**Step 3: Add `pub mod go;` to `languages/mod.rs`**

**Step 4: Run tests**

Run: `cargo test -p svt-analyzer go`
Expected: all 7 Go analyzer tests pass

**Step 5: Commit**

```bash
git add crates/analyzer/src/languages/go.rs crates/analyzer/src/languages/mod.rs
git commit -m "feat(analyzer): add Go language analyzer (struct/interface/function/method extraction)"
```

---

### Task 4: Python Discovery — `PythonPackageInfo` Type + `discover_python_packages()`

**Files:**
- Modify: `crates/analyzer/src/types.rs` — add `PythonPackageInfo`
- Modify: `crates/analyzer/src/discovery.rs` — add `discover_python_packages()`, `walk_py_files()`

**Step 1: Write `PythonPackageInfo` type**

Add to `types.rs`:
```rust
/// Information about a Python package discovered in the project.
#[derive(Debug, Clone)]
pub struct PythonPackageInfo {
    /// Package name (from pyproject.toml name field, setup.py, or directory name).
    pub name: String,
    /// Root directory of the package (where pyproject.toml/setup.py lives).
    pub root: PathBuf,
    /// Source root directory (root/src/<name>/ or root/<name>/ or root/).
    pub source_root: PathBuf,
    /// All .py source files under the source root.
    pub source_files: Vec<PathBuf>,
}
```

**Step 2: Write failing tests**

Add to `discovery.rs` tests:
```rust
#[test]
fn discovers_python_package_from_pyproject_toml() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("pyproject.toml"),
        "[project]\nname = \"my-app\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("src/my_app")).unwrap();
    std::fs::write(
        dir.path().join("src/my_app/__init__.py"),
        "",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/my_app/core.py"),
        "def hello(): pass\n",
    )
    .unwrap();

    let packages = discover_python_packages(dir.path()).unwrap();
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].name, "my-app");
    assert!(!packages[0].source_files.is_empty());
}

#[test]
fn discovers_python_package_from_setup_py() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("setup.py"),
        "from setuptools import setup\nsetup(name='legacy-app')\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("legacy_app")).unwrap();
    std::fs::write(dir.path().join("legacy_app/__init__.py"), "").unwrap();
    std::fs::write(dir.path().join("legacy_app/main.py"), "def run(): pass\n").unwrap();

    let packages = discover_python_packages(dir.path()).unwrap();
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].name, "legacy-app");
}

#[test]
fn python_discovery_skips_venv_and_test_dirs() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("pyproject.toml"), "[project]\nname = \"app\"\n").unwrap();
    std::fs::create_dir_all(dir.path().join("app")).unwrap();
    std::fs::write(dir.path().join("app/__init__.py"), "").unwrap();
    std::fs::write(dir.path().join("app/core.py"), "x = 1\n").unwrap();
    std::fs::create_dir_all(dir.path().join(".venv/lib")).unwrap();
    std::fs::write(dir.path().join(".venv/lib/site.py"), "x = 1\n").unwrap();

    let packages = discover_python_packages(dir.path()).unwrap();
    assert_eq!(packages.len(), 1);
    for f in &packages[0].source_files {
        assert!(
            !f.to_str().unwrap().contains(".venv"),
            "should skip .venv dir"
        );
    }
}

#[test]
fn python_discovery_returns_empty_when_no_manifest() {
    let dir = TempDir::new().unwrap();
    let packages = discover_python_packages(dir.path()).unwrap();
    assert!(packages.is_empty());
}
```

**Step 3: Run tests to verify they fail**

Run: `cargo test -p svt-analyzer discover_python -- --no-run 2>&1`
Expected: compilation error

**Step 4: Implement `discover_python_packages()` and `walk_py_files()`**

Add to `discovery.rs`:
```rust
/// Discover Python packages in a project tree.
///
/// Looks for `pyproject.toml` or `setup.py` files (skipping `.venv/`, `venv/`,
/// `__pycache__/`, `node_modules/`, `target/`). For each package found,
/// collects `.py` source files.
pub fn discover_python_packages(
    project_root: &Path,
) -> Result<Vec<PythonPackageInfo>, DiscoveryError> {
    let skip_dirs = [
        ".venv",
        "venv",
        "__pycache__",
        "node_modules",
        "target",
        ".git",
        ".tox",
        "dist",
        "build",
        ".eggs",
    ];
    let mut packages = Vec::new();
    let mut seen_roots: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

    // Walk for pyproject.toml
    for entry in WalkDir::new(project_root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            !skip_dirs.contains(&name)
        })
        .filter_map(|e| e.ok())
    {
        let is_pyproject = entry.file_name() == "pyproject.toml";
        let is_setup_py = entry.file_name() == "setup.py";

        if (!is_pyproject && !is_setup_py) || !entry.file_type().is_file() {
            continue;
        }

        let pkg_dir = entry.path().parent().unwrap_or(project_root);
        if !seen_roots.insert(pkg_dir.to_path_buf()) {
            continue; // Already processed this directory
        }

        let name = if is_pyproject {
            parse_pyproject_name(entry.path())
        } else {
            parse_setup_py_name(entry.path())
        }
        .unwrap_or_else(|| {
            pkg_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

        // Find source root: prefer src/<pkg_name>/, then <pkg_name>/, then root
        let pkg_name_underscore = name.replace('-', "_");
        let source_root = if pkg_dir.join("src").join(&pkg_name_underscore).is_dir() {
            pkg_dir.join("src").join(&pkg_name_underscore)
        } else if pkg_dir.join(&pkg_name_underscore).is_dir() {
            pkg_dir.join(&pkg_name_underscore)
        } else if pkg_dir.join("src").is_dir() {
            pkg_dir.join("src")
        } else {
            pkg_dir.to_path_buf()
        };

        let source_files = walk_py_files(&source_root);

        if !source_files.is_empty() {
            packages.push(PythonPackageInfo {
                name,
                root: pkg_dir.to_path_buf(),
                source_root,
                source_files,
            });
        }
    }

    Ok(packages)
}

/// Parse package name from `pyproject.toml`.
fn parse_pyproject_name(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    // Simple TOML parsing: look for name = "..." under [project]
    let mut in_project = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_project = trimmed == "[project]";
            continue;
        }
        if in_project && trimmed.starts_with("name") {
            if let Some(value) = trimmed.split('=').nth(1) {
                let name = value.trim().trim_matches('"').trim_matches('\'');
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
    }
    None
}

/// Parse package name from `setup.py`.
fn parse_setup_py_name(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    // Simple heuristic: look for name='...' or name="..."
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name=") || trimmed.starts_with("name =") {
            let value = trimmed
                .split('=')
                .nth(1)?
                .trim()
                .trim_end_matches(',')
                .trim_matches('"')
                .trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Recursively walk a directory and collect all `.py` files.
///
/// Skips `__pycache__/`, `.venv/`, `venv/`, test files.
fn walk_py_files(dir: &Path) -> Vec<PathBuf> {
    if !dir.exists() {
        return Vec::new();
    }
    WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            name != "__pycache__" && name != ".venv" && name != "venv" && name != ".tox"
        })
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            ext == "py"
                && !filename.starts_with("test_")
                && !filename.ends_with("_test.py")
                && filename != "conftest.py"
                && filename != "setup.py"
        })
        .map(|e| e.into_path())
        .collect()
}
```

Add import for `PythonPackageInfo` in `discovery.rs` use statement.

**Step 5: Run tests**

Run: `cargo test -p svt-analyzer discover_python`
Expected: 4 tests pass

**Step 6: Commit**

```bash
git add crates/analyzer/src/types.rs crates/analyzer/src/discovery.rs
git commit -m "feat(analyzer): add Python project discovery (pyproject.toml/setup.py, .py file walking)"
```

---

### Task 5: Python Analyzer — `PythonAnalyzer` Implementing `LanguageAnalyzer`

**Files:**
- Create: `crates/analyzer/src/languages/python.rs`
- Modify: `crates/analyzer/src/languages/mod.rs` — add `pub mod python;`

**Step 1: Write failing tests**

Create `crates/analyzer/src/languages/python.rs` with tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::path::PathBuf;

    fn parse_py_source(source: &str) -> ParseResult {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.py");
        std::fs::write(&file, source).unwrap();
        let analyzer = PythonAnalyzer::new();
        analyzer.analyze_crate("mypackage", &[file.as_path()])
    }

    #[test]
    fn extracts_top_level_function() {
        let result = parse_py_source("def hello():\n    pass\n");
        let funcs: Vec<_> = result.items.iter().filter(|i| i.sub_kind == "function").collect();
        assert_eq!(funcs.len(), 1);
        assert!(funcs[0].qualified_name.contains("hello"));
        assert_eq!(funcs[0].kind, NodeKind::Unit);
        assert_eq!(funcs[0].language, "python");
    }

    #[test]
    fn extracts_class_definition() {
        let result = parse_py_source("class MyService:\n    pass\n");
        let classes: Vec<_> = result.items.iter().filter(|i| i.sub_kind == "class").collect();
        assert_eq!(classes.len(), 1);
        assert!(classes[0].qualified_name.contains("MyService"));
        assert_eq!(classes[0].kind, NodeKind::Unit);
    }

    #[test]
    fn extracts_class_methods() {
        let result = parse_py_source(
            "class MyService:\n    def start(self):\n        pass\n    def stop(self):\n        pass\n",
        );
        let methods: Vec<_> = result.items.iter().filter(|i| i.sub_kind == "method").collect();
        assert_eq!(methods.len(), 2);
    }

    #[test]
    fn extracts_import_relations() {
        let result = parse_py_source("import os\nfrom pathlib import Path\n");
        let imports: Vec<_> = result.relations.iter().filter(|r| r.kind == EdgeKind::Depends).collect();
        assert!(imports.len() >= 2, "should have import dependencies for os and pathlib");
    }

    #[test]
    fn extracts_decorated_function() {
        let result = parse_py_source("@staticmethod\ndef helper():\n    pass\n");
        let funcs: Vec<_> = result.items.iter().filter(|i| i.sub_kind == "function").collect();
        assert_eq!(funcs.len(), 1);
    }

    #[test]
    fn handles_empty_file() {
        let result = parse_py_source("");
        assert!(result.items.is_empty());
    }

    #[test]
    fn language_id_is_python() {
        let analyzer = PythonAnalyzer::new();
        assert_eq!(analyzer.language_id(), "python");
    }
}
```

**Step 2: Implement `PythonAnalyzer`**

Write implementation above tests in `python.rs`:
```rust
//! Python language analyzer using tree-sitter-python.
//!
//! Extracts structural elements (classes, functions, methods) and import
//! relationships from Python source files.

use std::path::Path;

use svt_core::model::{EdgeKind, NodeKind};

use crate::types::{AnalysisItem, AnalysisRelation, AnalysisWarning};

use super::{LanguageAnalyzer, ParseResult};

/// Python source code analyzer using tree-sitter-python.
#[derive(Debug)]
pub struct PythonAnalyzer {
    _private: (),
}

impl PythonAnalyzer {
    /// Create a new `PythonAnalyzer`.
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for PythonAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAnalyzer for PythonAnalyzer {
    fn language_id(&self) -> &str {
        "python"
    }

    fn analyze_crate(&self, package_name: &str, files: &[&Path]) -> ParseResult {
        let mut result = ParseResult::default();

        let mut parser = tree_sitter::Parser::new();
        if parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .is_err()
        {
            result.warnings.push(AnalysisWarning {
                source_ref: String::new(),
                message: "failed to load tree-sitter-python grammar".to_string(),
            });
            return result;
        }

        for file in files {
            match std::fs::read_to_string(file) {
                Ok(source) => {
                    parse_python_file(
                        &mut parser,
                        &source,
                        file,
                        package_name,
                        &mut result,
                    );
                }
                Err(e) => {
                    result.warnings.push(AnalysisWarning {
                        source_ref: file.display().to_string(),
                        message: format!("failed to read file: {e}"),
                    });
                }
            }
        }

        result
    }
}

fn parse_python_file(
    parser: &mut tree_sitter::Parser,
    source: &str,
    file: &Path,
    package_name: &str,
    result: &mut ParseResult,
) {
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => {
            result.warnings.push(AnalysisWarning {
                source_ref: file.display().to_string(),
                message: "tree-sitter parse returned None".to_string(),
            });
            return;
        }
    };

    let root = tree.root_node();
    let source_ref_base = file.display().to_string();

    for child in root.children(&mut root.walk()) {
        match child.kind() {
            "function_definition" => {
                extract_function(&child, source, package_name, None, &source_ref_base, result);
            }
            "class_definition" => {
                extract_class(&child, source, package_name, &source_ref_base, result);
            }
            "decorated_definition" => {
                // The decorated child is the actual function/class
                for inner in child.children(&mut child.walk()) {
                    match inner.kind() {
                        "function_definition" => {
                            extract_function(
                                &inner, source, package_name, None, &source_ref_base, result,
                            );
                        }
                        "class_definition" => {
                            extract_class(
                                &inner, source, package_name, &source_ref_base, result,
                            );
                        }
                        _ => {}
                    }
                }
            }
            "import_statement" => {
                extract_import(&child, source, package_name, result);
            }
            "import_from_statement" => {
                extract_import_from(&child, source, package_name, result);
            }
            _ => {}
        }
    }
}

fn extract_function(
    node: &tree_sitter::Node,
    source: &str,
    package_name: &str,
    class_name: Option<&str>,
    source_ref_base: &str,
    result: &mut ParseResult,
) {
    let name = match node.child_by_field_name("name") {
        Some(n) => match n.utf8_text(source.as_bytes()) {
            Ok(s) => s.to_string(),
            Err(_) => return,
        },
        None => return,
    };

    // Skip private/dunder methods for noise reduction (keep __init__)
    if name.starts_with('_') && name != "__init__" {
        return;
    }

    let line = node.start_position().row + 1;
    let (qn, sub_kind, parent_qn) = if let Some(cls) = class_name {
        (
            format!("{package_name}::{cls}::{name}"),
            "method",
            Some(format!("{package_name}::{cls}")),
        )
    } else {
        (
            format!("{package_name}::{name}"),
            "function",
            Some(package_name.to_string()),
        )
    };

    result.items.push(AnalysisItem {
        qualified_name: qn,
        kind: NodeKind::Unit,
        sub_kind: sub_kind.to_string(),
        parent_qualified_name: parent_qn,
        source_ref: format!("{source_ref_base}:{line}"),
        language: "python".to_string(),
    });
}

fn extract_class(
    node: &tree_sitter::Node,
    source: &str,
    package_name: &str,
    source_ref_base: &str,
    result: &mut ParseResult,
) {
    let name = match node.child_by_field_name("name") {
        Some(n) => match n.utf8_text(source.as_bytes()) {
            Ok(s) => s.to_string(),
            Err(_) => return,
        },
        None => return,
    };

    let line = node.start_position().row + 1;
    result.items.push(AnalysisItem {
        qualified_name: format!("{package_name}::{name}"),
        kind: NodeKind::Unit,
        sub_kind: "class".to_string(),
        parent_qualified_name: Some(package_name.to_string()),
        source_ref: format!("{source_ref_base}:{line}"),
        language: "python".to_string(),
    });

    // Extract methods from class body
    if let Some(body) = node.child_by_field_name("body") {
        for child in body.children(&mut body.walk()) {
            match child.kind() {
                "function_definition" => {
                    extract_function(
                        &child,
                        source,
                        package_name,
                        Some(&name),
                        source_ref_base,
                        result,
                    );
                }
                "decorated_definition" => {
                    for inner in child.children(&mut child.walk()) {
                        if inner.kind() == "function_definition" {
                            extract_function(
                                &inner,
                                source,
                                package_name,
                                Some(&name),
                                source_ref_base,
                                result,
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn extract_import(
    node: &tree_sitter::Node,
    source: &str,
    package_name: &str,
    result: &mut ParseResult,
) {
    // import foo, bar, baz
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "dotted_name" {
            if let Ok(name) = child.utf8_text(source.as_bytes()) {
                result.relations.push(AnalysisRelation {
                    source_qualified_name: package_name.to_string(),
                    target_qualified_name: name.replace('.', "::"),
                    kind: EdgeKind::Depends,
                });
            }
        }
    }
}

fn extract_import_from(
    node: &tree_sitter::Node,
    source: &str,
    package_name: &str,
    result: &mut ParseResult,
) {
    // from foo.bar import baz
    if let Some(module_name) = node.child_by_field_name("module_name") {
        if let Ok(name) = module_name.utf8_text(source.as_bytes()) {
            result.relations.push(AnalysisRelation {
                source_qualified_name: package_name.to_string(),
                target_qualified_name: name.replace('.', "::"),
                kind: EdgeKind::Depends,
            });
        }
    }
}
```

**Step 3: Add `pub mod python;` to `languages/mod.rs`**

**Step 4: Run tests**

Run: `cargo test -p svt-analyzer python`
Expected: all 7 Python analyzer tests pass

**Step 5: Commit**

```bash
git add crates/analyzer/src/languages/python.rs crates/analyzer/src/languages/mod.rs
git commit -m "feat(analyzer): add Python language analyzer (class/function/method/import extraction)"
```

---

### Task 6: Register Analyzers + Integrate into `analyze_project()`

**Files:**
- Modify: `crates/analyzer/src/languages/mod.rs` — register Go + Python in `with_defaults()`
- Modify: `crates/analyzer/src/lib.rs` — add Go + Python phases
- Modify: `crates/analyzer/src/types.rs` — add counters to `AnalysisSummary`

**Step 1: Register in `AnalyzerRegistry::with_defaults()`**

In `languages/mod.rs`, update `with_defaults()`:
```rust
pub fn with_defaults() -> Self {
    let mut registry = Self::new();
    registry.register(Box::new(rust::RustAnalyzer::new()));
    registry.register(Box::new(typescript::TypeScriptAnalyzer::new()));
    registry.register(Box::new(go::GoAnalyzer::new()));
    registry.register(Box::new(python::PythonAnalyzer::new()));
    registry
}
```

Update the registry test:
```rust
fn analyzer_registry_with_defaults_has_all_built_ins() {
    let registry = AnalyzerRegistry::with_defaults();
    assert!(registry.get("rust").is_some());
    assert!(registry.get("typescript").is_some());
    assert!(registry.get("go").is_some());
    assert!(registry.get("python").is_some());
    let mut ids = registry.language_ids();
    ids.sort();
    assert_eq!(ids, vec!["go", "python", "rust", "typescript"]);
}
```

**Step 2: Add counters to `AnalysisSummary`**

In `types.rs`, add to `AnalysisSummary`:
```rust
/// Number of Go modules analyzed.
pub go_packages_analyzed: usize,
/// Number of Python packages analyzed.
pub python_packages_analyzed: usize,
```

**Step 3: Add Go and Python phases to `analyze_project()`**

In `lib.rs`, add imports and phases after existing Phase 2:

```rust
// Phase 3: Go analysis
let go_packages = discover_go_packages(project_root).unwrap_or_default();
let go_analyzer = languages::go::GoAnalyzer::new();
let mut go_packages_analyzed = 0;

for package in &go_packages {
    // Emit module-level item
    all_items.push(AnalysisItem {
        qualified_name: package.name.clone(),
        kind: NodeKind::Service,
        sub_kind: "module".to_string(),
        parent_qualified_name: None,
        source_ref: package.root.join("go.mod").display().to_string(),
        language: "go".to_string(),
    });

    let file_refs: Vec<&Path> = package.source_files.iter().map(|p| p.as_path()).collect();
    files_analyzed += file_refs.len();

    let parse_result = go_analyzer.analyze_crate(&package.name, &file_refs);
    all_items.extend(parse_result.items);
    all_relations.extend(parse_result.relations);
    all_warnings.extend(parse_result.warnings);
    go_packages_analyzed += 1;
}

// Phase 4: Python analysis
let python_packages = discover_python_packages(project_root).unwrap_or_default();
let python_analyzer = languages::python::PythonAnalyzer::new();
let mut python_packages_analyzed = 0;

for package in &python_packages {
    // Emit package-level item
    all_items.push(AnalysisItem {
        qualified_name: package.name.replace('-', "_"),
        kind: NodeKind::Service,
        sub_kind: "package".to_string(),
        parent_qualified_name: None,
        source_ref: package.root.display().to_string(),
        language: "python".to_string(),
    });

    let file_refs: Vec<&Path> = package.source_files.iter().map(|p| p.as_path()).collect();
    files_analyzed += file_refs.len();

    let parse_result = python_analyzer.analyze_crate(
        &package.name.replace('-', "_"),
        &file_refs,
    );
    all_items.extend(parse_result.items);
    all_relations.extend(parse_result.relations);
    all_warnings.extend(parse_result.warnings);
    python_packages_analyzed += 1;
}
```

Update the existing `map_to_graph` comment (Phase 3 → Phase 5) and `create_snapshot` (Phase 4 → Phase 6).

Update `AnalysisSummary` construction:
```rust
Ok(AnalysisSummary {
    version,
    crates_analyzed: layout.crates.len(),
    ts_packages_analyzed,
    go_packages_analyzed,
    python_packages_analyzed,
    files_analyzed,
    nodes_created: nodes.len(),
    edges_created: edges.len(),
    warnings: all_warnings,
})
```

Add imports in `lib.rs`:
```rust
use crate::discovery::{discover_project, discover_ts_packages, discover_go_packages, discover_python_packages};
```

**Step 4: Run tests**

Run: `cargo test -p svt-analyzer`
Expected: all existing tests still pass, plus registry test updated

**Step 5: Commit**

```bash
git add crates/analyzer/src/languages/mod.rs crates/analyzer/src/lib.rs crates/analyzer/src/types.rs
git commit -m "feat(analyzer): integrate Go and Python analyzers into analyze_project pipeline"
```

---

### Task 7: Update CLI Output + Dog-food Tests

**Files:**
- Modify: `crates/cli/src/main.rs` — update analyze output to show Go/Python counts
- Modify: `crates/analyzer/tests/dogfood.rs` — update assertions

**Step 1: Update CLI analyze output**

In `main.rs`, update the analysis summary print to include Go/Python:
```rust
println!(
    "Analysis complete (v{}):\n  {} Rust crates, {} TS packages, {} Go modules, {} Python packages\n  {} files analyzed\n  {} nodes, {} edges\n  {} warnings",
    summary.version,
    summary.crates_analyzed,
    summary.ts_packages_analyzed,
    summary.go_packages_analyzed,
    summary.python_packages_analyzed,
    summary.files_analyzed,
    summary.nodes_created,
    summary.edges_created,
    summary.warnings.len()
);
```

**Step 2: Update dogfood test output**

In `tests/dogfood.rs`, update the println to include Go/Python:
```rust
println!(
    "Dog-food analysis: {} crates, {} TS packages, {} Go modules, {} Python packages, {} files, {} nodes, {} edges, {} warnings",
    summary.crates_analyzed,
    summary.ts_packages_analyzed,
    summary.go_packages_analyzed,
    summary.python_packages_analyzed,
    summary.files_analyzed,
    summary.nodes_created,
    summary.edges_created,
    summary.warnings.len()
);
```

**Step 3: Run full test suite**

Run: `cargo test`
Expected: all tests pass (existing + new)

**Step 4: Run clippy and fmt**

Run: `cargo clippy --workspace -- -D warnings && cargo fmt --check`
Expected: clean

**Step 5: Commit**

```bash
git add crates/cli/src/main.rs crates/analyzer/tests/dogfood.rs
git commit -m "feat(cli): update analyze output for Go and Python analyzer counts"
```

---

### Task 8: Full Verification + PROGRESS.md Update

**Files:**
- Modify: `docs/plan/PROGRESS.md`

**Step 1: Run full test suite**

Run: `cargo test 2>&1 | tail -5`
Expected: all Rust tests pass

Run: `cd web && npm test -- --run 2>&1 | tail -5`
Expected: 19 vitest tests pass

**Step 2: Run quality checks**

Run: `cargo clippy --workspace -- -D warnings`
Expected: clean

Run: `cargo fmt --check`
Expected: clean

**Step 3: Update PROGRESS.md**

Add M15 row to completed milestones table and update test counts.

**Step 4: Commit and push**

```bash
git add docs/plan/PROGRESS.md
git commit -m "docs: mark milestone 15 as complete with progress summary"
git push
```
