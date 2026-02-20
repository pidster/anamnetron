//! Plugin API for extending svt with custom constraint evaluators and export formats.
//!
//! Plugin authors depend on `svt-core` and implement the [`SvtPlugin`] trait.
//! The [`declare_plugin!`] macro generates the required `extern "C"` entry point
//! for dynamic loading.

use crate::conformance::ConstraintEvaluator;
use crate::export::ExportFormat;

/// Current plugin API version. Plugins must return this from
/// [`SvtPlugin::api_version`] to be loaded successfully.
pub const SVT_PLUGIN_API_VERSION: u32 = 1;

/// Errors that can occur during plugin loading.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// Failed to load the shared library at the given path.
    #[error("failed to load plugin at '{path}': {reason}")]
    LoadFailed {
        /// Filesystem path of the plugin library.
        path: String,
        /// Human-readable reason for the failure.
        reason: String,
    },
    /// The required `svt_plugin_create` symbol was not found in the library.
    #[error("symbol 'svt_plugin_create' not found in '{path}'")]
    SymbolNotFound {
        /// Filesystem path of the plugin library.
        path: String,
    },
    /// The plugin reports an API version that does not match the host.
    #[error("API version mismatch for plugin '{plugin_name}': expected {expected}, got {actual}")]
    ApiVersionMismatch {
        /// Name reported by the plugin.
        plugin_name: String,
        /// API version the host expects.
        expected: u32,
        /// API version the plugin reported.
        actual: u32,
    },
}

/// Trait implemented by svt plugins.
///
/// A plugin provides metadata (name, version, API version) and may contribute
/// additional [`ConstraintEvaluator`]s and [`ExportFormat`]s to the host
/// application.
///
/// # Safety
///
/// Plugins are loaded as shared libraries. The [`declare_plugin!`] macro
/// generates the required `extern "C"` entry point.
pub trait SvtPlugin: Send + Sync {
    /// Human-readable name of the plugin.
    fn name(&self) -> &str;

    /// Semantic version string of the plugin (e.g. `"0.1.0"`).
    fn version(&self) -> &str;

    /// Plugin API version. Must match [`SVT_PLUGIN_API_VERSION`] for the
    /// plugin to be accepted by the host.
    fn api_version(&self) -> u32;

    /// Constraint evaluators contributed by this plugin.
    ///
    /// Returns an empty vec by default.
    fn constraint_evaluators(&self) -> Vec<Box<dyn ConstraintEvaluator>> {
        Vec::new()
    }

    /// Export formats contributed by this plugin.
    ///
    /// Returns an empty vec by default.
    fn export_formats(&self) -> Vec<Box<dyn ExportFormat>> {
        Vec::new()
    }
}

/// Declare a struct as an svt plugin.
///
/// The plugin type must implement [`Default`] and [`SvtPlugin`].
///
/// Generates an `extern "C"` function named `svt_plugin_create` that
/// constructs the plugin and returns a raw pointer to it as a trait object.
///
/// # Example
///
/// ```ignore
/// use svt_core::plugin::{SvtPlugin, SVT_PLUGIN_API_VERSION};
///
/// struct MyPlugin;
///
/// impl SvtPlugin for MyPlugin {
///     fn name(&self) -> &str { "my-plugin" }
///     fn version(&self) -> &str { "0.1.0" }
///     fn api_version(&self) -> u32 { SVT_PLUGIN_API_VERSION }
/// }
///
/// svt_core::declare_plugin!(MyPlugin);
/// ```
#[macro_export]
macro_rules! declare_plugin {
    ($plugin_type:ty) => {
        /// Entry point called by the svt plugin loader.
        ///
        /// # Safety
        ///
        /// The caller must ensure the returned pointer is freed by converting
        /// it back into a `Box<dyn SvtPlugin>`.
        #[no_mangle]
        pub extern "C" fn svt_plugin_create() -> *mut dyn $crate::plugin::SvtPlugin {
            let plugin: Box<dyn $crate::plugin::SvtPlugin> = Box::new(<$plugin_type>::default());
            Box::into_raw(plugin)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mock plugin for testing the SvtPlugin trait.
    struct MockPlugin;

    impl SvtPlugin for MockPlugin {
        fn name(&self) -> &str {
            "mock-plugin"
        }

        fn version(&self) -> &str {
            "1.2.3"
        }

        fn api_version(&self) -> u32 {
            SVT_PLUGIN_API_VERSION
        }
    }

    #[test]
    fn mock_plugin_has_correct_metadata() {
        let plugin = MockPlugin;
        assert_eq!(plugin.name(), "mock-plugin");
        assert_eq!(plugin.version(), "1.2.3");
        assert_eq!(plugin.api_version(), SVT_PLUGIN_API_VERSION);
    }

    #[test]
    fn mock_plugin_default_contributions_are_empty() {
        let plugin = MockPlugin;
        assert!(
            plugin.constraint_evaluators().is_empty(),
            "default constraint_evaluators() should return an empty vec"
        );
        assert!(
            plugin.export_formats().is_empty(),
            "default export_formats() should return an empty vec"
        );
    }

    #[test]
    fn api_version_constant_is_one() {
        assert_eq!(SVT_PLUGIN_API_VERSION, 1);
    }

    #[test]
    fn plugin_error_display_formats_correctly() {
        let load_err = PluginError::LoadFailed {
            path: "/tmp/libfoo.so".to_string(),
            reason: "file not found".to_string(),
        };
        let display = format!("{load_err}");
        assert!(
            display.contains("/tmp/libfoo.so"),
            "LoadFailed display should contain the path, got: {display}"
        );
        assert!(
            display.contains("file not found"),
            "LoadFailed display should contain the reason, got: {display}"
        );

        let sym_err = PluginError::SymbolNotFound {
            path: "/tmp/libbar.so".to_string(),
        };
        let display = format!("{sym_err}");
        assert!(
            display.contains("/tmp/libbar.so"),
            "SymbolNotFound display should contain the path, got: {display}"
        );
        assert!(
            display.contains("svt_plugin_create"),
            "SymbolNotFound display should mention the symbol name, got: {display}"
        );

        let ver_err = PluginError::ApiVersionMismatch {
            plugin_name: "test-plugin".to_string(),
            expected: 1,
            actual: 99,
        };
        let display = format!("{ver_err}");
        assert!(
            display.contains("test-plugin"),
            "ApiVersionMismatch display should contain the plugin name, got: {display}"
        );
        assert!(
            display.contains("1"),
            "ApiVersionMismatch display should contain expected version, got: {display}"
        );
        assert!(
            display.contains("99"),
            "ApiVersionMismatch display should contain actual version, got: {display}"
        );
    }
}
