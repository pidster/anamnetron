//! Dynamic plugin loading for svt.
//!
//! Loads shared libraries (.dylib/.so/.dll) that implement the [`SvtPlugin`] trait
//! via the `svt_plugin_create` entry point. This module lives in `svt-cli` rather
//! than `svt-core` because `libloading` is platform-specific and core must remain
//! WASM-compatible.

use std::path::Path;

use svt_core::conformance::ConstraintRegistry;
use svt_core::export::ExportRegistry;
use svt_core::plugin::{PluginError, SvtPlugin, SVT_PLUGIN_API_VERSION};

/// Type of the `svt_plugin_create` entry point exported by plugin shared libraries.
///
/// This intentionally uses `dyn SvtPlugin` across the FFI boundary — the pointer
/// is produced by `Box::into_raw` in `declare_plugin!` and consumed by
/// `Box::from_raw` in the loader. This is safe only when the plugin and host are
/// compiled with the same Rust compiler version against the same `svt-core` crate
/// version, so that the vtable layout of `dyn SvtPlugin` is identical on both sides.
#[allow(improper_ctypes_definitions)]
type PluginCreateFn = unsafe extern "C" fn() -> *mut dyn SvtPlugin;

/// Loads svt plugins from shared libraries at runtime.
///
/// The loader discovers `.dylib`/`.so`/`.dll` files, opens them with `libloading`,
/// looks up the `svt_plugin_create` symbol, and calls it to obtain plugin instances.
/// Both the [`libloading::Library`] handle and the [`Box<dyn SvtPlugin>`] are kept
/// alive for the lifetime of the loader.
///
/// Library handles and plugin instances are stored in parallel vecs so that
/// [`plugins()`](PluginLoader::plugins) can return a contiguous `&[Box<dyn SvtPlugin>]`.
pub struct PluginLoader {
    /// Library handles kept alive so function pointers and vtables remain valid.
    /// Order matches `plugins` — `libraries[i]` is the library for `plugins[i]`.
    libraries: Vec<libloading::Library>,
    /// Plugin instances obtained from the libraries' entry points.
    plugins: Vec<Box<dyn SvtPlugin>>,
}

impl PluginLoader {
    /// Create an empty plugin loader with no plugins loaded.
    #[must_use]
    pub fn new() -> Self {
        Self {
            libraries: Vec::new(),
            plugins: Vec::new(),
        }
    }

    /// Load a single plugin from a shared library at `path`.
    ///
    /// This function:
    /// 1. Opens the library with [`libloading::Library::new`].
    /// 2. Looks up the `svt_plugin_create` symbol.
    /// 3. Calls the symbol to obtain a `*mut dyn SvtPlugin`, wraps it in a `Box`.
    /// 4. Verifies that `api_version()` matches [`SVT_PLUGIN_API_VERSION`].
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::LoadFailed`] if the library cannot be opened,
    /// [`PluginError::SymbolNotFound`] if the entry point is missing, or
    /// [`PluginError::ApiVersionMismatch`] if the versions disagree.
    pub fn load(&mut self, path: &Path) -> Result<(), PluginError> {
        let path_str = path.display().to_string();

        // SAFETY: `Library::new` loads a shared library into the process. This is
        // inherently unsafe because the library may contain arbitrary code that runs
        // on load (e.g. constructors). We trust that plugin authors provide well-
        // behaved libraries.
        let library =
            unsafe { libloading::Library::new(path) }.map_err(|e| PluginError::LoadFailed {
                path: path_str.clone(),
                reason: e.to_string(),
            })?;

        // SAFETY: `library.get` looks up a symbol by name. The symbol may not exist
        // or may have a different type than expected. We verify the symbol name
        // matches the convention established by `declare_plugin!`.
        let create_fn: libloading::Symbol<'_, PluginCreateFn> = unsafe {
            library.get(b"svt_plugin_create")
        }
        .map_err(|_| PluginError::SymbolNotFound {
            path: path_str.clone(),
        })?;

        // SAFETY: We call the entry point which returns a raw pointer allocated by
        // `Box::into_raw` in `declare_plugin!`. We convert it back into a Box to
        // reclaim ownership. We check for null before calling `Box::from_raw` to
        // defend against misbehaving plugins that don't use `declare_plugin!`.
        let plugin: Box<dyn SvtPlugin> = unsafe {
            let raw = create_fn();
            if raw.is_null() {
                return Err(PluginError::LoadFailed {
                    path: path_str,
                    reason: "svt_plugin_create returned null".to_string(),
                });
            }
            Box::from_raw(raw)
        };

        let actual_version = plugin.api_version();
        if actual_version != SVT_PLUGIN_API_VERSION {
            return Err(PluginError::ApiVersionMismatch {
                plugin_name: plugin.name().to_string(),
                expected: SVT_PLUGIN_API_VERSION,
                actual: actual_version,
            });
        }

        self.libraries.push(library);
        self.plugins.push(plugin);

        Ok(())
    }

    /// Scan a directory for shared library files and attempt to load each one.
    ///
    /// Files are matched by platform-appropriate extension (`.dylib` on macOS,
    /// `.dll` on Windows, `.so` on Linux). Returns a vec of errors for any
    /// libraries that failed to load; successfully loaded plugins are added to
    /// the loader.
    ///
    /// Returns an empty vec if the directory does not exist or contains no
    /// matching files.
    pub fn scan_directory(&mut self, dir: &Path) -> Vec<PluginError> {
        let mut errors = Vec::new();

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return errors,
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

    /// Register all loaded plugins' contributions into the provided registries.
    ///
    /// Each plugin's [`SvtPlugin::constraint_evaluators`] are added to the
    /// constraint registry, and [`SvtPlugin::export_formats`] are added to the
    /// export registry.
    pub fn register_all(&self, constraints: &mut ConstraintRegistry, exports: &mut ExportRegistry) {
        for plugin in &self.plugins {
            for evaluator in plugin.constraint_evaluators() {
                constraints.register(evaluator);
            }
            for format in plugin.export_formats() {
                exports.register(format);
            }
        }
    }

    /// Return a slice of all loaded plugin instances.
    #[must_use]
    pub fn plugins(&self) -> &[Box<dyn SvtPlugin>] {
        &self.plugins
    }
}

impl Default for PluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Return the platform-appropriate shared library file extension.
///
/// - macOS: `"dylib"`
/// - Windows: `"dll"`
/// - Other (Linux, etc.): `"so"`
#[must_use]
pub fn shared_library_extension() -> &'static str {
    if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_loader_has_no_plugins() {
        let loader = PluginLoader::new();
        assert!(
            loader.plugins().is_empty(),
            "a new loader should have no plugins"
        );
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
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let mut loader = PluginLoader::new();
        let errors = loader.scan_directory(dir.path());
        assert!(
            errors.is_empty(),
            "scanning an empty directory should produce no errors"
        );
        assert!(
            loader.plugins().is_empty(),
            "no plugins should be loaded from an empty directory"
        );
    }

    #[test]
    fn scan_nonexistent_directory_returns_no_errors() {
        let mut loader = PluginLoader::new();
        let errors = loader.scan_directory(Path::new("/nonexistent/path/that/does/not/exist"));
        assert!(
            errors.is_empty(),
            "scanning a nonexistent directory should produce no errors"
        );
    }

    #[test]
    fn scan_directory_skips_non_library_files() {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        std::fs::write(dir.path().join("readme.txt"), b"hello").unwrap();
        std::fs::write(dir.path().join("config.json"), b"{}").unwrap();
        std::fs::write(dir.path().join("data.csv"), b"a,b").unwrap();

        let mut loader = PluginLoader::new();
        let errors = loader.scan_directory(dir.path());
        assert!(
            errors.is_empty(),
            "non-library files should be skipped without error"
        );
        assert!(
            loader.plugins().is_empty(),
            "no plugins should be loaded from non-library files"
        );
    }

    #[test]
    fn load_nonexistent_file_returns_load_failed() {
        let mut loader = PluginLoader::new();
        let result = loader.load(Path::new("/nonexistent/libfake.dylib"));
        assert!(result.is_err(), "loading a nonexistent file should fail");
        match result.unwrap_err() {
            PluginError::LoadFailed { path, .. } => {
                assert!(
                    path.contains("libfake.dylib"),
                    "error should contain the path"
                );
            }
            other => panic!("expected LoadFailed, got: {other:?}"),
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn load_non_plugin_library_returns_symbol_not_found() {
        let mut loader = PluginLoader::new();
        let result = loader.load(Path::new("/usr/lib/libSystem.B.dylib"));
        assert!(
            result.is_err(),
            "loading a non-plugin library should fail with SymbolNotFound"
        );
        match result.unwrap_err() {
            PluginError::SymbolNotFound { path } => {
                assert!(
                    path.contains("libSystem"),
                    "error should reference the library path, got: {path}"
                );
            }
            other => panic!("expected SymbolNotFound, got: {other:?}"),
        }
    }

    #[test]
    fn register_all_with_no_plugins_does_nothing() {
        let loader = PluginLoader::new();
        let mut constraints = ConstraintRegistry::new();
        let mut exports = ExportRegistry::new();

        loader.register_all(&mut constraints, &mut exports);

        assert!(
            constraints.kinds().is_empty(),
            "empty loader should not add any constraint evaluators"
        );
        assert!(
            exports.names().is_empty(),
            "empty loader should not add any export formats"
        );
    }
}
