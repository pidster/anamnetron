# Dynamic Plugin Loading Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Support external plugins loaded at runtime from shared libraries, enabling third-party constraint evaluators and export formats without recompiling SVT.

**Architecture:** SvtPlugin trait in svt-core defines the plugin contract (constraint evaluators + export formats). PluginLoader in svt-cli handles dynamic loading via libloading. CLI gains `--plugin` flag and `svt plugin list` command. Orchestrator support is deferred -- LanguageOrchestrator lives in svt-analyzer and cannot be referenced from svt-core without violating the inward dependency rule.

**Tech Stack:** Rust, libloading (dynamic library loading), thiserror (error types)

**Design doc:** `docs/plan/2026-02-20-dynamic-plugin-loading-design.md`

---

### Task 1: Define SvtPlugin trait and plugin API in svt-core

**Files:**
- Create: `crates/core/src/plugin.rs`
- Modify: `crates/core/src/lib.rs:26` (add `pub mod plugin;`)

**Context:** The `SvtPlugin` trait defines the contract all plugins implement. It lives in svt-core so plugin crates only need to depend on svt-core. The trait references `ConstraintEvaluator` (from `crate::conformance`) and `ExportFormat` (from `crate::export`), both behind the `store` feature which is always enabled for native builds. The module is gated behind `#[cfg(feature = "store")]` to match existing patterns.

**Step 1: Write the failing tests**

Create `crates/core/src/plugin.rs` with ONLY the test module:

```rust
//! Plugin API for SVT external plugins.
//!
//! Plugins are Rust cdylib shared libraries that implement [`SvtPlugin`]
//! and export a `svt_plugin_create` entry point. The host loads plugins
//! at runtime using `libloading` and registers their contributions.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conformance::ConstraintEvaluator;
    use crate::export::ExportFormat;

    struct MockPlugin;

    impl SvtPlugin for MockPlugin {
        fn name(&self) -> &str {
            "mock-plugin"
        }
        fn version(&self) -> &str {
            "0.1.0"
        }
        fn api_version(&self) -> u32 {
            SVT_PLUGIN_API_VERSION
        }
    }

    #[test]
    fn mock_plugin_has_correct_metadata() {
        let plugin = MockPlugin;
        assert_eq!(plugin.name(), "mock-plugin");
        assert_eq!(plugin.version(), "0.1.0");
        assert_eq!(plugin.api_version(), SVT_PLUGIN_API_VERSION);
    }

    #[test]
    fn mock_plugin_default_contributions_are_empty() {
        let plugin = MockPlugin;
        assert!(plugin.constraint_evaluators().is_empty());
        assert!(plugin.export_formats().is_empty());
    }

    #[test]
    fn api_version_constant_is_one() {
        assert_eq!(SVT_PLUGIN_API_VERSION, 1);
    }

    #[test]
    fn plugin_error_display_formats_correctly() {
        let err = PluginError::ApiVersionMismatch {
            plugin_name: "test".to_string(),
            expected: 1,
            actual: 2,
        };
        let msg = format!("{err}");
        assert!(msg.contains("test"), "should include plugin name");
        assert!(msg.contains("1"), "should include expected version");
        assert!(msg.contains("2"), "should include actual version");
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-core plugin::tests --no-default-features --features store-sqlite`
Expected: FAIL with "cannot find value `SVT_PLUGIN_API_VERSION`" and "cannot find trait `SvtPlugin`"

**Step 3: Implement SvtPlugin trait, API version constant, PluginError, and declare_plugin! macro**

Add the following above the `#[cfg(test)]` block in `crates/core/src/plugin.rs`:

```rust
use crate::conformance::ConstraintEvaluator;
use crate::export::ExportFormat;

/// Current plugin API version. Plugins must return this from
/// [`SvtPlugin::api_version`]. Bumped on breaking API changes.
pub const SVT_PLUGIN_API_VERSION: u32 = 1;

/// Trait implemented by all SVT plugins.
///
/// A plugin provides zero or more contributions to each extension point.
/// The host calls these methods once at load time and registers the returned
/// trait objects into the appropriate registries.
pub trait SvtPlugin: Send + Sync {
    /// Human-readable plugin name.
    fn name(&self) -> &str;

    /// Plugin version string (e.g., "0.1.0").
    fn version(&self) -> &str;

    /// Plugin API version. Must match [`SVT_PLUGIN_API_VERSION`].
    fn api_version(&self) -> u32;

    /// Constraint evaluators provided by this plugin.
    fn constraint_evaluators(&self) -> Vec<Box<dyn ConstraintEvaluator>> {
        vec![]
    }

    /// Export formats provided by this plugin.
    fn export_formats(&self) -> Vec<Box<dyn ExportFormat>> {
        vec![]
    }
}

/// Errors that occur during plugin loading.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// Failed to load the shared library.
    #[error("failed to load plugin library at {path}: {reason}")]
    LoadFailed {
        /// Path to the library.
        path: String,
        /// OS-level error message.
        reason: String,
    },

    /// The library does not export the expected entry point symbol.
    #[error("library at {path} is not an SVT plugin (missing svt_plugin_create symbol)")]
    SymbolNotFound {
        /// Path to the library.
        path: String,
    },

    /// The plugin's API version does not match the host's.
    #[error("plugin '{plugin_name}' requires API v{actual}, host supports v{expected}")]
    ApiVersionMismatch {
        /// Plugin name.
        plugin_name: String,
        /// Host's API version.
        expected: u32,
        /// Plugin's API version.
        actual: u32,
    },
}

/// Convenience macro that generates the `extern "C"` entry point for a plugin.
///
/// The plugin type must implement [`Default`] and [`SvtPlugin`].
///
/// # Example
///
/// ```ignore
/// use svt_core::plugin::SvtPlugin;
///
/// #[derive(Default)]
/// struct MyPlugin;
///
/// impl SvtPlugin for MyPlugin {
///     fn name(&self) -> &str { "my-plugin" }
///     fn version(&self) -> &str { "0.1.0" }
///     fn api_version(&self) -> u32 { svt_core::plugin::SVT_PLUGIN_API_VERSION }
/// }
///
/// svt_core::declare_plugin!(MyPlugin);
/// ```
#[macro_export]
macro_rules! declare_plugin {
    ($plugin_type:ty) => {
        #[no_mangle]
        pub extern "C" fn svt_plugin_create() -> *mut dyn $crate::plugin::SvtPlugin {
            let plugin = <$plugin_type>::default();
            Box::into_raw(Box::new(plugin) as Box<dyn $crate::plugin::SvtPlugin>)
        }
    };
}
```

Then add to `crates/core/src/lib.rs` after the `export` module (around line 36):

```rust
/// Plugin API: SvtPlugin trait, declare_plugin! macro, PluginError.
#[cfg(feature = "store")]
pub mod plugin;
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-core plugin::tests`
Expected: 4 tests PASS

**Step 5: Run clippy and fmt**

Run: `cargo clippy -p svt-core && cargo fmt -p svt-core --check`
Expected: clean

**Step 6: Commit**

```bash
git add crates/core/src/plugin.rs crates/core/src/lib.rs
git commit -m "feat(core): add SvtPlugin trait, API version, PluginError, declare_plugin! macro"
```

---

### Task 2: Implement PluginLoader in svt-cli

**Files:**
- Create: `crates/cli/src/plugin.rs`
- Modify: `crates/cli/Cargo.toml:14` (add `libloading` dependency)

**Context:** The PluginLoader handles dynamic library discovery and loading. It lives in svt-cli (not svt-core) because `libloading` is platform-specific and svt-core must remain WASM-compatible. This is a deviation from the design doc which placed the loader in svt-core -- the architecture rule "Core logic must compile to WASM" takes precedence.

The loader uses `libloading::Library::new()` to open shared libraries, looks up the `svt_plugin_create` symbol, calls it to get a `*mut dyn SvtPlugin`, wraps it in `Box::from_raw()`, checks the API version, and keeps both the Library handle and plugin Box alive.

**Step 1: Add libloading dependency**

Modify `crates/cli/Cargo.toml` to add after the `serde_yaml` line (line 16):

```toml
libloading = "0.8"
```

**Step 2: Write the failing tests**

Create `crates/cli/src/plugin.rs` with ONLY the test module:

```rust
//! Plugin loading: discovery, loading, and registration of external SVT plugins.

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn new_loader_has_no_plugins() {
        let loader = PluginLoader::new();
        assert!(loader.plugins().is_empty());
    }

    #[test]
    fn shared_library_extension_is_platform_correct() {
        let ext = shared_library_extension();
        if cfg!(target_os = "macos") {
            assert_eq!(ext, "dylib");
        } else if cfg!(target_os = "windows") {
            assert_eq!(ext, "dll");
        } else {
            assert_eq!(ext, "so");
        }
    }

    #[test]
    fn scan_empty_directory_returns_no_errors() {
        let tmp = TempDir::new().unwrap();
        let mut loader = PluginLoader::new();
        let errors = loader.scan_directory(tmp.path());
        assert!(errors.is_empty());
        assert!(loader.plugins().is_empty());
    }

    #[test]
    fn scan_nonexistent_directory_returns_no_errors() {
        let mut loader = PluginLoader::new();
        let errors = loader.scan_directory(&PathBuf::from("/nonexistent/path"));
        assert!(errors.is_empty());
        assert!(loader.plugins().is_empty());
    }

    #[test]
    fn scan_directory_skips_non_library_files() {
        let tmp = TempDir::new().unwrap();
        // Create some non-library files
        std::fs::write(tmp.path().join("readme.txt"), "not a plugin").unwrap();
        std::fs::write(tmp.path().join("data.json"), "{}").unwrap();

        let mut loader = PluginLoader::new();
        let errors = loader.scan_directory(tmp.path());
        assert!(errors.is_empty());
        assert!(loader.plugins().is_empty());
    }

    #[test]
    fn load_nonexistent_file_returns_load_failed() {
        let mut loader = PluginLoader::new();
        let result = loader.load(&PathBuf::from("/nonexistent/libfake.dylib"));
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::LoadFailed { path, .. } => {
                assert!(path.contains("nonexistent"));
            }
            other => panic!("expected LoadFailed, got: {other}"),
        }
    }

    #[test]
    fn load_non_plugin_library_returns_symbol_not_found() {
        // Try to load a system library that exists but is not an SVT plugin.
        // On macOS: /usr/lib/libSystem.B.dylib
        // On Linux: /lib/x86_64-linux-gnu/libc.so.6 or similar
        let lib_path = if cfg!(target_os = "macos") {
            PathBuf::from("/usr/lib/libSystem.B.dylib")
        } else {
            // Skip on non-macOS for now; the test still covers the code path
            return;
        };
        if !lib_path.exists() {
            return; // Skip if system lib not found
        }

        let mut loader = PluginLoader::new();
        let result = loader.load(&lib_path);
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::SymbolNotFound { path } => {
                assert!(path.contains("libSystem"));
            }
            other => panic!("expected SymbolNotFound, got: {other}"),
        }
    }

    #[test]
    fn register_all_with_no_plugins_does_nothing() {
        use svt_core::conformance::ConstraintRegistry;
        use svt_core::export::ExportRegistry;

        let loader = PluginLoader::new();
        let mut constraints = ConstraintRegistry::new();
        let mut exports = ExportRegistry::new();

        loader.register_all(&mut constraints, &mut exports);

        // Empty registries should remain empty
        assert!(constraints.kinds().is_empty());
        assert!(exports.names().is_empty());
    }
}
```

**Step 3: Run tests to verify they fail**

Run: `cargo test -p svt-cli plugin::tests`
Expected: FAIL with "cannot find struct `PluginLoader`"

**Step 4: Implement PluginLoader**

Add the following above the `#[cfg(test)]` block in `crates/cli/src/plugin.rs`:

```rust
use std::path::Path;

use svt_core::conformance::ConstraintRegistry;
use svt_core::export::ExportRegistry;
use svt_core::plugin::{PluginError, SvtPlugin, SVT_PLUGIN_API_VERSION};

/// Return the platform-specific shared library extension.
fn shared_library_extension() -> &'static str {
    if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    }
}

/// Loads and manages external SVT plugins from shared libraries.
///
/// The loader keeps `libloading::Library` handles alive for the duration
/// of the program. Dropping the loader drops the libraries and invalidates
/// all plugin trait objects.
pub struct PluginLoader {
    /// Loaded libraries (kept alive so function pointers remain valid).
    _libraries: Vec<libloading::Library>,
    /// Loaded plugin instances.
    plugins: Vec<Box<dyn SvtPlugin>>,
}

impl PluginLoader {
    /// Create an empty plugin loader.
    pub fn new() -> Self {
        Self {
            _libraries: Vec::new(),
            plugins: Vec::new(),
        }
    }

    /// Load a plugin from a specific shared library path.
    ///
    /// Returns an error if the library cannot be loaded, the entry point
    /// symbol is missing, or the API version does not match.
    pub fn load(&mut self, path: &Path) -> Result<(), PluginError> {
        let path_str = path.display().to_string();

        // Safety: loading a shared library can execute arbitrary code in
        // its init functions. We trust plugin authors (documented in design).
        let library = unsafe { libloading::Library::new(path) }.map_err(|e| {
            PluginError::LoadFailed {
                path: path_str.clone(),
                reason: e.to_string(),
            }
        })?;

        // Look up the entry point symbol.
        let create_fn: libloading::Symbol<unsafe extern "C" fn() -> *mut dyn SvtPlugin> =
            unsafe { library.get(b"svt_plugin_create") }.map_err(|_| {
                PluginError::SymbolNotFound {
                    path: path_str.clone(),
                }
            })?;

        // Call the entry point to create the plugin instance.
        // Safety: the symbol was found and has the expected signature.
        // The plugin must not panic across FFI (documented requirement).
        let raw_ptr = unsafe { create_fn() };
        let plugin = unsafe { Box::from_raw(raw_ptr) };

        // Check API version.
        let actual = plugin.api_version();
        if actual != SVT_PLUGIN_API_VERSION {
            return Err(PluginError::ApiVersionMismatch {
                plugin_name: plugin.name().to_string(),
                expected: SVT_PLUGIN_API_VERSION,
                actual,
            });
        }

        self._libraries.push(library);
        self.plugins.push(plugin);

        Ok(())
    }

    /// Scan a directory for plugin shared libraries and attempt to load each one.
    ///
    /// Returns a list of errors for plugins that failed to load. Successfully
    /// loaded plugins are added to the loader. Non-existent directories and
    /// non-library files are silently skipped.
    pub fn scan_directory(&mut self, dir: &Path) -> Vec<PluginError> {
        let mut errors = Vec::new();

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return errors, // Directory doesn't exist or isn't readable
        };

        let ext = shared_library_extension();

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some(ext) {
                if let Err(e) = self.load(&path) {
                    errors.push(e);
                }
            }
        }

        errors
    }

    /// Register all loaded plugin contributions into the provided registries.
    pub fn register_all(
        &self,
        constraints: &mut ConstraintRegistry,
        exports: &mut ExportRegistry,
    ) {
        for plugin in &self.plugins {
            for evaluator in plugin.constraint_evaluators() {
                constraints.register(evaluator);
            }
            for format in plugin.export_formats() {
                exports.register(format);
            }
        }
    }

    /// Return all loaded plugins.
    pub fn plugins(&self) -> &[Box<dyn SvtPlugin>] {
        &self.plugins
    }
}
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p svt-cli plugin::tests`
Expected: 7 tests PASS

**Step 6: Run clippy and fmt**

Run: `cargo clippy -p svt-cli && cargo fmt -p svt-cli --check`
Expected: clean

**Step 7: Commit**

```bash
git add crates/cli/Cargo.toml crates/cli/src/plugin.rs
git commit -m "feat(cli): add PluginLoader with dynamic library discovery and loading"
```

---

### Task 3: CLI integration (--plugin flag, `svt plugin list`)

**Files:**
- Modify: `crates/cli/src/main.rs:17-24` (add --plugin flag to Cli struct)
- Modify: `crates/cli/src/main.rs:27-38` (add Plugin command)
- Modify: `crates/cli/src/main.rs:171-231` (wire loader into run_check, run_export)
- Modify: `crates/cli/src/main.rs:480-490` (dispatch Plugin command)

**Context:** The CLI needs three changes: (1) a `--plugin <path>` flag on the top-level Cli struct for loading specific plugins, (2) a `svt plugin list` subcommand that shows loaded plugins and their contributions, and (3) wiring the PluginLoader into `run_check` and `run_export` so plugin-provided constraint evaluators and export formats are available.

**Step 1: Write the failing test**

Add an integration test at `crates/cli/tests/plugin_cli.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn plugin_list_with_no_plugins_shows_empty() {
    Command::cargo_bin("svt")
        .unwrap()
        .args(["plugin", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No plugins loaded"));
}

#[test]
fn plugin_flag_with_nonexistent_file_warns() {
    Command::cargo_bin("svt")
        .unwrap()
        .args(["--plugin", "/nonexistent/lib.dylib", "plugin", "list"])
        .assert()
        .success() // Should not fail, just warn and skip
        .stderr(predicate::str::contains("failed to load plugin"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-cli --test plugin_cli`
Expected: FAIL with "unrecognized subcommand 'plugin'"

**Step 3: Implement CLI changes**

Modify `crates/cli/src/main.rs`:

First, add `mod plugin;` near the top (after the imports):

```rust
mod plugin;
```

Add `--plugin` flag to the `Cli` struct (after the `store` field, around line 21):

```rust
    /// Load external plugin(s) from shared library paths.
    #[arg(long = "plugin", global = true)]
    plugins: Vec<PathBuf>,
```

Add `Plugin` variant to the `Commands` enum (after the `Diff` variant, around line 37):

```rust
    /// Manage and list loaded plugins.
    Plugin(PluginArgs),
```

Add `PluginArgs` struct (after `DiffArgs`, around line 105):

```rust
#[derive(clap::Args, Debug)]
struct PluginArgs {
    #[command(subcommand)]
    command: PluginCommands,
}

#[derive(Subcommand, Debug)]
enum PluginCommands {
    /// List all loaded plugins and their contributions.
    List,
}
```

Add helper function to build the PluginLoader (before `main`):

```rust
/// Build a PluginLoader from CLI flags and convention directories.
fn build_plugin_loader(plugin_paths: &[PathBuf]) -> plugin::PluginLoader {
    let mut loader = plugin::PluginLoader::new();

    // 1. CLI-specified plugins
    for path in plugin_paths {
        if let Err(e) = loader.load(path) {
            eprintln!("  WARN  {e}");
        }
    }

    // 2. Project-local plugins (.svt/plugins/)
    let local_dir = PathBuf::from(".svt/plugins");
    for e in loader.scan_directory(&local_dir) {
        eprintln!("  WARN  {e}");
    }

    // 3. User-global plugins (~/.svt/plugins/)
    if let Some(home) = dirs_or_home() {
        let global_dir = home.join(".svt/plugins");
        for e in loader.scan_directory(&global_dir) {
            eprintln!("  WARN  {e}");
        }
    }

    loader
}

/// Get the user's home directory.
fn dirs_or_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
```

Add `run_plugin_list` function:

```rust
fn run_plugin_list(loader: &plugin::PluginLoader) -> Result<()> {
    let plugins = loader.plugins();
    if plugins.is_empty() {
        println!("No plugins loaded.");
        return Ok(());
    }

    for plugin in plugins {
        println!("{} v{} (API v{})", plugin.name(), plugin.version(), plugin.api_version());
        let evaluators = plugin.constraint_evaluators();
        if !evaluators.is_empty() {
            println!("  Constraint evaluators:");
            for eval in &evaluators {
                println!("    - {}", eval.kind());
            }
        }
        let formats = plugin.export_formats();
        if !formats.is_empty() {
            println!("  Export formats:");
            for fmt in &formats {
                println!("    - {}", fmt.name());
            }
        }
        if evaluators.is_empty() && formats.is_empty() {
            println!("  (no contributions)");
        }
    }
    Ok(())
}
```

Modify `run_check` to accept a `PluginLoader` reference and register plugin contributions (change the function signature and registry initialization):

Replace line `let registry = ConstraintRegistry::with_defaults();` with:

```rust
    let mut registry = ConstraintRegistry::with_defaults();
    let mut _export_registry = ExportRegistry::new(); // needed for register_all signature
    loader.register_all(&mut registry, &mut _export_registry);
```

The function signature becomes:
```rust
fn run_check(store_path: &Path, args: &CheckArgs, loader: &plugin::PluginLoader) -> Result<()> {
```

Modify `run_export` similarly. The function signature becomes:
```rust
fn run_export(store_path: &Path, args: &ExportArgs, loader: &plugin::PluginLoader) -> Result<()> {
```

Replace `let registry = ExportRegistry::with_defaults();` with:
```rust
    let mut registry = ExportRegistry::with_defaults();
    let mut _constraint_registry = ConstraintRegistry::new(); // needed for register_all signature
    loader.register_all(&mut _constraint_registry, &mut registry);
```

Update `main()` to build the loader and dispatch:

```rust
fn main() -> Result<()> {
    let cli = Cli::parse();
    let loader = build_plugin_loader(&cli.plugins);

    match &cli.command {
        Commands::Import(args) => run_import(&cli.store, args),
        Commands::Check(args) => run_check(&cli.store, args, &loader),
        Commands::Analyze(args) => run_analyze(&cli.store, args),
        Commands::Export(args) => run_export(&cli.store, args, &loader),
        Commands::Diff(args) => run_diff(&cli.store, args),
        Commands::Plugin(args) => match &args.command {
            PluginCommands::List => run_plugin_list(&loader),
        },
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-cli --test plugin_cli`
Expected: 2 tests PASS

Also run all CLI tests:
Run: `cargo test -p svt-cli`
Expected: All tests PASS (existing tests should not break)

**Step 5: Run clippy and fmt**

Run: `cargo clippy -p svt-cli && cargo fmt -p svt-cli --check`
Expected: clean

**Step 6: Commit**

```bash
git add crates/cli/src/main.rs crates/cli/tests/plugin_cli.rs
git commit -m "feat(cli): add --plugin flag, svt plugin list command, wire plugins into check and export"
```

---

### Task 4: Mock plugin registration end-to-end test

**Files:**
- Modify: `crates/core/src/plugin.rs` (add mock plugin with real contributions)

**Context:** This test verifies that a mock plugin's contributions (constraint evaluator + export format) are correctly registered into the respective registries. It tests the full in-process flow without requiring a cdylib.

**Step 1: Write the failing test**

Add the following tests to the `#[cfg(test)]` module in `crates/core/src/plugin.rs`:

```rust
    use crate::conformance::{ConstraintEvaluator, ConstraintRegistry, ConstraintResult, ConstraintStatus};
    use crate::export::{ExportFormat, ExportRegistry};
    use crate::model::{Constraint, Severity, Version};
    use crate::store::{GraphStore, Result as StoreResult};

    /// A mock constraint evaluator for testing.
    #[derive(Debug)]
    struct MockEvaluator;

    impl ConstraintEvaluator for MockEvaluator {
        fn kind(&self) -> &str {
            "mock_constraint"
        }
        fn evaluate(
            &self,
            _store: &dyn GraphStore,
            constraint: &Constraint,
            _version: Version,
        ) -> StoreResult<ConstraintResult> {
            Ok(ConstraintResult {
                constraint_name: constraint.name.clone(),
                constraint_kind: "mock_constraint".to_string(),
                status: ConstraintStatus::Pass,
                severity: Severity::Info,
                message: "Mock always passes".to_string(),
                violations: vec![],
            })
        }
    }

    /// A mock export format for testing.
    #[derive(Debug)]
    struct MockFormat;

    impl ExportFormat for MockFormat {
        fn name(&self) -> &str {
            "mock"
        }
        fn export(&self, _store: &dyn GraphStore, _version: Version) -> StoreResult<String> {
            Ok("mock export output".to_string())
        }
    }

    /// A plugin that provides real contributions.
    struct ContributingPlugin;

    impl SvtPlugin for ContributingPlugin {
        fn name(&self) -> &str {
            "contributing-plugin"
        }
        fn version(&self) -> &str {
            "0.2.0"
        }
        fn api_version(&self) -> u32 {
            SVT_PLUGIN_API_VERSION
        }
        fn constraint_evaluators(&self) -> Vec<Box<dyn ConstraintEvaluator>> {
            vec![Box::new(MockEvaluator)]
        }
        fn export_formats(&self) -> Vec<Box<dyn ExportFormat>> {
            vec![Box::new(MockFormat)]
        }
    }

    #[test]
    fn contributing_plugin_provides_evaluator() {
        let plugin = ContributingPlugin;
        let evaluators = plugin.constraint_evaluators();
        assert_eq!(evaluators.len(), 1);
        assert_eq!(evaluators[0].kind(), "mock_constraint");
    }

    #[test]
    fn contributing_plugin_provides_format() {
        let plugin = ContributingPlugin;
        let formats = plugin.export_formats();
        assert_eq!(formats.len(), 1);
        assert_eq!(formats[0].name(), "mock");
    }

    #[test]
    fn plugin_contributions_register_into_registries() {
        let plugin = ContributingPlugin;

        let mut constraints = ConstraintRegistry::new();
        let mut exports = ExportRegistry::new();

        for evaluator in plugin.constraint_evaluators() {
            constraints.register(evaluator);
        }
        for format in plugin.export_formats() {
            exports.register(format);
        }

        assert!(
            constraints.get("mock_constraint").is_some(),
            "mock_constraint should be registered"
        );
        assert!(
            exports.get("mock").is_some(),
            "mock format should be registered"
        );
    }
```

**Step 2: Run tests to verify they pass**

Run: `cargo test -p svt-core plugin::tests`
Expected: 7 tests PASS (4 from Task 1 + 3 new)

**Step 3: Commit**

```bash
git add crates/core/src/plugin.rs
git commit -m "test(core): add mock plugin registration end-to-end tests"
```

---

### Task 5: Dog-food verification and documentation

**Files:**
- Modify: `docs/plan/PROGRESS.md`

**Context:** Verify that all existing tests still pass, clippy and fmt are clean, and the plugin system works as expected. Update PROGRESS.md with the new milestone.

**Step 1: Run the full test suite**

Run: `cargo test`
Expected: All tests PASS

**Step 2: Run clippy and fmt across workspace**

Run: `cargo clippy --workspace && cargo fmt --all --check`
Expected: clean

**Step 3: Verify svt plugin list works**

Run: `cargo run -- plugin list`
Expected: "No plugins loaded."

**Step 4: Verify svt plugin list with --plugin flag (error case)**

Run: `cargo run -- --plugin /nonexistent/lib.dylib plugin list`
Expected: stderr has "WARN  failed to load plugin library", stdout has "No plugins loaded."

**Step 5: Update PROGRESS.md**

Add a new section for M17: Dynamic Plugin Loading with status information.

**Step 6: Commit**

```bash
git add docs/plan/PROGRESS.md
git commit -m "docs: update progress for dynamic plugin loading (M17)"
```
