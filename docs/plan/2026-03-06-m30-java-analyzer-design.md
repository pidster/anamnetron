# M30: Java Analyzer — Design

## Goal

Add Java language support to the analysis pipeline, enabling structural extraction from Java source code and project discovery for Maven and Gradle projects. Follows the established `LanguageDescriptor` + `LanguageParser` pattern used by Go, Python, and TypeScript analyzers.

## Architecture

The Java analyzer uses the same descriptor + parser architecture as other language analyzers:

- **`JavaAnalyzer`** in `crates/analyzer/src/languages/java.rs` implements `LanguageAnalyzer` and `CoreLanguageParser`
- **`LanguageDescriptor`** configures discovery (manifest files, extensions, skip directories)
- **`DescriptorOrchestrator`** in `crates/analyzer/src/orchestrator/java.rs` delegates discovery and parsing
- **tree-sitter-java** provides the grammar for AST-based structural extraction

No new crate dependencies are needed beyond `tree-sitter-java`. All parsing uses simple string matching for manifest files (pom.xml, build.gradle) — no XML or Groovy parser crate required.

## Language Descriptor

```rust
LanguageDescriptor {
    language_id: "java",
    manifest_files: ["pom.xml", "build.gradle", "build.gradle.kts"],
    source_extensions: [".java"],
    skip_directories: ["target", "build", ".gradle", ".git", "node_modules", ".idea"],
    top_level_kind: NodeKind::Service,
    top_level_sub_kind: "module",
}
```

## Structural Extraction

### Node Mapping

| Java Element       | NodeKind    | sub_kind      | Notes                                  |
|--------------------|-------------|---------------|----------------------------------------|
| Package directory  | Component   | `package`     | Emitted by `emit_structural_items()`   |
| Class              | Unit        | `class`       | Top-level and nested                   |
| Interface          | Unit        | `interface`   |                                        |
| Enum               | Unit        | `enum`        |                                        |
| Annotation type    | Unit        | `annotation`  | `@interface` declarations              |
| Method             | Unit        | `method`      | Instance and static methods            |
| Constructor        | Unit        | `constructor` |                                        |
| Field              | Unit        | `field`       | Instance and static fields             |

### Edge Mapping

| Relationship                  | EdgeKind     | Notes                                    |
|-------------------------------|--------------|------------------------------------------|
| Class extends class           | `Extends`    | Single inheritance                       |
| Interface extends interface   | `Extends`    | Multiple allowed                         |
| Class implements interface    | `Implements` | Multiple allowed                         |
| Method calls method           | `Calls`      | From `visit_java_call_expressions()`     |
| Import dependency             | `Depends`    | Package-level, from import statements    |
| Cross-module dependency       | `Depends`    | From Maven/Gradle declared dependencies  |

### Tree-Sitter Node Types

Key tree-sitter-java grammar nodes used for extraction:

- `class_declaration` — class name, superclass, superinterfaces, body
- `interface_declaration` — interface name, extends_interfaces, body
- `enum_declaration` — enum name, interfaces, body
- `annotation_type_declaration` — annotation name, body
- `method_declaration` — return type, name, parameters, body, annotations
- `constructor_declaration` — name, parameters, body
- `field_declaration` — type, declarators
- `import_declaration` — scoped identifier path, wildcard flag, static flag
- `package_declaration` — scoped identifier path
- `method_invocation` — object, name, arguments

## Package Hierarchy

Java packages map to `Component` nodes derived from directory structure under source roots (`src/main/java/`, `src/test/java/`).

`emit_structural_items()` walks the source file paths, extracts the directory structure relative to the source root, and emits one `Component` node per unique package directory. For example, `src/main/java/com/example/service/` produces:

- `module_name::com` (Component, package)
- `module_name::com.example` (Component, package)
- `module_name::com.example.service` (Component, package)

`post_process()` reparents class/interface/enum items under their package's qualified name using the `package` declaration from each source file.

## Import Resolution

Imports are extracted from `import_declaration` nodes:

- **Single-type import**: `import com.example.Foo;` — emits `Depends` edge from current package to `com.example.Foo`
- **Wildcard import**: `import com.example.*;` — emits `Depends` edge from current package to `com.example`
- **Static import**: `import static com.example.Foo.bar;` — emits `Depends` edge similarly

Import targets are stored during parsing for use in `post_process()` to resolve `Extends` and `Implements` edges to their fully qualified names.

## Extends / Implements Edges

Extracted from heritage clauses in class/interface declarations:

- `class Foo extends Bar` — `Extends` edge from `Foo` to `Bar`
- `class Foo implements Baz, Qux` — `Implements` edges from `Foo` to each interface
- `interface Foo extends Bar, Baz` — `Extends` edges from `Foo` to each parent interface

In `post_process()`, simple names (e.g., `Bar`) are resolved to qualified names using the collected import map. If no import matches, the name is assumed to be in the same package.

## Call Graph Analysis

`visit_java_call_expressions()` walks method/constructor bodies recursively, looking for `method_invocation` nodes:

- **Simple call**: `foo()` — emits `Calls` edge to `module::package::EnclosingClass::foo`
- **Qualified call**: `obj.method()` — emits `Calls` edge; object type resolved heuristically from local scope or left as `obj.method`
- **This call**: `this.method()` — emits `Calls` edge to current class's method
- **Static call**: `ClassName.method()` — emits `Calls` edge to `ClassName::method`
- **Chained calls**: `a.b().c()` — each invocation emitted separately

As with other analyzers, precise type inference is not attempted. Method call targets use best-effort heuristic resolution.

## Test Detection

Two complementary strategies:

### Path-Based Detection
- Files under `src/test/java/` are tagged as test files
- `is_java_test_file(path)` checks for this path pattern

### Annotation-Based Detection
- JUnit 5: `@Test`, `@ParameterizedTest`, `@RepeatedTest`
- JUnit 4: `@Test` (from `org.junit.Test`)
- TestNG: `@Test` (from `org.testng.annotations.Test`)

Methods with any of these annotations receive the `"test"` tag. Files containing test-annotated methods also have their items tagged.

`java_test_tags()` inspects method annotations to determine test status.

## Maven Discovery

`pom.xml` manifests are detected by the `DescriptorOrchestrator` walker.

### Name Extraction
From `pom.xml`, extract `<artifactId>` as the module name. Simple line-based parsing:
- Find `<artifactId>...</artifactId>` — use first occurrence (parent artifactId comes after `<parent>` block)

### Dependency Extraction
`parse_maven_dependencies()` extracts `<groupId>` and `<artifactId>` from `<dependency>` elements. Only workspace-internal dependencies (matching other discovered unit names) are kept.

### Multi-Module
Maven multi-module projects list submodules in `<modules>`. The `DescriptorOrchestrator` discovers these automatically by walking subdirectories that contain their own `pom.xml`.

## Gradle Discovery

`build.gradle` and `build.gradle.kts` manifests are detected by the walker.

### Name Extraction
From build scripts, look for:
- `rootProject.name = 'my-project'` in `settings.gradle`
- `group = 'com.example'` and project directory name as fallback

Simple line-based parsing with string matching.

### Dependency Extraction
`parse_gradle_dependencies()` extracts from dependency blocks:
- `implementation 'group:artifact:version'`
- `implementation "group:artifact:version"`
- `api 'group:artifact:version'`
- `compile 'group:artifact:version'` (legacy)

### Multi-Module
Gradle multi-module projects use `include` in `settings.gradle`. Subdirectories with their own `build.gradle` are discovered automatically by the walker.

## Files to Create / Modify

### New Files
- `crates/analyzer/src/languages/java.rs` — Java language parser (~600-800 lines)
- `crates/analyzer/src/orchestrator/java.rs` — Java orchestrator (small, ~20 lines)

### Modified Files
- `crates/analyzer/Cargo.toml` — add `tree-sitter-java` dependency
- `crates/analyzer/src/languages/mod.rs` — add `pub mod java;`, register in `AnalyzerRegistry`
- `crates/analyzer/src/orchestrator/mod.rs` — add `pub mod java;`, register in `OrchestratorRegistry`
- `crates/analyzer/src/orchestrator/descriptor.rs` — add `"java"` branch in `extract_dependencies_from_manifest`, add XML/Gradle name extraction
- `crates/analyzer/src/types.rs` — add `java_packages_analyzed` to `AnalysisSummary`
- `crates/analyzer/src/lib.rs` — populate java counter in both analysis functions
- `crates/cli/src/main.rs` — add Java count to CLI output

## Testing Strategy

- Unit tests inline in `java.rs` for each extraction function
- Test both happy paths and edge cases (empty classes, nested classes, multiple inheritance)
- Property-based tests for serialization round-trips if applicable
- Integration test: analyze a synthetic Java project with Maven structure
- Registry tests: verify "java" appears in both `AnalyzerRegistry` and `OrchestratorRegistry`
