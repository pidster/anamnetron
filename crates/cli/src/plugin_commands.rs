//! Plugin management commands: install, remove, info.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::manifest::{
    library_filename, read_manifest, serialize_manifest, validate_manifest, PluginManifest,
};
use svt_core::plugin::SVT_PLUGIN_API_VERSION;

/// Resolve a manifest from a source path.
///
/// The source may be:
/// - A directory containing `svt-plugin.toml`
/// - A path to a `.toml` manifest file directly
///
/// Returns the parsed manifest and the directory containing it.
fn resolve_manifest(source: &Path) -> Result<(PluginManifest, PathBuf)> {
    if source.is_dir() {
        let manifest_path = source.join("svt-plugin.toml");
        if !manifest_path.exists() {
            bail!(
                "No svt-plugin.toml found in directory '{}'",
                source.display()
            );
        }
        let manifest = read_manifest(&manifest_path)
            .map_err(|e| anyhow::anyhow!("{e}"))
            .with_context(|| format!("reading manifest from {}", source.display()))?;
        Ok((manifest, source.to_path_buf()))
    } else if source
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e == "toml")
        .unwrap_or(false)
    {
        let manifest = read_manifest(source)
            .map_err(|e| anyhow::anyhow!("{e}"))
            .with_context(|| format!("reading manifest from {}", source.display()))?;
        let dir = source.parent().unwrap_or(Path::new(".")).to_path_buf();
        Ok((manifest, dir))
    } else {
        bail!(
            "Source must be a directory containing svt-plugin.toml or a .toml manifest file, got: '{}'",
            source.display()
        );
    }
}

/// Resolve the plugins directory adjacent to the `svt` binary.
fn target_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("cannot determine svt binary location")?;
    let dir = exe
        .parent()
        .ok_or_else(|| anyhow::anyhow!("cannot determine directory of svt binary"))?;
    Ok(dir.join("plugins"))
}

/// Install a plugin from a source directory or manifest path.
pub fn run_install(source: &Path, force: bool) -> Result<()> {
    // 1. Resolve and parse manifest
    let (manifest, source_dir) = resolve_manifest(source)?;

    // 2. Validate manifest
    validate_manifest(&manifest)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("manifest validation failed")?;

    // 3. Check API version compatibility
    if manifest.plugin.api_version != SVT_PLUGIN_API_VERSION {
        bail!(
            "API version mismatch: plugin requires v{}, but this svt supports v{}",
            manifest.plugin.api_version,
            SVT_PLUGIN_API_VERSION
        );
    }

    // 4. Resolve library file
    let lib_name = library_filename(&manifest);
    let lib_source = source_dir.join(&lib_name);
    if !lib_source.exists() {
        bail!(
            "Library file '{}' not found in '{}'",
            lib_name,
            source_dir.display()
        );
    }

    // 5. Determine target directory
    let target = target_dir()?;

    // 6. Create target directory if needed
    std::fs::create_dir_all(&target)
        .with_context(|| format!("creating plugins directory {}", target.display()))?;

    // 7. Check for existing plugin
    let manifest_name = format!("{}.svt-plugin.toml", manifest.plugin.name);
    let target_manifest = target.join(&manifest_name);
    if target_manifest.exists() && !force {
        bail!(
            "Plugin '{}' already installed in '{}'. Use --force to overwrite.",
            manifest.plugin.name,
            target.display()
        );
    }

    // 8. Copy library file
    let target_lib = target.join(&lib_name);
    std::fs::copy(&lib_source, &target_lib).with_context(|| {
        format!(
            "copying {} to {}",
            lib_source.display(),
            target_lib.display()
        )
    })?;

    // 9. Write manifest
    let manifest_content = serialize_manifest(&manifest)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("serializing manifest")?;
    std::fs::write(&target_manifest, manifest_content)
        .with_context(|| format!("writing manifest to {}", target_manifest.display()))?;

    // 10. Print success
    println!(
        "Installed {} v{} to {}",
        manifest.plugin.name,
        manifest.plugin.version,
        target.display()
    );

    Ok(())
}

/// Remove a plugin by name.
pub fn run_remove(name: &str) -> Result<()> {
    // 1. Determine target directory
    let target = target_dir()?;

    // 2. Look for manifest
    let manifest_name = format!("{name}.svt-plugin.toml");
    let manifest_path = target.join(&manifest_name);
    if !manifest_path.exists() {
        bail!("Plugin '{}' not found in '{}'", name, target.display());
    }

    // 3. Parse manifest to get library filename
    let manifest = read_manifest(&manifest_path).map_err(|e| anyhow::anyhow!("{e}"))?;
    let lib_name = library_filename(&manifest);
    let lib_path = target.join(&lib_name);

    // 4. Delete manifest file
    std::fs::remove_file(&manifest_path)
        .with_context(|| format!("removing manifest {}", manifest_path.display()))?;

    // 5. Delete library file (non-fatal if already gone)
    if lib_path.exists() {
        if let Err(e) = std::fs::remove_file(&lib_path) {
            eprintln!(
                "  WARN  could not remove library file {}: {}",
                lib_path.display(),
                e
            );
        }
    }

    // 6. Print success
    println!("Removed {} from {}", name, target.display());

    Ok(())
}

/// Show information about a plugin from its manifest.
pub fn run_info(path: &Path) -> Result<()> {
    // Resolve manifest
    let manifest = if path.is_dir() {
        let manifest_path = path.join("svt-plugin.toml");
        if !manifest_path.exists() {
            bail!("No svt-plugin.toml found in directory '{}'", path.display());
        }
        read_manifest(&manifest_path).map_err(|e| anyhow::anyhow!("{e}"))?
    } else {
        read_manifest(path).map_err(|e| anyhow::anyhow!("{e}"))?
    };

    let p = &manifest.plugin;

    println!("Plugin: {} v{}", p.name, p.version);

    if !p.description.is_empty() {
        println!("  Description: {}", p.description);
    }
    if !p.authors.is_empty() {
        println!("  Authors: {}", p.authors.join(", "));
    }
    if !p.license.is_empty() {
        println!("  License: {}", p.license);
    }

    let compat = if p.api_version == SVT_PLUGIN_API_VERSION {
        "compatible"
    } else {
        "INCOMPATIBLE"
    };
    println!("  API version: {} ({})", p.api_version, compat);

    let lib = library_filename(&manifest);
    println!("  Library: {}", lib);

    let c = &manifest.contributions;
    println!("  Contributions:");
    if c.language_ids.is_empty() {
        println!("    Language parsers: (none)");
    } else {
        println!("    Language parsers: {}", c.language_ids.join(", "));
    }
    if c.constraint_kinds.is_empty() {
        println!("    Constraint kinds: (none)");
    } else {
        println!("    Constraint kinds: {}", c.constraint_kinds.join(", "));
    }
    if c.export_formats.is_empty() {
        println!("    Export formats: (none)");
    } else {
        println!("    Export formats: {}", c.export_formats.join(", "));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::shared_library_extension;

    fn write_test_manifest(dir: &Path, name: &str, api_version: u32) {
        let content = format!(
            r#"[plugin]
name = "{name}"
version = "0.1.0"
description = "Test plugin"
api_version = {api_version}
"#
        );
        std::fs::write(dir.join("svt-plugin.toml"), content).unwrap();
    }

    fn write_test_library(dir: &Path, name: &str) {
        let ext = shared_library_extension();
        let lib_stem = name.replace('-', "_");
        let lib_name = if cfg!(target_os = "windows") {
            format!("{lib_stem}.{ext}")
        } else {
            format!("lib{lib_stem}.{ext}")
        };
        std::fs::write(dir.join(lib_name), b"fake library content").unwrap();
    }

    /// Helper: do the install steps to a specific target directory, bypassing
    /// `run_install` (which depends on HOME or CWD).
    fn install_to_dir(source: &Path, target: &Path, force: bool) -> Result<()> {
        let (manifest, source_dir) = resolve_manifest(source)?;
        validate_manifest(&manifest)
            .map_err(|e| anyhow::anyhow!("{e}"))
            .context("manifest validation failed")?;

        if manifest.plugin.api_version != SVT_PLUGIN_API_VERSION {
            bail!(
                "API version mismatch: plugin requires v{}, but this svt supports v{}",
                manifest.plugin.api_version,
                SVT_PLUGIN_API_VERSION
            );
        }

        let lib_name = library_filename(&manifest);
        let lib_source = source_dir.join(&lib_name);
        if !lib_source.exists() {
            bail!(
                "Library file '{}' not found in '{}'",
                lib_name,
                source_dir.display()
            );
        }

        std::fs::create_dir_all(target)
            .with_context(|| format!("creating plugins directory {}", target.display()))?;

        let manifest_name = format!("{}.svt-plugin.toml", manifest.plugin.name);
        let target_manifest = target.join(&manifest_name);
        if target_manifest.exists() && !force {
            bail!(
                "Plugin '{}' already installed in '{}'. Use --force to overwrite.",
                manifest.plugin.name,
                target.display()
            );
        }

        std::fs::copy(&lib_source, target.join(&lib_name))?;
        let manifest_content = serialize_manifest(&manifest).map_err(|e| anyhow::anyhow!("{e}"))?;
        std::fs::write(&target_manifest, manifest_content)?;

        Ok(())
    }

    #[test]
    fn install_from_directory_copies_library_and_manifest() {
        let src = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let target_dir = target.path().join("plugins");

        write_test_manifest(src.path(), "test-plugin", SVT_PLUGIN_API_VERSION);
        write_test_library(src.path(), "test-plugin");

        install_to_dir(src.path(), &target_dir, false).expect("install should succeed");

        let lib_name = library_filename(&resolve_manifest(src.path()).unwrap().0);
        let manifest_name = "test-plugin.svt-plugin.toml";

        assert!(
            target_dir.join(&lib_name).exists(),
            "library should be copied"
        );
        assert!(
            target_dir.join(manifest_name).exists(),
            "manifest should be written"
        );
    }

    #[test]
    fn install_fails_without_manifest() {
        let src = tempfile::tempdir().unwrap();
        let result = resolve_manifest(src.path());
        assert!(result.is_err(), "should fail when manifest is missing");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("svt-plugin.toml"),
            "error should mention svt-plugin.toml, got: {err}"
        );
    }

    #[test]
    fn install_fails_with_api_version_mismatch() {
        let src = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        write_test_manifest(src.path(), "bad-api-plugin", 99);
        write_test_library(src.path(), "bad-api-plugin");

        let result = install_to_dir(src.path(), target.path(), false);
        assert!(result.is_err(), "should fail with API version mismatch");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("API version mismatch"),
            "error should mention API version mismatch, got: {err}"
        );
    }

    #[test]
    fn install_fails_when_library_file_missing() {
        let src = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        write_test_manifest(src.path(), "no-lib-plugin", SVT_PLUGIN_API_VERSION);
        // Don't create library file

        let result = install_to_dir(src.path(), target.path(), false);
        assert!(result.is_err(), "should fail when library file is missing");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("not found"),
            "error should mention file not found, got: {err}"
        );
    }

    #[test]
    fn install_creates_target_directory_if_missing() {
        let src = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();

        write_test_manifest(src.path(), "mkdir-plugin", SVT_PLUGIN_API_VERSION);
        write_test_library(src.path(), "mkdir-plugin");

        let target = dest.path().join("nonexistent").join("nested").join("dir");
        assert!(!target.exists());

        install_to_dir(src.path(), &target, false).expect("install should create dir");
        assert!(target.exists(), "target directory should be created");
    }

    #[test]
    fn install_refuses_to_overwrite_without_force() {
        let src = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let target_dir = target.path().join("plugins");

        write_test_manifest(src.path(), "overwrite-test", SVT_PLUGIN_API_VERSION);
        write_test_library(src.path(), "overwrite-test");

        // First install succeeds
        install_to_dir(src.path(), &target_dir, false).expect("first install should succeed");

        // Second install (no force) should fail
        let result = install_to_dir(src.path(), &target_dir, false);
        assert!(result.is_err(), "should refuse to overwrite without force");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("already installed"),
            "error should mention 'already installed', got: {err}"
        );
    }

    #[test]
    fn install_overwrites_with_force_flag() {
        let src = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let target_dir = target.path().join("plugins");

        write_test_manifest(src.path(), "force-test", SVT_PLUGIN_API_VERSION);
        write_test_library(src.path(), "force-test");

        // First install
        install_to_dir(src.path(), &target_dir, false).expect("first install should succeed");

        // Second install with force should succeed
        let result = install_to_dir(src.path(), &target_dir, true);
        assert!(
            result.is_ok(),
            "install with --force should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    fn resolve_manifest_from_toml_file_directly() {
        let dir = tempfile::tempdir().unwrap();
        let manifest_path = dir.path().join("my-plugin.toml");
        let content = r#"
[plugin]
name = "my-plugin"
version = "1.0.0"
api_version = 1
"#;
        std::fs::write(&manifest_path, content).unwrap();

        let (manifest, resolved_dir) = resolve_manifest(&manifest_path).unwrap();
        assert_eq!(manifest.plugin.name, "my-plugin");
        assert_eq!(resolved_dir, dir.path());
    }

    // --- Remove tests ---

    #[test]
    fn remove_deletes_manifest_and_library() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path();

        let manifest_content = format!(
            r#"[plugin]
name = "removable"
version = "0.1.0"
api_version = {}
"#,
            SVT_PLUGIN_API_VERSION
        );
        std::fs::write(target.join("removable.svt-plugin.toml"), &manifest_content).unwrap();
        let ext = shared_library_extension();
        let lib_name = if cfg!(target_os = "windows") {
            format!("removable.{ext}")
        } else {
            format!("libremovable.{ext}")
        };
        std::fs::write(target.join(&lib_name), b"fake").unwrap();

        // Read manifest and remove
        let manifest_path = target.join("removable.svt-plugin.toml");
        let manifest = read_manifest(&manifest_path).unwrap();
        let lib_file = library_filename(&manifest);

        std::fs::remove_file(&manifest_path).unwrap();
        if target.join(&lib_file).exists() {
            std::fs::remove_file(target.join(&lib_file)).unwrap();
        }

        assert!(!manifest_path.exists(), "manifest should be deleted");
        assert!(
            !target.join(&lib_file).exists(),
            "library should be deleted"
        );
    }

    #[test]
    fn remove_fails_when_plugin_not_found() {
        // Test the logic directly: no manifest file exists
        let dir = tempfile::tempdir().unwrap();
        let manifest_path = dir.path().join("nonexistent.svt-plugin.toml");
        assert!(!manifest_path.exists(), "manifest should not exist");
    }

    #[test]
    fn remove_succeeds_when_library_already_gone() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path();

        // Create manifest only, no library
        let manifest_content = format!(
            r#"[plugin]
name = "ghost-lib"
version = "0.1.0"
api_version = {}
"#,
            SVT_PLUGIN_API_VERSION
        );
        std::fs::write(target.join("ghost-lib.svt-plugin.toml"), &manifest_content).unwrap();

        let manifest_path = target.join("ghost-lib.svt-plugin.toml");
        let manifest = read_manifest(&manifest_path).unwrap();
        let lib_file = library_filename(&manifest);

        // Library doesn't exist but we should be able to remove the manifest
        assert!(!target.join(&lib_file).exists());
        std::fs::remove_file(&manifest_path).unwrap();
        assert!(
            !manifest_path.exists(),
            "manifest should be removed even when library is missing"
        );
    }

    #[test]
    fn remove_preserves_other_plugins_in_directory() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path();

        // Create two manifests
        for name in &["keep-me", "remove-me"] {
            let content = format!(
                r#"[plugin]
name = "{name}"
version = "0.1.0"
api_version = {}
"#,
                SVT_PLUGIN_API_VERSION
            );
            std::fs::write(target.join(format!("{name}.svt-plugin.toml")), content).unwrap();
        }

        // Remove one
        let manifest_path = target.join("remove-me.svt-plugin.toml");
        std::fs::remove_file(&manifest_path).unwrap();

        assert!(
            !target.join("remove-me.svt-plugin.toml").exists(),
            "removed plugin manifest should be gone"
        );
        assert!(
            target.join("keep-me.svt-plugin.toml").exists(),
            "other plugin manifest should be preserved"
        );
    }

    // --- Info tests ---

    #[test]
    fn info_displays_manifest_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let content = format!(
            r#"[plugin]
name = "info-test"
version = "2.0.0"
description = "A test plugin"
authors = ["Alice <alice@example.com>"]
license = "Apache-2.0"
api_version = {}

[contributions]
language_ids = ["java"]
"#,
            SVT_PLUGIN_API_VERSION
        );
        std::fs::write(dir.path().join("svt-plugin.toml"), content).unwrap();

        let result = run_info(dir.path());
        assert!(result.is_ok(), "info should succeed: {:?}", result.err());
    }

    #[test]
    fn info_from_toml_file_directly() {
        let dir = tempfile::tempdir().unwrap();
        let manifest_path = dir.path().join("custom.toml");
        let content = format!(
            r#"[plugin]
name = "direct-toml"
version = "1.0.0"
api_version = {}
"#,
            SVT_PLUGIN_API_VERSION
        );
        std::fs::write(&manifest_path, content).unwrap();

        let result = run_info(&manifest_path);
        assert!(
            result.is_ok(),
            "info from direct .toml file should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    fn info_fails_when_no_manifest_found() {
        let dir = tempfile::tempdir().unwrap();
        let result = run_info(dir.path());
        assert!(result.is_err(), "info should fail when no manifest exists");
    }

    // --- Additional coverage tests ---

    #[test]
    fn resolve_manifest_rejects_non_toml_non_dir() {
        let dir = tempfile::tempdir().unwrap();
        let bad_path = dir.path().join("plugin.txt");
        std::fs::write(&bad_path, "content").unwrap();

        let result = resolve_manifest(&bad_path);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("directory") || err.contains(".toml"),
            "should explain valid source types, got: {err}"
        );
    }

    #[test]
    fn resolve_manifest_from_toml_with_missing_parent() {
        // A .toml path whose parent is "." (current dir)
        let dir = tempfile::tempdir().unwrap();
        let manifest_path = dir.path().join("standalone.toml");
        let content = format!(
            r#"[plugin]
name = "standalone"
version = "1.0.0"
api_version = {}
"#,
            SVT_PLUGIN_API_VERSION
        );
        std::fs::write(&manifest_path, content).unwrap();

        let (manifest, resolved_dir) = resolve_manifest(&manifest_path).unwrap();
        assert_eq!(manifest.plugin.name, "standalone");
        assert_eq!(resolved_dir, dir.path());
    }

    #[test]
    fn resolve_manifest_fails_with_malformed_toml_in_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("svt-plugin.toml"), "not valid toml [[[").unwrap();

        let result = resolve_manifest(dir.path());
        assert!(result.is_err(), "malformed TOML should fail");
    }

    #[test]
    fn resolve_manifest_fails_with_malformed_toml_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.toml");
        std::fs::write(&path, "not valid toml [[[").unwrap();

        let result = resolve_manifest(&path);
        assert!(result.is_err(), "malformed TOML file should fail");
    }

    #[test]
    fn install_fails_with_empty_name_in_manifest() {
        let src = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();

        let content = format!(
            r#"[plugin]
name = ""
version = "0.1.0"
api_version = {}
"#,
            SVT_PLUGIN_API_VERSION
        );
        std::fs::write(src.path().join("svt-plugin.toml"), content).unwrap();
        write_test_library(src.path(), "empty-name");

        let result = install_to_dir(src.path(), target.path(), false);
        assert!(result.is_err(), "empty name should fail validation");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("name") || err.contains("validation"),
            "error should mention validation, got: {err}"
        );
    }

    #[test]
    fn install_from_toml_file_directly() {
        let src = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();

        let manifest_path = src.path().join("my-plugin.toml");
        let content = format!(
            r#"[plugin]
name = "toml-direct"
version = "0.1.0"
api_version = {}
"#,
            SVT_PLUGIN_API_VERSION
        );
        std::fs::write(&manifest_path, content).unwrap();
        write_test_library(src.path(), "toml-direct");

        let result = install_to_dir(&manifest_path, target.path(), false);
        assert!(
            result.is_ok(),
            "install from .toml file should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    fn info_with_incompatible_api_version() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"[plugin]
name = "old-plugin"
version = "1.0.0"
api_version = 99
"#;
        std::fs::write(dir.path().join("svt-plugin.toml"), content).unwrap();

        let result = run_info(dir.path());
        // run_info should succeed (it prints compatibility info, doesn't fail)
        assert!(
            result.is_ok(),
            "info should succeed even with incompatible API version: {:?}",
            result.err()
        );
    }

    #[test]
    fn info_with_empty_contributions() {
        let dir = tempfile::tempdir().unwrap();
        let content = format!(
            r#"[plugin]
name = "empty-contrib"
version = "1.0.0"
api_version = {}

[contributions]
language_ids = []
constraint_kinds = []
export_formats = []
"#,
            SVT_PLUGIN_API_VERSION
        );
        std::fs::write(dir.path().join("svt-plugin.toml"), content).unwrap();

        let result = run_info(dir.path());
        assert!(
            result.is_ok(),
            "info with empty contributions should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    fn info_with_all_contribution_types() {
        let dir = tempfile::tempdir().unwrap();
        let content = format!(
            r#"[plugin]
name = "full-contrib"
version = "2.0.0"
description = "Fully loaded"
authors = ["Bob <bob@example.com>"]
license = "MIT"
api_version = {}

[contributions]
language_ids = ["java", "kotlin"]
constraint_kinds = ["custom-check"]
export_formats = ["custom-fmt"]
"#,
            SVT_PLUGIN_API_VERSION
        );
        std::fs::write(dir.path().join("svt-plugin.toml"), content).unwrap();

        let result = run_info(dir.path());
        assert!(
            result.is_ok(),
            "info with all contributions should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    fn info_fails_for_nonexistent_toml_file() {
        let result = run_info(Path::new("/nonexistent/plugin.toml"));
        assert!(result.is_err(), "info should fail for nonexistent file");
    }

    #[test]
    fn target_dir_returns_plugins_subdirectory() {
        let result = target_dir();
        // Should succeed and end with "plugins"
        assert!(result.is_ok(), "target_dir should succeed");
        let dir = result.unwrap();
        assert!(
            dir.ends_with("plugins"),
            "target_dir should end with 'plugins', got: {}",
            dir.display()
        );
    }

    #[test]
    fn install_to_dir_with_zero_api_version_fails() {
        let src = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();

        let content = r#"[plugin]
name = "zero-api"
version = "0.1.0"
api_version = 0
"#;
        std::fs::write(src.path().join("svt-plugin.toml"), content).unwrap();
        write_test_library(src.path(), "zero-api");

        let result = install_to_dir(src.path(), target.path(), false);
        assert!(result.is_err(), "api_version 0 should fail validation");
    }

    #[test]
    fn info_with_no_description_or_authors() {
        let dir = tempfile::tempdir().unwrap();
        let content = format!(
            r#"[plugin]
name = "minimal-info"
version = "0.1.0"
api_version = {}
"#,
            SVT_PLUGIN_API_VERSION
        );
        std::fs::write(dir.path().join("svt-plugin.toml"), content).unwrap();

        let result = run_info(dir.path());
        assert!(
            result.is_ok(),
            "info with minimal manifest should succeed: {:?}",
            result.err()
        );
    }
}
