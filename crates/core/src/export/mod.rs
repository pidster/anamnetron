//! Export graph data in various formats.

pub mod dot;
pub mod mermaid;
pub mod svg;

use crate::model::{Version, DEFAULT_PROJECT_ID};
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
        crate::interchange_store::export_json(store, DEFAULT_PROJECT_ID, version)
    }
}

/// Built-in DOT (Graphviz) exporter.
#[derive(Debug)]
pub struct DotExporter;

impl ExportFormat for DotExporter {
    fn name(&self) -> &str {
        "dot"
    }
    fn export(&self, store: &dyn GraphStore, version: Version) -> Result<String> {
        dot::to_dot(store, version)
    }
}

/// SVG exporter via Graphviz `dot` command.
#[derive(Debug)]
pub struct SvgExporter;

impl ExportFormat for SvgExporter {
    fn name(&self) -> &str {
        "svg"
    }
    fn export(&self, store: &dyn GraphStore, version: Version) -> Result<String> {
        svg::to_svg(store, version)
    }
}

/// PNG exporter via Graphviz `dot` command.
///
/// Note: PNG is binary. Use [`svg::to_png_bytes`] for raw binary output.
/// The `export()` method returns an error directing to use `--output` flag.
#[derive(Debug)]
pub struct PngExporter;

impl ExportFormat for PngExporter {
    fn name(&self) -> &str {
        "png"
    }
    fn export(&self, _store: &dyn GraphStore, _version: Version) -> Result<String> {
        Err(crate::store::StoreError::Internal(
            "PNG is a binary format. Use `svt export --format png --output FILE`".to_string(),
        ))
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
        registry.register(Box::new(DotExporter));
        registry.register(Box::new(SvgExporter));
        registry.register(Box::new(PngExporter));
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
        let dot = DotExporter;
        assert_eq!(dot.name(), "dot");
        let svg = SvgExporter;
        assert_eq!(svg.name(), "svg");
        let png = PngExporter;
        assert_eq!(png.name(), "png");
    }

    #[test]
    fn export_registry_with_defaults_has_all_built_ins() {
        let registry = ExportRegistry::with_defaults();
        assert!(registry.get("mermaid").is_some());
        assert!(registry.get("json").is_some());
        assert!(registry.get("dot").is_some());
        assert!(registry.get("svg").is_some());
        assert!(registry.get("png").is_some());
        assert!(registry.get("unknown").is_none());
        let mut names = registry.names();
        names.sort();
        assert_eq!(names, vec!["dot", "json", "mermaid", "png", "svg"]);
    }

    #[test]
    fn export_registry_with_defaults_includes_svg() {
        let registry = ExportRegistry::with_defaults();
        assert!(
            registry.get("svg").is_some(),
            "svg format should be registered"
        );
    }

    #[test]
    fn export_registry_with_defaults_includes_png() {
        let registry = ExportRegistry::with_defaults();
        assert!(
            registry.get("png").is_some(),
            "png format should be registered"
        );
    }
}
