# Project Configuration & CLI Restructure Design

**Date:** 2026-03-05
**Status:** Draft
**Scope:** `.svt/config.yaml`, CLI flag changes, server simplification, plugin & store directory restructure

## Motivation

The current CLI requires many flags repeated on every invocation (`--project`, `--store`, `--server`, `--plugin`, design file paths, source paths). The server loads a single design file at startup via `--design`, which doesn't scale to multiple projects. Plugin installation is project-local (`.svt/plugins/`) which doesn't work in CI where `svt` is an installed binary.

A project-level config file eliminates repetition, captures project identity, and enables simpler workflows for both local development and CI.

## Design Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Config file at `.svt/config.yaml`, committed to VCS | Project identity and design file locations are shared across the team |
| 2 | Store DB moves to `.svt/data/store`, gitignored at `.svt/data/` | Separates committed config from local-only data |
| 3 | Config type lives in `svt-core` | Available to CLI now; server can store as project metadata later |
| 4 | Multiple design files merge into one design snapshot | A project has one logical design, split across files for manageability |
| 5 | Multiple source paths produce one combined analysis snapshot | Consistent with existing multi-language analysis |
| 6 | Server removes `--design`, `--project`, `--project-name` flags | Server becomes a pure data host; all config is client-side |
| 7 | Plugins installed alongside `svt` binary, not per-project | Works in CI; `--plugin-dir` overrides the default install location |
| 8 | `svt` CLI takes `--project-dir` to point at a project | Explicit project targeting; reads `.svt/config.yaml` from that directory |

## Config File Schema

```yaml
# .svt/config.yaml

# Project identity
project: my-app                    # Project ID for multi-tenancy (required)
name: "My Application"             # Human-readable display name (optional)
description: "Backend services"    # Project description (optional)

# Design model(s) — paths relative to project root
# All files are merged into a single design snapshot on import
design:
  - design/architecture.yaml
  - design/frontend.yaml
  - design/backend.yaml

# Analysis source directories — paths relative to project root
# All sources produce one combined analysis snapshot
sources:
  - path: .                        # Default if omitted
    # exclude:                     # Optional additional excludes beyond defaults
    #   - vendor/
    #   - third_party/

# Remote server for push operations
server:
  url: http://localhost:3000       # Used by `svt push` when --server not specified
```

### Required vs Optional Fields

| Field | Required | Default | Notes |
|-------|----------|---------|-------|
| `project` | Yes | — | Must be a valid project ID (lowercase alphanumeric + hyphens/underscores) |
| `name` | No | Same as `project` | Human-readable name |
| `description` | No | None | |
| `design` | No | Empty list | No design files to import |
| `sources` | No | `[{path: "."}]` | Analyze project root |
| `sources[].path` | Yes (per entry) | — | Relative to project root |
| `sources[].exclude` | No | Empty list | Additional excludes |
| `server` | No | None | Push requires either config or `--server` flag |
| `server.url` | Yes (if `server` present) | — | |

### Validation Rules

- `project` must pass `validate_project_id()` (existing validation)
- `design` paths must exist and be `.yaml`/`.yml`/`.json` files
- `sources[].path` must be existing directories within the project root
- `server.url` must be a valid URL with scheme (http/https)
- No duplicate entries in `design` or `sources`

## Directory Structure

### Before (current)

```
.svt/                    # Gitignored entirely
  store                  # CozoDB database
  plugins/               # Project-local plugins
```

### After

```
.svt/                    # Directory NOT ignored
  config.yaml            # Committed — project config
  data/                  # Gitignored — local-only data
    store                # CozoDB database
```

### Plugin Location

Plugins move from per-project to per-installation:

```
<svt-install-dir>/
  svt                    # Binary
  plugins/               # Default plugin directory
    libsvt_plugin_java.dylib
    svt-plugin-java.svt-plugin.toml
```

Discovery order:
1. `--plugin-dir <PATH>` (CLI override)
2. `SVT_PLUGIN_DIR` environment variable
3. `<svt-binary-dir>/plugins/` (default, adjacent to binary)

### .gitignore Change

```diff
- .svt
+ .svt/data
```

## CLI Changes

### Global Flags

```
svt [global flags] <command>
```

| Flag | Current | New | Notes |
|------|---------|-----|-------|
| `--store <PATH>` | Default `.svt/store` | Removed | Derived: `<project-dir>/.svt/data/store` |
| `--project <NAME>` | Default `"default"` | Removed | Read from config `project` field |
| `--plugin <PATH>...` | Repeat per library | Removed | Replaced by `--plugin-dir` |
| `--project-dir <PATH>` | — | New, default `"."` | Directory containing `.svt/config.yaml` |
| `--plugin-dir <PATH>` | — | New, default see above | Directory containing plugin libraries |

### Config Resolution

At startup, the CLI:

1. Resolves `--project-dir` (default: current directory)
2. Looks for `<project-dir>/.svt/config.yaml`
3. If found, parses and validates config
4. If not found, operates with defaults (project=`"default"`, no design files, source=`.`)
5. CLI flags override config values where applicable

### Command Changes

#### `svt import`

**Current:**
```
svt import <FILE>                    # Import one file
```

**New:**
```
svt import                           # Import all design files from config (merged)
svt import --file <PATH>             # Import a specific file (override)
```

Behavior with no args:
1. Read `design` list from config
2. Create one design snapshot
3. Parse and load each file into that snapshot (merged)
4. Report total nodes/edges/constraints

#### `svt analyze`

**Current:**
```
svt analyze [<PATH>] --commit-ref <REF> --incremental
```

**New:**
```
svt analyze --commit-ref <REF> --incremental
```

- Source paths read from config `sources` list (default: `["."]`)
- `<PATH>` positional arg removed; use config or `--project-dir`
- `--commit-ref` auto-detected from git HEAD if not specified (existing behavior)

#### `svt push`

**Current:**
```
svt push --server <URL> --version <N>
```

**New:**
```
svt push                             # Push latest analysis to configured server
svt push --kind design               # Push latest design snapshot
svt push --kind analysis             # Push latest analysis snapshot (default)
svt push --kind all                  # Push both latest design and analysis
svt push --server <URL>              # Override server URL
svt push --version <N>               # Push specific version
```

- Server URL read from config `server.url` if `--server` not specified
- Project ID read from config `project` field
- `--kind` flag selects what to push (default: `analysis`)

#### `svt check`

No significant changes. Operates on the local store. Project ID from config.

#### `svt export`

No significant changes. Operates on the local store. Project ID from config.

#### `svt diff`

No changes. Operates on version numbers directly.

#### `svt plugin`

**Current:**
```
svt plugin list
svt plugin install <SOURCE> --global --force
svt plugin remove <NAME> --global
svt plugin info <PATH>
```

**New:**
```
svt plugin list                      # List plugins from --plugin-dir
svt plugin install <SOURCE> --force  # Install to --plugin-dir
svt plugin remove <NAME>             # Remove from --plugin-dir
svt plugin info <PATH>               # Show manifest info (unchanged)
```

- `--global` flag removed (plugins are always in the plugin dir)
- Plugin discovery uses `--plugin-dir` / `SVT_PLUGIN_DIR` / default

#### `svt store`

```
svt store info                       # No change (uses project-dir store)
svt store compact --keep <N>...      # No change
svt store reset --force              # No change
```

Store path is always `<project-dir>/.svt/data/store`.

#### `svt init` (new)

```
svt init                             # Create .svt/config.yaml interactively
svt init --project <NAME>            # Create with specified project name
```

Scaffolds:
- `.svt/config.yaml` with project name (auto-detected from git dir name)
- `.svt/data/` directory
- Appends `.svt/data` to `.gitignore` if not present

## Server Changes

### Flags

**Current:**
```
svt-server --store <PATH> --design <PATH> --project <PATH>
           --project-name <NAME> --port <N> --host <ADDR>
```

**New:**
```
svt-server --store <PATH> --port <N> --host <ADDR>
```

| Flag | Status |
|------|--------|
| `--store <PATH>` | Required (no more in-memory default) |
| `--port <N>` | Kept, default 3000 |
| `--host <ADDR>` | Kept, default 0.0.0.0 |
| `--design <PATH>` | Removed |
| `--project <PATH>` | Removed |
| `--project-name <NAME>` | Removed |

The server is a pure data host. Projects and their data arrive via the push API.

### API

No API changes needed. The push endpoint already accepts both `kind: "design"` and `kind: "analysis"`, and auto-creates projects.

## Core Type

```rust
// svt-core::config

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    /// Source directory path (relative to project root).
    pub path: PathBuf,
    /// Additional directories to exclude from analysis.
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Remote server URL.
    pub url: String,
}

fn default_sources() -> Vec<SourceConfig> {
    vec![SourceConfig {
        path: PathBuf::from("."),
        exclude: vec![],
    }]
}
```

### Config Loading

```rust
impl ProjectConfig {
    /// Load from a YAML file.
    pub fn load(path: &Path) -> Result<Self, ConfigError>;

    /// Load from `.svt/config.yaml` relative to a project directory.
    pub fn load_from_project_dir(project_dir: &Path) -> Result<Option<Self>, ConfigError>;

    /// Validate all fields and referenced paths against a project root.
    pub fn validate(&self, project_root: &Path) -> Result<(), ConfigError>;
}
```

## Migration Path

### Backward Compatibility

- If no `.svt/config.yaml` exists, CLI operates with current defaults
- Old `.svt/store` location still works via `--store` override (though deprecated)
- Server continues to accept pushes from old CLI versions (API unchanged)

### Migration Steps for Existing Projects

1. Run `svt init` to create `.svt/config.yaml`
2. Move `.svt/store` to `.svt/data/store`
3. Update `.gitignore`: replace `.svt` with `.svt/data`
4. Install plugins to `<svt-binary-dir>/plugins/` instead of `.svt/plugins/`

## Example Workflows

### Local Development

```bash
# One-time setup
svt init --project my-app
# Edit .svt/config.yaml to add design files and server URL

# Daily workflow
svt import                    # Merge design files into design snapshot
svt analyze                   # Analyze source directories
svt check --analysis          # Conformance: design vs analysis
svt push --kind all           # Push both to server
```

### CI Pipeline

```bash
# svt is pre-installed with plugins
svt --project-dir /workspace/my-app analyze --incremental
svt --project-dir /workspace/my-app import
svt --project-dir /workspace/my-app check --analysis --fail-on error
svt --project-dir /workspace/my-app push --kind all
```

### Multiple Projects in One Repo (Monorepo)

```bash
# Each sub-project has its own .svt/config.yaml
svt --project-dir services/auth analyze
svt --project-dir services/api analyze
svt --project-dir services/auth push --kind all
svt --project-dir services/api push --kind all
```

## Implementation Plan

This design should be implemented as a milestone (pre-M30), since M30 (Java Analyzer) and future work benefit from the simplified CLI.

### Phase 1: Core Config Type
- Add `ProjectConfig`, `SourceConfig`, `ServerConfig` types to `svt-core`
- Config loading, parsing, validation
- Unit tests for config parsing and validation

### Phase 2: Directory Restructure
- Store path changes to `.svt/data/store`
- Plugin discovery from binary-adjacent directory
- `svt init` command
- `.gitignore` pattern change

### Phase 3: CLI Flag Restructure
- Replace `--store`, `--project`, `--plugin` with `--project-dir`, `--plugin-dir`
- Config resolution at startup (config + flag overrides)
- Update all commands to use resolved config

### Phase 4: Command Behavior Changes
- `svt import` with no args: merge design files from config
- `svt push --kind`: support design/analysis/all
- `svt analyze`: read source paths from config

### Phase 5: Server Simplification
- Remove `--design`, `--project`, `--project-name` flags
- Make `--store` required
- Update server documentation

### Phase 6: Tests & Documentation
- Integration tests for config-driven workflows
- Update CLI help text
- Update README and docs
