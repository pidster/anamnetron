//! Export graph data in various formats.

pub mod mermaid;

use crate::model::Version;
use crate::store::{GraphStore, Result};

/// Extension point for export formats.
pub trait ExportFormat: Send + Sync {
    /// The format name (used in CLI `--format` flag).
    fn name(&self) -> &str;
    /// Export graph data for the given version as a string.
    fn export(&self, store: &dyn GraphStore, version: Version) -> Result<String>;
}

/// Built-in Mermaid flowchart exporter.
#[derive(Debug)]
pub struct MermaidExporter;

impl ExportFormat for MermaidExporter {
    fn name(&self) -> &str {
        "mermaid"
    }
    fn export(&self, store: &dyn GraphStore, version: Version) -> Result<String> {
        mermaid::to_mermaid(store, version)
    }
}

/// Built-in JSON interchange exporter.
#[derive(Debug)]
pub struct JsonExporter;

impl ExportFormat for JsonExporter {
    fn name(&self) -> &str {
        "json"
    }
    fn export(&self, store: &dyn GraphStore, version: Version) -> Result<String> {
        crate::interchange_store::export_json(store, version)
    }
}

/// Registry of export formats, keyed by format name.
pub struct ExportRegistry {
    formats: std::collections::HashMap<String, Box<dyn ExportFormat>>,
}

impl ExportRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            formats: std::collections::HashMap::new(),
        }
    }

    /// Create a registry with all built-in formats pre-registered.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(MermaidExporter));
        registry.register(Box::new(JsonExporter));
        registry
    }

    /// Register an export format.
    pub fn register(&mut self, format: Box<dyn ExportFormat>) {
        self.formats.insert(format.name().to_string(), format);
    }

    /// Look up a format by name.
    pub fn get(&self, name: &str) -> Option<&dyn ExportFormat> {
        self.formats.get(name).map(|f| f.as_ref())
    }

    /// List all registered format names.
    pub fn names(&self) -> Vec<&str> {
        self.formats.keys().map(|k| k.as_str()).collect()
    }
}

impl Default for ExportRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_format_trait_returns_correct_name() {
        let mermaid = MermaidExporter;
        assert_eq!(mermaid.name(), "mermaid");
        let json = JsonExporter;
        assert_eq!(json.name(), "json");
    }

    #[test]
    fn export_registry_with_defaults_has_all_built_ins() {
        let registry = ExportRegistry::with_defaults();
        assert!(registry.get("mermaid").is_some());
        assert!(registry.get("json").is_some());
        assert!(registry.get("unknown").is_none());
        let mut names = registry.names();
        names.sort();
        assert_eq!(names, vec!["json", "mermaid"]);
    }
}
