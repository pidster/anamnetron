//! Content hashing and file change detection for incremental analysis.
//!
//! Uses BLAKE3 for fast, deterministic content hashing of source files.
//! Manifests track per-file hashes grouped by language unit, enabling
//! unit-level skip decisions during incremental analysis.

use std::collections::HashSet;
use std::io;
use std::path::Path;

use svt_core::model::FileManifestEntry;

use crate::orchestrator::LanguageUnit;

/// Hash file contents with BLAKE3, returning a 64-character hex string.
pub fn hash_file(path: &Path) -> io::Result<String> {
    let contents = std::fs::read(path)?;
    Ok(blake3::hash(&contents).to_hex().to_string())
}

/// Build a file manifest from discovered language units.
///
/// Hashes every source file, storing paths relative to `project_root`.
/// Returns the manifest entries and a list of warning messages for
/// files that could not be read.
pub fn build_manifest(
    project_root: &Path,
    units: &[(&str, &LanguageUnit)],
) -> (Vec<FileManifestEntry>, Vec<String>) {
    let mut entries = Vec::new();
    let mut warnings = Vec::new();

    for &(language_id, unit) in units {
        for file in &unit.source_files {
            let relative = file
                .strip_prefix(project_root)
                .unwrap_or(file)
                .to_string_lossy()
                .to_string();

            match hash_file(file) {
                Ok(hash) => {
                    entries.push(FileManifestEntry {
                        path: relative,
                        hash,
                        unit_name: unit.name.clone(),
                        language: language_id.to_string(),
                    });
                }
                Err(e) => {
                    warnings.push(format!("cannot hash {}: {}", file.display(), e));
                }
            }
        }
    }

    (entries, warnings)
}

/// Compare two manifests and return the set of unit names that need re-analysis.
///
/// A unit is considered changed if any of its files are added, removed, or
/// modified compared to the previous manifest.
pub fn changed_units(
    current: &[FileManifestEntry],
    previous: &[FileManifestEntry],
) -> HashSet<String> {
    use std::collections::HashMap;

    // Build lookup: path -> (hash, unit_name) for each manifest
    let prev_map: HashMap<&str, (&str, &str)> = previous
        .iter()
        .map(|e| (e.path.as_str(), (e.hash.as_str(), e.unit_name.as_str())))
        .collect();

    let curr_map: HashMap<&str, (&str, &str)> = current
        .iter()
        .map(|e| (e.path.as_str(), (e.hash.as_str(), e.unit_name.as_str())))
        .collect();

    let mut changed = HashSet::new();

    // Check for new or modified files in current
    for (path, (hash, unit_name)) in &curr_map {
        match prev_map.get(path) {
            Some((prev_hash, _)) => {
                if hash != prev_hash {
                    changed.insert(unit_name.to_string());
                }
            }
            None => {
                // New file — unit is changed
                changed.insert(unit_name.to_string());
            }
        }
    }

    // Check for removed files (in previous but not in current)
    for (path, (_, unit_name)) in &prev_map {
        if !curr_map.contains_key(path) {
            changed.insert(unit_name.to_string());
        }
    }

    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hash_file_returns_64_char_hex_string() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.rs");
        std::fs::write(&file, b"fn main() {}").unwrap();

        let hash = hash_file(&file).unwrap();
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_file_is_deterministic() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.rs");
        std::fs::write(&file, b"fn main() {}").unwrap();

        let h1 = hash_file(&file).unwrap();
        let h2 = hash_file(&file).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_file_differs_for_different_content() {
        let dir = tempfile::tempdir().unwrap();
        let f1 = dir.path().join("a.rs");
        let f2 = dir.path().join("b.rs");
        std::fs::write(&f1, b"fn a() {}").unwrap();
        std::fs::write(&f2, b"fn b() {}").unwrap();

        let h1 = hash_file(&f1).unwrap();
        let h2 = hash_file(&f2).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn build_manifest_creates_entries_for_all_files() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir(&src).unwrap();
        std::fs::write(src.join("lib.rs"), b"// lib").unwrap();
        std::fs::write(src.join("main.rs"), b"// main").unwrap();

        let unit = LanguageUnit {
            name: "my-crate".to_string(),
            language: "rust".to_string(),
            root: dir.path().to_path_buf(),
            source_root: src.clone(),
            source_files: vec![src.join("lib.rs"), src.join("main.rs")],
            top_level_kind: svt_core::model::NodeKind::Service,
            top_level_sub_kind: "crate".to_string(),
            source_ref: "src/lib.rs".to_string(),
            parent_qualified_name: None,
            workspace_dependencies: vec![],
        };

        let (entries, warnings) = build_manifest(dir.path(), &[("rust", &unit)]);
        assert!(warnings.is_empty());
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|e| e.unit_name == "my-crate"));
        assert!(entries.iter().all(|e| e.language == "rust"));
    }

    #[test]
    fn build_manifest_uses_relative_paths() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("src").join("lib.rs");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, b"// lib").unwrap();

        let unit = LanguageUnit {
            name: "my-crate".to_string(),
            language: "rust".to_string(),
            root: dir.path().to_path_buf(),
            source_root: dir.path().join("src"),
            source_files: vec![file],
            top_level_kind: svt_core::model::NodeKind::Service,
            top_level_sub_kind: "crate".to_string(),
            source_ref: "src/lib.rs".to_string(),
            parent_qualified_name: None,
            workspace_dependencies: vec![],
        };

        let (entries, _) = build_manifest(dir.path(), &[("rust", &unit)]);
        assert_eq!(entries[0].path, "src/lib.rs");
    }

    #[test]
    fn build_manifest_warns_on_unreadable_file() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nonexistent.rs");

        let unit = LanguageUnit {
            name: "my-crate".to_string(),
            language: "rust".to_string(),
            root: dir.path().to_path_buf(),
            source_root: dir.path().to_path_buf(),
            source_files: vec![missing],
            top_level_kind: svt_core::model::NodeKind::Service,
            top_level_sub_kind: "crate".to_string(),
            source_ref: "lib.rs".to_string(),
            parent_qualified_name: None,
            workspace_dependencies: vec![],
        };

        let (entries, warnings) = build_manifest(dir.path(), &[("rust", &unit)]);
        assert!(entries.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("cannot hash"));
    }

    #[test]
    fn changed_units_detects_new_file() {
        let previous = vec![];
        let current = vec![FileManifestEntry {
            path: "src/lib.rs".to_string(),
            hash: "a".repeat(64),
            unit_name: "my-crate".to_string(),
            language: "rust".to_string(),
        }];

        let changed = changed_units(&current, &previous);
        assert!(changed.contains("my-crate"));
    }

    #[test]
    fn changed_units_detects_modified_file() {
        let previous = vec![FileManifestEntry {
            path: "src/lib.rs".to_string(),
            hash: "a".repeat(64),
            unit_name: "my-crate".to_string(),
            language: "rust".to_string(),
        }];
        let current = vec![FileManifestEntry {
            path: "src/lib.rs".to_string(),
            hash: "b".repeat(64),
            unit_name: "my-crate".to_string(),
            language: "rust".to_string(),
        }];

        let changed = changed_units(&current, &previous);
        assert!(changed.contains("my-crate"));
    }

    #[test]
    fn changed_units_detects_removed_file() {
        let previous = vec![FileManifestEntry {
            path: "src/lib.rs".to_string(),
            hash: "a".repeat(64),
            unit_name: "my-crate".to_string(),
            language: "rust".to_string(),
        }];
        let current = vec![];

        let changed = changed_units(&current, &previous);
        assert!(changed.contains("my-crate"));
    }

    #[test]
    fn changed_units_returns_empty_when_no_changes() {
        let hash = "a".repeat(64);
        let entries = vec![FileManifestEntry {
            path: "src/lib.rs".to_string(),
            hash: hash.clone(),
            unit_name: "my-crate".to_string(),
            language: "rust".to_string(),
        }];

        let changed = changed_units(&entries, &entries);
        assert!(changed.is_empty());
    }

    #[test]
    fn changed_units_marks_new_unit_as_changed() {
        let previous = vec![FileManifestEntry {
            path: "src/lib.rs".to_string(),
            hash: "a".repeat(64),
            unit_name: "existing-crate".to_string(),
            language: "rust".to_string(),
        }];
        let current = vec![
            FileManifestEntry {
                path: "src/lib.rs".to_string(),
                hash: "a".repeat(64),
                unit_name: "existing-crate".to_string(),
                language: "rust".to_string(),
            },
            FileManifestEntry {
                path: "new-crate/src/lib.rs".to_string(),
                hash: "b".repeat(64),
                unit_name: "new-crate".to_string(),
                language: "rust".to_string(),
            },
        ];

        let changed = changed_units(&current, &previous);
        assert!(changed.contains("new-crate"));
        assert!(
            !changed.contains("existing-crate"),
            "unchanged unit should not appear"
        );
    }

    // -- proptest --

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn identical_manifests_produce_no_changes(
            count in 1usize..20,
        ) {
            let entries: Vec<FileManifestEntry> = (0..count)
                .map(|i| FileManifestEntry {
                    path: format!("src/file{i}.rs"),
                    hash: format!("{:064x}", i),
                    unit_name: format!("unit-{}", i % 3),
                    language: "rust".to_string(),
                })
                .collect();

            let changed = changed_units(&entries, &entries);
            prop_assert!(changed.is_empty(), "identical manifests should produce no changes");
        }

        #[test]
        fn all_new_units_are_changed(
            count in 1usize..20,
        ) {
            let current: Vec<FileManifestEntry> = (0..count)
                .map(|i| FileManifestEntry {
                    path: format!("src/file{i}.rs"),
                    hash: format!("{:064x}", i),
                    unit_name: format!("unit-{i}"),
                    language: "rust".to_string(),
                })
                .collect();

            let changed = changed_units(&current, &[]);
            // Every unit in current should appear as changed
            let expected_units: HashSet<String> = current.iter().map(|e| e.unit_name.clone()).collect();
            prop_assert_eq!(changed, expected_units);
        }
    }
}
