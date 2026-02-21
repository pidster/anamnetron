//! Plugin manifest format (`svt-plugin.toml`) parsing and validation.
//!
//! The manifest describes a plugin's metadata (name, version, API version) and
//! its contributions (constraint kinds, export formats, language IDs). It is used
//! by `svt plugin install` and `svt plugin info` commands to manage plugins.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::plugin::shared_library_extension;

/// Top-level plugin manifest, parsed from `svt-plugin.toml`.
///
/// # Example
///
/// ```toml
/// [plugin]
/// name = "svt-plugin-java"
/// version = "0.1.0"
/// description = "Java language analyzer for SVT"
/// authors = ["Jane Doe <jane@example.com>"]
/// license = "MIT"
/// api_version = 1
/// library = "svt_plugin_java"
///
/// [contributions]
/// constraint_kinds = []
/// export_formats = []
/// language_ids = ["java"]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin metadata.
    pub plugin: PluginMetadata,
    /// Plugin contributions.
    #[serde(default)]
    pub contributions: PluginContributions,
}

/// Metadata about a plugin: name, version, description, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Plugin name (e.g. `"svt-plugin-java"`).
    pub name: String,
    /// Semantic version string (e.g. `"0.1.0"`).
    pub version: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: String,
    /// List of authors (e.g. `["Jane Doe <jane@example.com>"]`).
    #[serde(default)]
    pub authors: Vec<String>,
    /// License identifier (e.g. `"MIT"`).
    #[serde(default)]
    pub license: String,
    /// Plugin API version. Must match [`svt_core::plugin::SVT_PLUGIN_API_VERSION`].
    pub api_version: u32,
    /// Library stem name. If omitted, derived from the plugin name by replacing
    /// hyphens with underscores.
    pub library: Option<String>,
}

/// Describes what a plugin contributes to the host.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginContributions {
    /// Constraint evaluator kind strings contributed by this plugin.
    #[serde(default)]
    pub constraint_kinds: Vec<String>,
    /// Export format names contributed by this plugin.
    #[serde(default)]
    pub export_formats: Vec<String>,
    /// Language IDs contributed by this plugin.
    #[serde(default)]
    pub language_ids: Vec<String>,
}

/// Errors that can occur when reading, parsing, or validating a plugin manifest.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    /// Failed to read the manifest file from disk.
    #[error("failed to read manifest at '{path}': {reason}")]
    Io {
        /// Path that was attempted.
        path: String,
        /// Reason for the failure.
        reason: String,
    },
    /// Failed to parse the TOML content into a manifest.
    #[error("failed to parse manifest: {0}")]
    Parse(String),
    /// Manifest parsed successfully but has invalid field values.
    #[error("invalid manifest: {0}")]
    Validation(String),
    /// Failed to serialize a manifest back to TOML.
    #[error("failed to serialize manifest: {0}")]
    Serialize(String),
}

/// Parse a TOML string into a [`PluginManifest`].
///
/// # Errors
///
/// Returns [`ManifestError::Parse`] if the TOML is malformed or missing
/// required fields.
pub fn parse_manifest(content: &str) -> Result<PluginManifest, ManifestError> {
    toml::from_str(content).map_err(|e| ManifestError::Parse(e.to_string()))
}

/// Read and parse a manifest file from `path`.
///
/// # Errors
///
/// Returns [`ManifestError::Io`] if the file cannot be read, or
/// [`ManifestError::Parse`] if the content is not valid TOML.
pub fn read_manifest(path: &Path) -> Result<PluginManifest, ManifestError> {
    let content = std::fs::read_to_string(path).map_err(|e| ManifestError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    parse_manifest(&content)
}

/// Serialize a [`PluginManifest`] back to TOML.
///
/// # Errors
///
/// Returns [`ManifestError::Serialize`] if serialization fails.
pub fn serialize_manifest(manifest: &PluginManifest) -> Result<String, ManifestError> {
    toml::to_string_pretty(manifest).map_err(|e| ManifestError::Serialize(e.to_string()))
}

/// Validate that a parsed manifest has sensible field values.
///
/// Checks:
/// - `name` is non-empty
/// - `api_version` is non-zero
///
/// # Errors
///
/// Returns [`ManifestError::Validation`] with a description of the first
/// validation error found.
pub fn validate_manifest(manifest: &PluginManifest) -> Result<(), ManifestError> {
    if manifest.plugin.name.trim().is_empty() {
        return Err(ManifestError::Validation(
            "plugin name must not be empty".to_string(),
        ));
    }
    if manifest.plugin.api_version == 0 {
        return Err(ManifestError::Validation(
            "api_version must be greater than 0".to_string(),
        ));
    }
    Ok(())
}

/// Derive the platform-appropriate library filename from a manifest.
///
/// Uses `manifest.plugin.library` if set, otherwise derives from the plugin name
/// by replacing hyphens with underscores.
///
/// Returns a filename like `libsvt_plugin_java.dylib` (macOS),
/// `libsvt_plugin_java.so` (Linux), or `svt_plugin_java.dll` (Windows).
#[must_use]
pub fn library_filename(manifest: &PluginManifest) -> String {
    let stem = manifest
        .plugin
        .library
        .as_deref()
        .map(|s| s.to_string())
        .unwrap_or_else(|| manifest.plugin.name.replace('-', "_"));

    let ext = shared_library_extension();

    if cfg!(target_os = "windows") {
        format!("{stem}.{ext}")
    } else {
        format!("lib{stem}.{ext}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_manifest_succeeds() {
        let toml = r#"
[plugin]
name = "svt-plugin-java"
version = "0.1.0"
api_version = 1
"#;
        let manifest = parse_manifest(toml).expect("should parse minimal manifest");
        assert_eq!(manifest.plugin.name, "svt-plugin-java");
        assert_eq!(manifest.plugin.version, "0.1.0");
        assert_eq!(manifest.plugin.api_version, 1);
        assert!(manifest.plugin.library.is_none());
        assert!(manifest.plugin.description.is_empty());
        assert!(manifest.plugin.authors.is_empty());
        assert!(manifest.plugin.license.is_empty());
    }

    #[test]
    fn parse_full_manifest_with_all_fields() {
        let toml = r#"
[plugin]
name = "svt-plugin-java"
version = "0.1.0"
description = "Java language analyzer for SVT"
authors = ["Jane Doe <jane@example.com>"]
license = "MIT"
api_version = 1
library = "svt_plugin_java"

[contributions]
constraint_kinds = []
export_formats = []
language_ids = ["java"]
"#;
        let manifest = parse_manifest(toml).expect("should parse full manifest");
        assert_eq!(manifest.plugin.name, "svt-plugin-java");
        assert_eq!(
            manifest.plugin.description,
            "Java language analyzer for SVT"
        );
        assert_eq!(manifest.plugin.authors, vec!["Jane Doe <jane@example.com>"]);
        assert_eq!(manifest.plugin.license, "MIT");
        assert_eq!(manifest.plugin.library.as_deref(), Some("svt_plugin_java"));
        assert_eq!(manifest.contributions.language_ids, vec!["java"]);
        assert!(manifest.contributions.constraint_kinds.is_empty());
        assert!(manifest.contributions.export_formats.is_empty());
    }

    #[test]
    fn parse_manifest_missing_name_fails() {
        let toml = r#"
[plugin]
version = "0.1.0"
api_version = 1
"#;
        let err = parse_manifest(toml).unwrap_err();
        match err {
            ManifestError::Parse(msg) => {
                assert!(
                    msg.contains("name"),
                    "error should mention missing field 'name', got: {msg}"
                );
            }
            other => panic!("expected Parse, got: {other:?}"),
        }
    }

    #[test]
    fn parse_manifest_missing_api_version_fails() {
        let toml = r#"
[plugin]
name = "svt-plugin-java"
version = "0.1.0"
"#;
        let err = parse_manifest(toml).unwrap_err();
        match err {
            ManifestError::Parse(msg) => {
                assert!(
                    msg.contains("api_version"),
                    "error should mention missing field 'api_version', got: {msg}"
                );
            }
            other => panic!("expected Parse, got: {other:?}"),
        }
    }

    #[test]
    fn validate_manifest_rejects_empty_name() {
        let manifest = PluginManifest {
            plugin: PluginMetadata {
                name: "".to_string(),
                version: "0.1.0".to_string(),
                description: String::new(),
                authors: vec![],
                license: String::new(),
                api_version: 1,
                library: None,
            },
            contributions: PluginContributions::default(),
        };
        let err = validate_manifest(&manifest).unwrap_err();
        match err {
            ManifestError::Validation(msg) => {
                assert!(
                    msg.contains("name"),
                    "error should mention name, got: {msg}"
                );
            }
            other => panic!("expected Validation, got: {other:?}"),
        }
    }

    #[test]
    fn validate_manifest_rejects_zero_api_version() {
        let manifest = PluginManifest {
            plugin: PluginMetadata {
                name: "test-plugin".to_string(),
                version: "0.1.0".to_string(),
                description: String::new(),
                authors: vec![],
                license: String::new(),
                api_version: 0,
                library: None,
            },
            contributions: PluginContributions::default(),
        };
        let err = validate_manifest(&manifest).unwrap_err();
        match err {
            ManifestError::Validation(msg) => {
                assert!(
                    msg.contains("api_version"),
                    "error should mention api_version, got: {msg}"
                );
            }
            other => panic!("expected Validation, got: {other:?}"),
        }
    }

    #[test]
    fn library_filename_derives_from_name_on_current_platform() {
        let manifest = PluginManifest {
            plugin: PluginMetadata {
                name: "svt-plugin-java".to_string(),
                version: "0.1.0".to_string(),
                description: String::new(),
                authors: vec![],
                license: String::new(),
                api_version: 1,
                library: None,
            },
            contributions: PluginContributions::default(),
        };
        let filename = library_filename(&manifest);
        if cfg!(target_os = "macos") {
            assert_eq!(filename, "libsvt_plugin_java.dylib");
        } else if cfg!(target_os = "windows") {
            assert_eq!(filename, "svt_plugin_java.dll");
        } else {
            assert_eq!(filename, "libsvt_plugin_java.so");
        }
    }

    #[test]
    fn library_filename_uses_explicit_library_field() {
        let manifest = PluginManifest {
            plugin: PluginMetadata {
                name: "svt-plugin-java".to_string(),
                version: "0.1.0".to_string(),
                description: String::new(),
                authors: vec![],
                license: String::new(),
                api_version: 1,
                library: Some("custom_lib_name".to_string()),
            },
            contributions: PluginContributions::default(),
        };
        let filename = library_filename(&manifest);
        if cfg!(target_os = "macos") {
            assert_eq!(filename, "libcustom_lib_name.dylib");
        } else if cfg!(target_os = "windows") {
            assert_eq!(filename, "custom_lib_name.dll");
        } else {
            assert_eq!(filename, "libcustom_lib_name.so");
        }
    }

    #[test]
    fn serialize_manifest_round_trips() {
        let original = PluginManifest {
            plugin: PluginMetadata {
                name: "svt-plugin-java".to_string(),
                version: "0.1.0".to_string(),
                description: "Java analyzer".to_string(),
                authors: vec!["Alice <alice@example.com>".to_string()],
                license: "MIT".to_string(),
                api_version: 1,
                library: Some("svt_plugin_java".to_string()),
            },
            contributions: PluginContributions {
                constraint_kinds: vec![],
                export_formats: vec![],
                language_ids: vec!["java".to_string()],
            },
        };
        let serialized = serialize_manifest(&original).expect("should serialize");
        let parsed = parse_manifest(&serialized).expect("should parse round-tripped manifest");
        assert_eq!(parsed.plugin.name, original.plugin.name);
        assert_eq!(parsed.plugin.version, original.plugin.version);
        assert_eq!(parsed.plugin.description, original.plugin.description);
        assert_eq!(parsed.plugin.authors, original.plugin.authors);
        assert_eq!(parsed.plugin.license, original.plugin.license);
        assert_eq!(parsed.plugin.api_version, original.plugin.api_version);
        assert_eq!(parsed.plugin.library, original.plugin.library);
        assert_eq!(
            parsed.contributions.language_ids,
            original.contributions.language_ids
        );
    }

    #[test]
    fn contributions_default_to_empty_vecs() {
        let toml = r#"
[plugin]
name = "minimal"
version = "0.1.0"
api_version = 1
"#;
        let manifest = parse_manifest(toml).expect("should parse");
        assert!(manifest.contributions.constraint_kinds.is_empty());
        assert!(manifest.contributions.export_formats.is_empty());
        assert!(manifest.contributions.language_ids.is_empty());
    }

    #[test]
    fn read_manifest_from_file() {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let path = dir.path().join("svt-plugin.toml");
        let content = r#"
[plugin]
name = "test-plugin"
version = "1.0.0"
api_version = 1
"#;
        std::fs::write(&path, content).expect("failed to write manifest");
        let manifest = read_manifest(&path).expect("should read manifest from file");
        assert_eq!(manifest.plugin.name, "test-plugin");
        assert_eq!(manifest.plugin.version, "1.0.0");
    }

    #[test]
    fn read_manifest_nonexistent_file_fails() {
        let err = read_manifest(Path::new("/nonexistent/svt-plugin.toml")).unwrap_err();
        match err {
            ManifestError::Io { path, .. } => {
                assert!(
                    path.contains("nonexistent"),
                    "error should contain the path, got: {path}"
                );
            }
            other => panic!("expected Io, got: {other:?}"),
        }
    }
}
