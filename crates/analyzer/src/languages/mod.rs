//! Language-specific analysis drivers.

pub mod go;
pub mod python;
pub mod rust;
pub mod svelte;
pub mod typescript;

use std::path::Path;

// Re-export ParseResult from svt-core.
pub use svt_core::analysis::ParseResult;

/// A language-specific source code analyzer.
pub trait LanguageAnalyzer: Send + Sync {
    /// Unique identifier for this language (e.g., "rust", "typescript").
    fn language_id(&self) -> &str;

    /// Parse a set of source files for a crate and return extracted items and relations.
    ///
    /// `crate_name` is the Rust crate name (e.g., "svt_core").
    /// `files` are the `.rs` source files to parse.
    fn analyze_crate(&self, crate_name: &str, files: &[&Path]) -> ParseResult;
}

/// Registry of language analyzers, keyed by language ID.
pub struct AnalyzerRegistry {
    analyzers: std::collections::HashMap<String, Box<dyn LanguageAnalyzer>>,
}

impl AnalyzerRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            analyzers: std::collections::HashMap::new(),
        }
    }

    /// Create a registry with all built-in analyzers pre-registered.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(rust::RustAnalyzer::new()));
        registry.register(Box::new(typescript::TypeScriptAnalyzer::new()));
        registry.register(Box::new(go::GoAnalyzer::new()));
        registry.register(Box::new(python::PythonAnalyzer::new()));
        registry
    }

    /// Register a language analyzer.
    pub fn register(&mut self, analyzer: Box<dyn LanguageAnalyzer>) {
        self.analyzers
            .insert(analyzer.language_id().to_string(), analyzer);
    }

    /// Look up an analyzer by language ID.
    #[must_use]
    pub fn get(&self, language_id: &str) -> Option<&dyn LanguageAnalyzer> {
        self.analyzers.get(language_id).map(|a| a.as_ref())
    }

    /// List all registered language IDs.
    #[must_use]
    pub fn language_ids(&self) -> Vec<&str> {
        self.analyzers.keys().map(|k| k.as_str()).collect()
    }
}

impl Default for AnalyzerRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_analyzer_has_correct_language_id() {
        let analyzer = rust::RustAnalyzer::new();
        assert_eq!(analyzer.language_id(), "rust");
    }

    #[test]
    fn typescript_analyzer_has_correct_language_id() {
        let analyzer = typescript::TypeScriptAnalyzer::new();
        assert_eq!(analyzer.language_id(), "typescript");
    }

    #[test]
    fn go_analyzer_has_correct_language_id() {
        let analyzer = go::GoAnalyzer::new();
        assert_eq!(analyzer.language_id(), "go");
    }

    #[test]
    fn python_analyzer_has_correct_language_id() {
        let analyzer = python::PythonAnalyzer::new();
        assert_eq!(analyzer.language_id(), "python");
    }

    #[test]
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

    #[test]
    fn analyzer_registry_register_adds_analyzer() {
        let mut registry = AnalyzerRegistry::new();
        assert!(registry.get("rust").is_none());
        registry.register(Box::new(rust::RustAnalyzer::new()));
        assert!(registry.get("rust").is_some());
    }
}
