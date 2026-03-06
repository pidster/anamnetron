//! Plugin API for extending svt with custom constraint evaluators and export formats.
//!
//! Plugin authors depend on `svt-core` and implement the [`SvtPlugin`] trait.
//! The [`declare_plugin!`] macro generates the required `extern "C"` entry point
//! for dynamic loading.

use crate::analysis::{LanguageDescriptor, LanguageParser};
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
/// additional [`ConstraintEvaluator`]s, [`ExportFormat`]s, and
/// [`LanguageParser`]s to the host application.
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

    /// Language parsers contributed by this plugin.
    ///
    /// Each entry pairs a [`LanguageDescriptor`] (discovery configuration) with
    /// a [`LanguageParser`] (source code parser). The host uses the descriptor
    /// to find project units and the parser to extract structure.
    ///
    /// Returns an empty vec by default.
    fn language_parsers(&self) -> Vec<(LanguageDescriptor, Box<dyn LanguageParser>)> {
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
/// #[derive(Default)]
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
        #[allow(improper_ctypes_definitions)]
        pub extern "C" fn svt_plugin_create() -> *mut dyn $crate::plugin::SvtPlugin {
            let plugin: Box<dyn $crate::plugin::SvtPlugin> = Box::new(<$plugin_type>::default());
            Box::into_raw(plugin)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conformance::{
        ConstraintEvaluator, ConstraintRegistry, ConstraintResult, ConstraintStatus,
    };
    use crate::export::{ExportFormat, ExportRegistry};
    use crate::model::{Constraint, Severity, Version};
    use crate::store::{GraphStore, Result as StoreResult};

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
    fn mock_plugin_default_language_parsers_is_empty() {
        let plugin = MockPlugin;
        assert!(
            plugin.language_parsers().is_empty(),
            "default language_parsers() should return an empty vec"
        );
    }

    #[test]
    fn api_version_constant_is_one() {
        assert_eq!(SVT_PLUGIN_API_VERSION, 1);
    }

    /// A mock constraint evaluator that always passes.
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

    /// A plugin that provides real contributions (constraint evaluator + export format).
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
        assert_eq!(evaluators.len(), 1, "should provide exactly one evaluator");
        assert_eq!(evaluators[0].kind(), "mock_constraint");
    }

    #[test]
    fn contributing_plugin_provides_format() {
        let plugin = ContributingPlugin;
        let formats = plugin.export_formats();
        assert_eq!(formats.len(), 1, "should provide exactly one format");
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
            "mock_constraint should be registered in the constraint registry"
        );
        assert!(
            exports.get("mock").is_some(),
            "mock format should be registered in the export registry"
        );
    }

    /// A plugin that overrides language_parsers with a mock.
    struct LanguagePlugin;

    impl SvtPlugin for LanguagePlugin {
        fn name(&self) -> &str {
            "language-plugin"
        }

        fn version(&self) -> &str {
            "0.3.0"
        }

        fn api_version(&self) -> u32 {
            SVT_PLUGIN_API_VERSION
        }

        fn language_parsers(&self) -> Vec<(LanguageDescriptor, Box<dyn LanguageParser>)> {
            use crate::model::NodeKind;
            vec![(
                LanguageDescriptor {
                    language_id: "mock-lang".to_string(),
                    manifest_files: vec!["mock.toml".to_string()],
                    source_extensions: vec![".mock".to_string()],
                    skip_directories: vec![],
                    top_level_kind: NodeKind::Component,
                    top_level_sub_kind: "mock".to_string(),
                },
                Box::new(MockParser),
            )]
        }
    }

    /// Minimal mock LanguageParser that does nothing.
    struct MockParser;

    impl LanguageParser for MockParser {
        fn parse(
            &self,
            _unit_name: &str,
            _files: &[&std::path::Path],
        ) -> crate::analysis::ParseResult {
            crate::analysis::ParseResult {
                items: vec![],
                relations: vec![],
                warnings: vec![],
            }
        }
    }

    #[test]
    fn language_plugin_provides_parsers() {
        let plugin = LanguagePlugin;
        let parsers = plugin.language_parsers();
        assert_eq!(parsers.len(), 1, "should provide exactly one parser");
        assert_eq!(parsers[0].0.language_id, "mock-lang");
        assert_eq!(parsers[0].0.source_extensions, vec![".mock"]);
    }

    #[test]
    fn contributing_plugin_language_parsers_default_is_empty() {
        // ContributingPlugin overrides evaluators and formats but NOT language_parsers
        let plugin = ContributingPlugin;
        assert!(
            plugin.language_parsers().is_empty(),
            "ContributingPlugin should have empty language_parsers by default"
        );
    }

    /// Test the declare_plugin! macro compiles and produces a valid function pointer.
    #[derive(Default)]
    struct MacroTestPlugin;

    impl SvtPlugin for MacroTestPlugin {
        fn name(&self) -> &str {
            "macro-test"
        }
        fn version(&self) -> &str {
            "0.0.1"
        }
        fn api_version(&self) -> u32 {
            SVT_PLUGIN_API_VERSION
        }
    }

    // Expand the macro in test context to verify it compiles.
    crate::declare_plugin!(MacroTestPlugin);

    #[test]
    fn declare_plugin_macro_creates_valid_plugin() {
        // Safety: we own the pointer and immediately convert it back
        let raw = svt_plugin_create();
        assert!(!raw.is_null(), "macro should produce a non-null pointer");
        // Convert back to Box to test and free
        let plugin: Box<dyn SvtPlugin> = unsafe { Box::from_raw(raw) };
        assert_eq!(plugin.name(), "macro-test");
        assert_eq!(plugin.version(), "0.0.1");
        assert_eq!(plugin.api_version(), SVT_PLUGIN_API_VERSION);
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
