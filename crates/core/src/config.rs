//! Project configuration: `.svt/config.yaml` parsing, validation, and types.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::validate_project_id;

/// Project configuration, typically loaded from `.svt/config.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Project ID for multi-tenancy.
    pub project: String,
    /// Human-readable project name.
    pub name: Option<String>,
    /// Project description.
    pub description: Option<String>,
    /// Design model file paths (relative to project root).
    #[serde(default)]
    pub design: Vec<PathBuf>,
    /// Source directories to analyze.
    #[serde(default = "default_sources")]
    pub sources: Vec<SourceConfig>,
    /// Remote server configuration.
    pub server: Option<ServerConfig>,
}

/// A source directory to analyze.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    /// Source directory path (relative to project root).
    pub path: PathBuf,
    /// Additional directories to exclude from analysis.
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// Remote server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Remote server URL.
    pub url: String,
}

/// Errors during config loading or validation.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// I/O error reading the config file.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// YAML parse error.
    #[error("parse error: {0}")]
    Parse(String),

    /// Validation error.
    #[error("validation error: {0}")]
    Validation(String),
}

/// Default source configuration: analyze the project root directory.
pub fn default_sources() -> Vec<SourceConfig> {
    vec![SourceConfig {
        path: PathBuf::from("."),
        exclude: vec![],
    }]
}

impl ProjectConfig {
    /// Load a project config from a YAML file.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path)?;
        serde_yaml::from_str(&contents).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    /// Load from `.svt/config.yaml` relative to a project directory.
    ///
    /// Returns `Ok(None)` if the config file does not exist.
    pub fn load_from_project_dir(project_dir: &Path) -> Result<Option<Self>, ConfigError> {
        let config_path = project_dir.join(".svt").join("config.yaml");
        if !config_path.exists() {
            return Ok(None);
        }
        Self::load(&config_path).map(Some)
    }

    /// Validate all fields and referenced paths against a project root.
    pub fn validate(&self, project_root: &Path) -> Result<(), ConfigError> {
        // Validate project ID.
        validate_project_id(&self.project)
            .map_err(|e| ConfigError::Validation(format!("invalid project ID: {e}")))?;

        // Validate design file extensions.
        for design_path in &self.design {
            match design_path.extension().and_then(|e| e.to_str()) {
                Some("yaml" | "yml" | "json") => {}
                _ => {
                    return Err(ConfigError::Validation(format!(
                        "design file must have .yaml, .yml, or .json extension: {}",
                        design_path.display()
                    )));
                }
            }
        }

        // Validate source paths are directories within the project root.
        let canonical_root = project_root
            .canonicalize()
            .map_err(|e| ConfigError::Validation(format!("cannot resolve project root: {e}")))?;

        for source in &self.sources {
            let resolved = project_root.join(&source.path);
            if !resolved.exists() {
                return Err(ConfigError::Validation(format!(
                    "source path does not exist: {}",
                    source.path.display()
                )));
            }
            if !resolved.is_dir() {
                return Err(ConfigError::Validation(format!(
                    "source path is not a directory: {}",
                    source.path.display()
                )));
            }
            let canonical_source = resolved.canonicalize().map_err(|e| {
                ConfigError::Validation(format!(
                    "cannot resolve source path '{}': {e}",
                    source.path.display()
                ))
            })?;
            if !canonical_source.starts_with(&canonical_root) {
                return Err(ConfigError::Validation(format!(
                    "source path escapes project root: {}",
                    source.path.display()
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_config_round_trips_through_yaml() {
        let config = ProjectConfig {
            project: "my-app".to_string(),
            name: Some("My Application".to_string()),
            description: Some("A test project".to_string()),
            design: vec![PathBuf::from("design/architecture.yaml")],
            sources: vec![SourceConfig {
                path: PathBuf::from("src"),
                exclude: vec!["vendor".to_string()],
            }],
            server: Some(ServerConfig {
                url: "http://localhost:3000".to_string(),
            }),
        };

        let yaml = serde_yaml::to_string(&config).expect("serialize");
        let back: ProjectConfig = serde_yaml::from_str(&yaml).expect("deserialize");
        assert_eq!(back.project, "my-app");
        assert_eq!(back.name.as_deref(), Some("My Application"));
        assert_eq!(back.description.as_deref(), Some("A test project"));
        assert_eq!(back.design.len(), 1);
        assert_eq!(back.sources.len(), 1);
        assert_eq!(back.sources[0].exclude, vec!["vendor".to_string()]);
        assert_eq!(
            back.server.as_ref().map(|s| s.url.as_str()),
            Some("http://localhost:3000")
        );
    }

    #[test]
    fn missing_optional_fields_get_defaults() {
        let yaml = "project: minimal-app\n";
        let config: ProjectConfig = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(config.project, "minimal-app");
        assert!(config.name.is_none());
        assert!(config.description.is_none());
        assert!(config.design.is_empty());
        // sources should default to [{path: "."}]
        assert_eq!(config.sources.len(), 1);
        assert_eq!(config.sources[0].path, PathBuf::from("."));
        assert!(config.sources[0].exclude.is_empty());
        assert!(config.server.is_none());
    }

    #[test]
    fn invalid_project_id_fails_validation() {
        let config = ProjectConfig {
            project: "INVALID-ID".to_string(),
            name: None,
            description: None,
            design: vec![],
            sources: default_sources(),
            server: None,
        };
        let tmp = tempfile::tempdir().expect("tempdir");
        let err = config.validate(tmp.path()).unwrap_err();
        assert!(
            err.to_string().contains("invalid project ID"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validation_catches_invalid_design_extension() {
        let config = ProjectConfig {
            project: "good-id".to_string(),
            name: None,
            description: None,
            design: vec![PathBuf::from("design/model.txt")],
            sources: default_sources(),
            server: None,
        };
        let tmp = tempfile::tempdir().expect("tempdir");
        let err = config.validate(tmp.path()).unwrap_err();
        assert!(
            err.to_string().contains(".yaml, .yml, or .json"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validation_catches_nonexistent_source_dir() {
        let config = ProjectConfig {
            project: "good-id".to_string(),
            name: None,
            description: None,
            design: vec![],
            sources: vec![SourceConfig {
                path: PathBuf::from("nonexistent"),
                exclude: vec![],
            }],
            server: None,
        };
        let tmp = tempfile::tempdir().expect("tempdir");
        let err = config.validate(tmp.path()).unwrap_err();
        assert!(
            err.to_string().contains("does not exist"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validation_passes_for_valid_config() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config = ProjectConfig {
            project: "valid-project".to_string(),
            name: Some("Valid".to_string()),
            description: None,
            design: vec![PathBuf::from("arch.yaml")],
            sources: vec![SourceConfig {
                path: PathBuf::from("."),
                exclude: vec![],
            }],
            server: None,
        };
        assert!(config.validate(tmp.path()).is_ok());
    }

    #[test]
    fn load_from_project_dir_returns_none_when_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let result = ProjectConfig::load_from_project_dir(tmp.path()).expect("no error");
        assert!(result.is_none());
    }

    #[test]
    fn load_from_project_dir_reads_config() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let svt_dir = tmp.path().join(".svt");
        std::fs::create_dir_all(&svt_dir).expect("create .svt dir");
        std::fs::write(svt_dir.join("config.yaml"), "project: loaded-project\n")
            .expect("write config");

        let config = ProjectConfig::load_from_project_dir(tmp.path())
            .expect("no error")
            .expect("config found");
        assert_eq!(config.project, "loaded-project");
    }

    #[test]
    fn validation_catches_source_path_escaping_root() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config = ProjectConfig {
            project: "good-id".to_string(),
            name: None,
            description: None,
            design: vec![],
            sources: vec![SourceConfig {
                path: PathBuf::from(".."),
                exclude: vec![],
            }],
            server: None,
        };
        let err = config.validate(tmp.path()).unwrap_err();
        assert!(
            err.to_string().contains("escapes project root"),
            "unexpected error: {err}"
        );
    }
}
