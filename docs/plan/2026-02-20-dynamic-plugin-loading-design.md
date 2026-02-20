# Dynamic Plugin Loading -- Design

## Goal

Support external plugins loaded at runtime from shared libraries, enabling third-party language analyzers, custom constraint evaluators, and export formats without recompiling SVT.

## Approach: C ABI Entry Point + SvtPlugin Trait

Each plugin is a Rust cdylib that links against `svt-core` and exports a single `extern "C"` entry point. The host loads the library with `libloading`, calls the entry point, and receives a `Box<dyn SvtPlugin>` containing all contributions.

Since both host and plugin compile against the same `svt-core` crate, trait objects and Rust types are layout-compatible. The `extern "C"` is only needed for the entry point symbol lookup.

### Plugin API

```rust
/// Current plugin API version. Plugins must return this from api_version().
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

    /// Plugin API version. Must match SVT_PLUGIN_API_VERSION.
    fn api_version(&self) -> u32;

    /// Constraint evaluators provided by this plugin.
    fn constraint_evaluators(&self) -> Vec<Box<dyn ConstraintEvaluator>> { vec![] }

    /// Export formats provided by this plugin.
    fn export_formats(&self) -> Vec<Box<dyn ExportFormat>> { vec![] }

    /// Language orchestrators provided by this plugin.
    fn orchestrators(&self) -> Vec<Box<dyn LanguageOrchestrator>> { vec![] }
}
```

### Plugin Entry Point

Each plugin cdylib exports:

```rust
#[no_mangle]
pub extern "C" fn svt_plugin_create() -> *mut dyn SvtPlugin
```

The host calls this, wraps the result in `Box::from_raw()`, checks `api_version()`, and registers contributions.

### Convenience Macro

`svt-core` provides a `declare_plugin!` macro that generates the `extern "C"` entry point:

```rust
svt_core::declare_plugin!(MyPlugin);

// Expands to:
#[no_mangle]
pub extern "C" fn svt_plugin_create() -> *mut dyn SvtPlugin {
    let plugin = MyPlugin::default();
    Box::into_raw(Box::new(plugin) as Box<dyn SvtPlugin>)
}
```

## Discovery

Plugins are discovered from three sources, in order:

1. **CLI flag:** `--plugin path/to/lib.dylib` (explicit, for development)
2. **Project-local:** `.svt/plugins/` directory in the project root
3. **User-global:** `~/.svt/plugins/` directory

File convention: any file with the platform's shared library extension (`.so` on Linux, `.dylib` on macOS, `.dll` on Windows) is treated as a potential plugin.

### Loading Flow

1. Collect plugin paths from CLI flags + directory scans
2. For each path:
   a. `libloading::Library::new(path)` -- if this fails, warn and skip
   b. Look up symbol `svt_plugin_create` -- if missing, warn and skip
   c. Call the symbol to get `*mut dyn SvtPlugin`
   d. Wrap in `unsafe { Box::from_raw(ptr) }`
   e. Check `api_version()` against `SVT_PLUGIN_API_VERSION` -- if mismatch, warn and skip
   f. Register all contributions into registries
3. Continue with normal operation

## CLI Integration

- **`svt plugin list`** -- list all discovered plugins and their contributions (name, version, what they provide)
- **`--plugin <path>`** global flag on `svt analyze`, `svt check`, `svt export` -- load a specific plugin
- Plugin contributions appear transparently in existing commands (e.g., `svt export --format custom` works if a plugin registered format "custom")

## Module Structure

New module in `svt-core`:

```
crates/core/src/plugin/
  mod.rs          -- SvtPlugin trait, SVT_PLUGIN_API_VERSION, declare_plugin! macro
  loader.rs       -- PluginLoader: discovery, loading, registration
```

`PluginLoader` struct:

```rust
pub struct PluginLoader {
    /// Loaded libraries (kept alive for the duration of the program).
    _libraries: Vec<libloading::Library>,
    /// Loaded plugin instances.
    plugins: Vec<Box<dyn SvtPlugin>>,
}

impl PluginLoader {
    pub fn new() -> Self;

    /// Load a plugin from a specific path.
    pub fn load(&mut self, path: &Path) -> Result<(), PluginError>;

    /// Scan a directory for plugins.
    pub fn scan_directory(&mut self, dir: &Path) -> Vec<PluginError>;

    /// Register all loaded plugin contributions into registries.
    pub fn register_all(
        &self,
        constraints: &mut ConstraintRegistry,
        exports: &mut ExportRegistry,
        orchestrators: &mut OrchestratorRegistry,
    );

    /// List loaded plugins.
    pub fn plugins(&self) -> &[Box<dyn SvtPlugin>];
}
```

## Error Handling

- **Missing/corrupt library:** Warning with path + OS error, skip
- **Symbol not found:** Warning "not an SVT plugin", skip
- **API version mismatch:** Warning "plugin X v0.1.0 requires API v2, host supports v1", skip
- **Plugin panics:** Documented as undefined behavior -- plugins must not panic across FFI. The `declare_plugin!` macro could wrap in `catch_unwind` for best-effort protection.

## Version Compatibility

`SVT_PLUGIN_API_VERSION` is bumped when the plugin API changes in a breaking way. The version number is checked at load time. Plugins compiled against a different API version are rejected with a clear error.

Since plugins link against `svt-core` as a Rust crate, they must be compiled against a compatible version. This is the same constraint as any Rust library -- semver compatibility of `svt-core` determines plugin compatibility.

## What This Does NOT Include

- No WASM plugin support (future, if needed)
- No plugin configuration files or settings
- No plugin dependency resolution
- No plugin hot-reloading
- No REST API for plugin management
- No plugin sandboxing (plugins run in-process with full trust)

## Testing

- `mock_plugin_registers_contributions` -- create a mock `SvtPlugin` impl, verify it registers into all three registries
- `load_test_plugin_from_cdylib` -- compile a test plugin as cdylib in a build script, load it at runtime
- `api_version_mismatch_skips_plugin` -- verify mismatched API version produces warning, not error
- `missing_symbol_skips_gracefully` -- load a non-plugin .so (e.g., libc), verify it's skipped
- `scan_directory_finds_plugins` -- create a temp dir with plugin files, verify discovery
- `svt_plugin_list_shows_loaded` -- integration test for CLI output
- `plugin_constraint_evaluator_used_in_check` -- end-to-end: plugin adds a constraint evaluator, `svt check` uses it
