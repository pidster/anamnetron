# Software Project Comprehension Platform: Visualisation Architecture

## Purpose

This document defines the visualisation architecture for a platform that enables users to maximise their comprehension of a software project, regardless of that project's documentation state, age, activity level, authorship model, or team structure. It maps comprehension dimensions to specific visualisation types, defines persona-specific views, and specifies the design conformance model.

## Foundational Principles

### Start from comprehension, not charts

Every visualisation must answer a specific comprehension question. If the question can be answered more clearly with a table, a number, or a sentence, use that instead. Visualisations earn their complexity cost only when they reveal patterns that simpler representations cannot.

### Adapt to evidence availability

The platform must degrade gracefully across the spectrum of project states:

| Evidence source       | Always available | Sometimes available | Rarely available     |
|:----------------------|:-----------------|:--------------------|:---------------------|
| Source code (AST)     | ✓                |                     |                      |
| File system structure | ✓                |                     |                      |
| Git history           |                  | ✓ (most projects)   |                      |
| Dependency manifests  |                  | ✓ (most projects)   |                      |
| Test suites           |                  | ✓                   |                      |
| CI/CD configuration   |                  | ✓                   |                      |
| Design documents      |                  |                     | ✓ (the minority)     |
| Runtime telemetry     |                  |                     | ✓ (mature projects)  |

Visualisations that depend on evidence that may be absent must indicate this clearly rather than rendering empty or misleading. The dashboard should surface a "data completeness" indicator so users understand what the platform can and cannot see.

### The graph DB is the single source of truth

All visualisations are projections of the underlying graph. The graph schema should be designed for query flexibility, not for any specific visualisation's needs. Visualisations consume query results; they do not drive the data model.

The platform uses **CozoDB** — a transactional, relational-graph-vector database with Datalog queries. This choice has significant architectural implications:

- **Relational model, not labelled-property graph.** Data is stored in relations (tables), and graph structure is implicit — derived through Datalog joins and recursion. This is more composable than the property graph model and means the schema can evolve without the rigidity of predefined node/edge labels.
- **Datalog for queries.** Recursive Datalog is naturally suited to graph traversal (dependency chains, transitive closures, cycle detection) and composes cleanly — queries can be built from reusable rules. This directly benefits conformance checking, where queries like "find all dependency paths that violate a design rule" are naturally recursive.
- **Built-in graph algorithms.** CozoDB provides efficient implementations of PageRank, community detection, shortest path, and other whole-graph algorithms within Datalog. This eliminates the need for external algorithm libraries for the force-directed graph's community detection and the dependency analysis's hub identification.
- **Native time travel.** CozoDB supports timestamped assertions and retractions of facts, enabling point-in-time queries. This is the foundation for trend analysis (evolution dimension) and conformance drift tracking — the platform can reconstruct the state of the codebase graph at any historical point without maintaining separate snapshots.
- **Embeddable or client-server.** The prototype's "launch against a file path" model uses CozoDB in embedded mode (like SQLite). The future service variant uses client-server mode. Same database, same queries, different deployment topology.
- **Vector search (HNSW).** CozoDB's integrated vector search within Datalog enables semantic similarity queries. This is directly applicable to AI-assisted design-to-code mapping: embed design component descriptions and code module summaries, then use vector proximity to propose mappings.

---

## Current State

Before defining the target architecture, it is important to establish what already exists. The platform has a working visualization stack that new work should build upon, not replace.

### Existing visualization layers

**Mermaid diagrams (primary view):** `MermaidView.svelte` renders four diagram types — flowchart, data flow, sequence, and C4 — from the graph store. It supports 0.25x–4x zoom (keyboard and mouse wheel), dark/light theming, source copying, and was deliberately promoted as the primary visualization (commit `c51db65`). Mermaid excels at exportable, human-readable diagrams and requires no additional rendering infrastructure.

**Cytoscape.js interactive graph:** `GraphView.svelte` provides an interactive graph exploration view with three layout algorithms (ELK layered, Dagre, fCOSE force-directed), compound node support for hierarchical nesting, a minimap/navigator, context menus (expand/collapse, focus neighbourhood, set scope), Tippy.js tooltips, and per-kind node colouring and edge styling. It includes conformance overlay (pass/fail/unimplemented/undocumented classes) and diff overlay (added/removed/changed). Performance thresholds are built in: texture mode above 300 nodes, edge hiding above 500 nodes.

**Navigation and filtering:** A tree navigation panel with single-click selection supports scoping (restricting the view to a subtree) and depth filtering. Node kind, sub-kind, edge kind, and language filters are available.

### Existing data model

The graph store uses six CozoDB relations: `metadata`, `snapshots`, `nodes`, `edges`, `constraints`, and `file_manifest`. Key design decisions:

- **Generic schema:** Nodes have `kind` (System/Service/Component/Unit) and `sub_kind` (domain-specific string) columns. Edges have `kind` (Contains/Depends/Calls/Implements/Extends/DataFlow/Exports). New kinds do not require schema changes.
- **Provenance tracking:** Both nodes and edges carry a `provenance` column (Design/Analysis/Import/Inferred) that distinguishes prescriptive from descriptive data.
- **Extensible metadata:** Nodes, edges, and constraints have a `metadata: Json?` column for arbitrary key-value properties.
- **Versioned snapshots:** Composite keys `(id, version)` enable multiple analysis snapshots.

### Existing conformance infrastructure

`crates/core/src/conformance.rs` provides a trait-based constraint evaluation system with four built-in evaluators: `must_not_depend`, `boundary`, `must_contain`, and `max_fan_in`. The `ConformanceReport` includes violation details, unimplemented design nodes, undocumented analysis nodes, and a summary with pass/fail/warn/not-evaluable counts. The `GraphView` already renders conformance results as visual overlays.

### Existing analyzer capabilities

The analyzer extracts **structure only** — module/package hierarchy, dependency edges (import/use), and type relationships (implements, extends) — for Rust, TypeScript/JavaScript, Go, and Python via tree-sitter. It supports incremental analysis (only re-analyzing changed units). **No metrics are currently extracted** — no LOC, no cyclomatic complexity, no fan-in/fan-out counts, no code age. The `Node.metadata` field is available but unpopulated.

### Installed frontend libraries

- Cytoscape.js `^3.33.1` with six plugins (dagre, fcose, elk, navigator, context-menus, popper)
- Mermaid `^11.12.3`
- Tippy.js `^6.3.7`, Popper.js `^2.11.8`, ELK.js `^0.11.0`
- **D3.js is not currently installed**

---

## Comprehension Dimensions and Data Sources

### Dimension 1: Structure — "What are the parts and how do they relate?"

**Questions answered:**
- What modules/services/components exist?
- What depends on what?
- Are there circular dependencies?
- Which components are hubs? Which are isolated?
- How modular is the system actually (vs. how it appears)?

**Data sources:** Static analysis of imports/calls, dependency manifests (package.json, Cargo.toml, go.mod, etc.), file system hierarchy, AST extraction.

**Graph representation:** Nodes = modules/packages/files/classes. Edges = depends-on (with weight = number of import references or call sites). Properties on nodes: LOC, language, file count, public API surface.

### Dimension 2: Quality and Risk — "Where should I worry?"

**Questions answered:**
- Where are the most complex parts of the codebase?
- What is large, complex, *and* frequently changed (hotspots)?
- Where is test coverage thin on code that matters?
- Where is dead code accumulating?
- What has unusually high fan-in or fan-out?

**Data sources:** Static analysis (cyclomatic complexity, cognitive complexity, LOC, fan-in/fan-out, nesting depth), test coverage reports, git history (churn rate, change frequency).

**Graph representation:** Metrics as properties on file/class/function nodes. Derived properties: hotspot score = f(churn, complexity), risk score = f(complexity, inverse coverage).

### Dimension 3: Evolution — "How has this system changed, and what does the pattern reveal?"

**Questions answered:**
- Which parts are actively developed vs. dormant?
- What is the change coupling pattern (what co-evolves)?
- Where are bursts of activity correlated with incidents?
- Is complexity increasing or decreasing over time?
- How old is the code, and does age correlate with risk?

**Data sources:** Git history (commits, diffs, timestamps, authors), CI/CD logs, issue tracker links in commits.

**Graph representation:** Temporal edges = co-change relationships (weighted by frequency, with temporal decay). Node properties: last modified date, commit count, churn rate, age. Historical snapshots enable trend analysis.

### Dimension 4: Ownership and Knowledge — "Who understands what?"

**Questions answered:**
- Who are the primary contributors to each component?
- Where is knowledge concentrated in one person (bus factor)?
- Do ownership boundaries align with architectural boundaries?
- For AI-authored code: which parts have had substantive human review?
- Who should I talk to about a given area?

**Data sources:** Git blame/log, PR review data (if available), CODEOWNERS files, AI attribution signals (commit metadata, tool signatures in commits).

**Graph representation:** Author/team nodes linked to file/module nodes via authored/reviewed edges. Properties: contribution percentage, recency of contribution, review depth.

### Dimension 5: Intent and Conformance — "What was this supposed to be?"

**Questions answered:**
- Does the actual architecture match the designed architecture?
- Which designed components exist in code? Which don't?
- Which code has no corresponding design element (organic growth)?
- Are dependency rules being respected?
- Is the system converging toward or diverging from its design?

**Data sources:** Design documents (Mermaid, C4, ADRs, structured text), architectural fitness functions, actual structure from Dimensions 1–4.

**Graph representation:** A second set of "design intent" nodes and edges overlaid on the "actual" graph. Conformance = the delta between these two subgraphs. See dedicated section below.

---

## Visualisation Specifications

### V1: Dependency Chord Diagram

**Comprehension dimension:** Structure (primary), Conformance (when design is present)

**What it reveals:** The coupling shape between architectural peers — magnitude, direction, and balance of dependencies between modules, services, or bounded contexts. Most effective at 5–25 entities.

**Data source (graph query):** Aggregate dependency edges between top-level modules. Weight = number of import references or call sites. Optionally overlay change coupling from git.

**Interaction model:**
- Hover on a segment: highlight all its connections, fade others, show aggregate in/out metrics.
- Click on a segment: filter to show only that module's dependencies, with detail panel showing specific files/functions involved.
- Toggle between static dependencies and change coupling. When both are active, use split arcs or a secondary colour channel to show where they agree vs. diverge.
- Filter by dependency direction (inbound, outbound, bidirectional).
- Threshold slider: hide weak connections below N references to reduce noise.

**Conformance overlay (design-present mode):**
- Designed allowed-dependency rules define which arcs *should* exist and which are prohibited.
- Arcs that violate dependency rules render in a violation colour (e.g., red) with a distinct line style.
- Missing expected dependencies render as dashed ghost arcs.
- A conformance summary score is shown: percentage of actual dependencies that conform to design rules.

**Aggregation levels:**
- System level: services or top-level bounded contexts (for microservices / large monorepos).
- Module level: packages or modules within a single service/application.
- User can drill from system → module level by clicking a segment.

**When to hide this view:** Fewer than 3 entities (nothing to compare), or more than ~30 entities (becomes unreadable — suggest the force-directed graph instead).

**Implementation notes:** D3 `d3-chord` for the interactive chord diagram (new D3.js dependency required). ECharts chord if rapid development is prioritised. The key implementation challenge is the dual-layer (static + change coupling) overlay; consider a toggle rather than simultaneous display to avoid visual overload. Note: the existing Mermaid flowchart view already shows dependency structure in diagram form — the chord diagram adds a quantitative coupling-magnitude view that Mermaid cannot express.

---

### V2: Quality/Risk Treemap

**Comprehension dimension:** Quality and Risk (primary), Evolution (via churn metrics)

**What it reveals:** The relative size and health of every part of the codebase simultaneously. The dual encoding (area = one metric, colour = another) makes outliers and correlations immediately visible across thousands of files.

**Data source (graph query):** Hierarchical traversal of the code structure (project → package → module → file, or deeper to class/function level). Each leaf node carries multiple metric properties.

**Metric pair presets (user-switchable):**

| Preset name           | Area metric       | Colour metric            | What it reveals                                      |
|:----------------------|:------------------|:-------------------------|:-----------------------------------------------------|
| Complexity landscape  | Lines of code     | Cyclomatic complexity    | Large complex monsters                               |
| Test coverage gaps    | Lines of code     | Test coverage %          | Large untested areas                                 |
| Hotspots              | Change frequency  | Complexity               | Frequently changed complex code (bug magnets)        |
| Code age              | Lines of code     | Last modified date       | Ancient code (potential maintenance risk)             |
| Knowledge risk        | Lines of code     | Number of authors        | Bus factor — large code known by few                 |
| Churn intensity       | Lines of code     | Churn rate (edits/month) | Code under active pressure                           |
| Conformance health    | Lines of code     | Conformance score        | Design violations by size (design-present only)      |
| AI authorship         | Lines of code     | AI-authored %            | Human vs. AI contribution distribution               |

**Colour scales:** Diverging scales (green → amber → red) for metrics with a clear good/bad direction (coverage, conformance). Sequential scales for neutral metrics (age, author count). The colour scale boundaries should be configurable — what counts as "red" for complexity varies enormously between projects.

**Interaction model:**
- Hover: tooltip with file/class name, both metric values, and a mini-sparkline of the colour metric's trend over time (if git history is available).
- Click: drill into the selected rectangle to show its children (zoom into a package to see its files, into a file to see its classes/functions).
- Breadcrumb navigation for drilling back out.
- Metric pair selector: dropdown or button group to switch between presets.
- Right-click or long-press: "Why is this red?" — link to the specific analysis detail (e.g., which functions drive the complexity, which lines are uncovered).

**Layout algorithm:** Squarified treemap (Bruls et al.) for readability. For animated transitions when switching metrics, use `treemapResquarify` to preserve topology and avoid disorienting rearrangement.

**When to hide specific presets:** If test coverage data is unavailable, grey out that preset rather than showing an empty or misleading view. Same for git-dependent presets on projects without history.

**Implementation notes:** D3 `d3-hierarchy` for layout computation, rendered as Svelte SVG `{#each}` blocks. Alternatively, ECharts treemap for faster implementation. The switchable metric pairs are the key UX challenge — pre-compute all metric values in the Datalog query so that switching presets is instantaneous in the Svelte store (no re-query to CozoDB). A single query returns all leaf nodes with all metric columns; the Svelte component selects which columns map to area and colour. Consider WebGL rendering (e.g., via deck.gl) for very large codebases (100k+ files) where SVG performance degrades.

---

### V3: Architecture Sunburst

**Comprehension dimension:** Structure (primary), Ownership, Conformance

**What it reveals:** The hierarchical organisation of the system, proportional composition at each level, and — critically — the ability to navigate through the hierarchy interactively. Each concentric ring is a level of the architecture.

**Data source (graph query):** Hierarchical traversal of code structure. In design-present mode, the hierarchy can be driven by the *designed* architecture rather than the file system, with actual code mapped into it.

**Modes:**

**Mode A — Actual structure navigation:**
- Rings = file system or package hierarchy levels.
- Arc size = proportional LOC or file count at each level.
- Colour = switchable: by language, by ownership (team/individual), by test coverage, by code age, by conformance score.
- Purpose: orient a new developer, understand compositional balance, spot lopsided modules.

**Mode B — Design structure navigation (design-present only):**
- Rings = designed architectural levels from C4/Mermaid/formal model (system → container → component → code).
- Arc size = LOC of actual code mapped to each designed element.
- Colour = conformance status: green (code matches design), amber (code exists but diverges), red (design element with no corresponding code), grey (code with no corresponding design element).
- Empty/missing design segments rendered as outlined ghost arcs.
- Purpose: navigate the *intended* architecture and see where reality conforms or diverges.

**Interaction model:**
- Click on a segment: zoom in (sunburst re-renders with that segment as the new root). Centre circle shows breadcrumb / "click to zoom out".
- Hover: tooltip with segment name, metrics summary, ownership, and conformance status if applicable.
- In design mode: click a ghost segment (missing implementation) to see the design specification for what should be there.
- Toggle between Mode A and Mode B to compare actual vs. designed hierarchy.

**Zoomable depth:** Critical for large projects. Only render 2–3 rings at a time (following the D3 zoomable sunburst pattern). Deeper levels appear as the user drills in. This keeps the visualisation readable regardless of hierarchy depth.

**When to hide this view:** Very flat projects with minimal hierarchy (e.g., a single-directory script collection). In such cases, the treemap alone suffices.

**Implementation notes:** D3 zoomable sunburst (Observable example is the reference implementation). The `vasturiano/sunburst-chart` web component is a solid alternative — it is framework-agnostic and can be used in Svelte via a wrapper component with `bind:this`. Alternatively, compute the sunburst layout with `d3.partition()` and render arcs using Svelte's `{#each}` blocks with `transition:` directives for smooth zoom animations. The critical challenge is the design-overlay mode, which requires a mapping function between designed components and actual code paths — this is part of the conformance model specification below.

---

### V4: Force-Directed Dependency Graph (partially exists)

**Comprehension dimension:** Structure (topology and clustering)

**What it reveals:** The natural clustering and topology of the codebase — which modules form coherent groups, which are bridges between groups, which are isolated. Community detection algorithms (Louvain, Leiden) applied to this graph reveal the *actual* modularity, which may differ significantly from the *intended* modularity.

**Why it complements the chord diagram:** The chord diagram shows coupling magnitude between known groups. The force-directed graph discovers groups you didn't know existed. It answers "is this system actually modular?" rather than "how coupled are the modules I already defined?"

**Current state:** The existing `GraphView.svelte` already provides a force-directed layout via Cytoscape.js fCOSE, with compound node support, neighbourhood highlighting, context menus, and conformance overlays. The enhancements below add community detection and richer colouring modes to the existing infrastructure rather than replacing it.

**Data source (graph query):** Same dependency edge data as the chord diagram, but at a finer granularity (file-to-file or class-to-class). Community detection algorithm labels each node with a cluster ID.

**Interaction model (existing + enhancements):**
- Nodes positioned by fCOSE force simulation (existing); edges as lines (existing).
- Colour by detected community (auto-assigned, **new**) or by designed component (if available, **new**).
- Toggle between community colouring and designed-component colouring — discrepancies between these two views are a primary conformance signal (**new**).
- Click a node: highlight its immediate dependencies via neighbourhood focus (existing context menu action).
- Hull rendering: toggle convex hulls around detected communities to see cluster boundaries (**new**).
- Search: find a specific file/class and centre the view on it (enhancement of existing filter).
- Filter: show/hide edges below a weight threshold, show/hide external dependencies (extends existing kind filter).

**Implementation notes:** Use the existing Cytoscape.js fCOSE layout — do not replace with D3 force simulation. Community detection runs within CozoDB using its built-in Louvain algorithm via Datalog — cluster labels are returned as part of the query result and passed to `GraphView` as node properties for colouring. For very large graphs (1000+ nodes), the existing GraphView already applies performance mitigations (texture mode, edge hiding). Evaluate sigma.js only if these prove insufficient.

---

### V5: Evolution Timeline (recommended addition)

**Comprehension dimension:** Evolution (primary), Conformance (trend)

**What it reveals:** How the codebase has changed over time — activity patterns, growth trajectory, and whether quality/conformance metrics are improving or degrading.

**Sub-views:**

**Activity heatmap:** Calendar grid (weeks × days or months × weeks) with cells coloured by commit count or lines changed. Filterable by module, team, or author. Shows development rhythm, quiet periods, burst activity.

**Metric trends:** Line charts of key metrics over time (total LOC, average complexity, test coverage %, number of conformance violations, dependency count). Overlaid with release markers or milestone markers from design docs. The critical question: is the system getting healthier or sicker?

**Module-level activity stream:** For each major module, a horizontal timeline bar showing periods of activity vs. dormancy. Stacked or aligned to show which modules are being worked on in parallel. Reveals whether development is focused (one module at a time) or scattered (many modules touched per sprint).

**Implementation notes:** ECharts calendar heatmap for the activity view. LayerCake with D3 scales, or Plotly, for metric trend lines. These are simpler visualisations but critical for the "evolution" comprehension dimension, which the three original diagram types don't cover well.

---

## Design Conformance Model

### Overview

Conformance checking is a first-class analytical capability, not a bolted-on feature. The model works by maintaining two parallel representations in CozoDB — the **designed architecture** and the **actual architecture** — and continuously computing the delta between them via Datalog queries.

### Design Document Ingestion

**Supported formats (initial):**
- **Mermaid diagrams:** Parse component diagrams, flowcharts, and C4 diagrams expressed in Mermaid syntax. Extract nodes (components) and edges (relationships/dependencies).
- **C4 model (Structurizr DSL or JSON):** Native support for system context, container, component, and code-level views. This maps directly to the sunburst ring levels.
- **Structured text (Markdown/YAML):** A defined schema for expressing architectural intent in plain text — component names, responsibilities, allowed dependencies, ownership assignments. This is the lowest-friction input for projects that have *some* design thinking but no formal models.
- **ADRs (Architecture Decision Records):** Extract architectural constraints and rules (e.g., "the API gateway must not directly access the database"). These become conformance rules rather than structural definitions.

**Later additions:** ArchiMate, UML (XMI), OpenAPI specs (for service contract conformance), formal architecture models (AADL, SysML).

### Graph Representation (CozoDB Relations)

The conformance model builds on the **existing** generic graph schema rather than defining parallel relations. Design intent and actual structure coexist in the same `nodes` and `edges` relations, distinguished by the `provenance` column. Conformance is computed by Datalog queries that join across provenance boundaries — it is not stored statically, so results are always current.

**Existing relations used by the conformance model:**

```datalog
# These relations already exist (see crates/core/src/store/cozo.rs):
# nodes { id, version => canonical_path, qualified_name?, kind, sub_kind, name,
#          language?, provenance, source_ref?, metadata? }
# edges { id, version => source, target, kind, provenance, metadata? }
# constraints { id, version => kind, name, scope, target?, params?, message, severity }
```

- **Design components** are `nodes` entries with `provenance: "design"`. The `kind` column uses the same C4-aligned levels: System, Service, Component, Unit. Design-specific attributes (responsibility, layer, owner, source document) are stored in the `metadata` JSON column.
- **Design dependencies** are `edges` entries with `provenance: "design"` and `kind: "depends"`.
- **Forbidden dependencies** are `constraints` entries with `kind: "must_not_depend"`, evaluated by the existing `MustNotDependEvaluator`.
- **Boundary rules** are `constraints` entries with `kind: "boundary"`, evaluated by the existing `BoundaryEvaluator`.

**Mapping layer (new relation):**

The one additional relation needed is the design-to-code mapping, which connects design nodes to analysis nodes:

```datalog
:create design_code_mapping {
    design_node: String, analysis_node: String, version: Int
    =>
    mapping_method: String,   # "explicit", "convention", "ai_suggested"
    confidence: Float,        # 0.0–1.0
    confirmed_by: String?,    # human reviewer, if AI-suggested
    confirmed_at: String?
}
```

**Conformance queries using the actual schema:**

```datalog
# Structural conformance: design nodes with no mapped analysis code (missing implementations)
missing_impl[design_id, name, path] :=
    *nodes{ id: design_id, version: v, name, canonical_path: path, provenance: "design" },
    not *design_code_mapping{ design_node: design_id, version: v }

# Structural conformance: analysis nodes with no design mapping (undocumented growth)
unmapped_code[analysis_id, name, path] :=
    *nodes{ id: analysis_id, version: v, name, canonical_path: path, provenance: "analysis" },
    not *design_code_mapping{ analysis_node: analysis_id, version: v }

# Dependency conformance: actual dependencies that violate a must_not_depend constraint
# (This is what the existing MustNotDependEvaluator computes programmatically;
#  the Datalog equivalent for ad-hoc queries:)
dependency_violations[src_path, tgt_path, constraint_name, sev] :=
    *edges{ version: v, source: src_id, target: tgt_id, kind: "depends", provenance: "analysis" },
    *nodes{ id: src_id, version: v, canonical_path: src_path },
    *nodes{ id: tgt_id, version: v, canonical_path: tgt_path },
    *constraints{ version: v, kind: "must_not_depend", name: constraint_name,
                  scope: scope_pat, target: tgt_pat, severity: sev },
    # scope_pat and tgt_pat are glob patterns matched in application code
    # (CozoDB Datalog does not natively support glob matching)

# Aggregate conformance score per design component
component_conformance[design_id, name, score] :=
    *nodes{ id: design_id, version: v, name, provenance: "design" },
    violation_count[design_id, v_count],
    mapping_count[design_id, m_count],
    score = if(m_count == 0, 0.0, 1.0 - (v_count / m_count))
```

**Note on constraint evaluation:** The existing `ConstraintRegistry` in `crates/core/src/conformance.rs` provides four built-in evaluators (`must_not_depend`, `boundary`, `must_contain`, `max_fan_in`) that operate programmatically via the `GraphStore` trait. The Datalog queries above show the equivalent logic for ad-hoc analysis. In practice, conformance checking should use the Rust evaluators for correctness (they handle glob pattern matching and edge traversal that Datalog cannot express natively) and reserve Datalog for exploratory queries and reporting aggregation.

The key advantage of the generic schema is composability: new node kinds, edge kinds, and constraint types can be added without schema changes. The `metadata` JSON column accommodates domain-specific attributes without relation proliferation.

### Mapping: Design to Code

The mapping between designed components and actual code is the critical operational challenge. Three approaches, in order of preference:

1. **Explicit mapping file:** A configuration file (YAML or similar) maintained alongside the design docs that maps design component names to code paths, packages, or module patterns. Highest accuracy, highest maintenance cost.

2. **Convention-based inference:** Use naming conventions, directory structure, and package hierarchy to automatically map code to design components. Works well for well-structured projects, poorly for legacy code.

3. **AI-assisted mapping:** Use an LLM to read design docs and code structure, propose a mapping, and have a human review/approve it. This is likely the pragmatic middle ground for initial onboarding of a project.

The platform should support all three, with (1) as the source of truth when present, (2) as the default, and (3) as a bootstrap mechanism.

### Conformance Checks

Each check produces a conformance score (0.0–1.0) and a set of specific violations with severity and location.

**Structural conformance:**
- For each `design_component`, does a corresponding mapped module exist? (Missing implementation — see `missing_impl` query above)
- For each `module`, does a corresponding `design_code_mapping` entry exist? (Undocumented growth — see `unmapped_code` query above)
- Does the actual nesting match the designed containment hierarchy?

**Dependency conformance:**
- For each actual dependency edge, is there a corresponding DESIGN_DEPENDS_ON or at minimum no DESIGN_FORBIDS_DEPENDENCY? (Dependency violation)
- For each designed dependency, does the actual dependency exist? (Missing integration)
- Are there actual circular dependencies where the design specifies acyclic relationships?

**Responsibility conformance (advanced):**
- If design docs specify component responsibilities, use static analysis (exported API surface, function names, class names) and optionally AI-assisted analysis to assess whether the actual code's responsibilities match. This is inherently fuzzy and should be presented as a confidence signal rather than a binary pass/fail.

**Quantitative drift:**
- Track conformance scores over time. Alert when the trend is negative (system diverging from design).
- Per-release conformance delta: did this release improve or degrade conformance?

---

## Persona Views (Future Consideration)

> **Note:** The persona views described below are aspirational design guidance for when a configurable dashboard framework exists. The current UI is a single-view application with a navigation tree, diagram panel, and detail panel. These descriptions inform the eventual dashboard design but are not near-term deliverables.

Each persona needs a different entry point into the same underlying data. These are not different dashboards but different **default configurations** of the same dashboard, with full ability to reconfigure.

### Developer (joining or exploring)

**Primary question:** "How do I orient myself in this codebase?"

**Default view:** Architecture sunburst (Mode A — actual structure) as the centre panel. Colour by language or module type. Click to drill into the area they're working on. Side panel shows: file list for the selected segment, recent commits, primary contributors (who to ask).

**Secondary views available:** Treemap (complexity landscape preset) to understand where the complex parts are. Force-directed graph to see the overall shape. Evolution timeline filtered to the module they're working on.

**Conformance relevance:** Low initially. Once oriented, the developer benefits from seeing dependency conformance violations in their area — "you're adding a dependency that the architecture doesn't allow."

### Architect (reviewing health)

**Primary question:** "Is the system's actual architecture healthy and aligned with its design?"

**Default view:** Chord diagram (dependency coupling) as the centre panel, with conformance overlay if design docs are present. Side panel shows: conformance summary scores, top violations, trend charts.

**Secondary views available:** Force-directed graph with community detection (to compare detected modularity vs. designed modularity). Treemap (hotspots preset) to identify risk areas. Sunburst (Mode B — design structure) to navigate conformance by component.

**Conformance relevance:** High. The architect is the primary consumer of conformance data. Dashboard should surface: new violations since last review, conformance trend, components with the highest violation density.

### Tech Lead (planning work)

**Primary question:** "Where should we invest effort, and what's the risk landscape?"

**Default view:** Treemap (hotspots preset) as the centre panel — the intersection of change frequency and complexity identifies where bugs will concentrate and where refactoring has the highest ROI. Side panel shows: ranked list of hotspot files with trend indicators, ownership information for each.

**Secondary views available:** Evolution timeline (to understand development rhythm and identify modules under sustained pressure). Chord diagram (change coupling) to understand the blast radius of planned changes. Treemap (knowledge risk preset) to identify bus-factor risks before they materialise.

**Conformance relevance:** Medium. The tech lead uses conformance data to prioritise architectural debt reduction: which violations are in active code (and therefore worth fixing) vs. in dormant code (and therefore lower priority)?

### Additional personas (future)

**Product/Programme Manager:** High-level composition view (sunburst coloured by feature area or team), evolution timeline for progress tracking, conformance trend for governance reporting.

**Security Reviewer:** Treemap coloured by external dependency count or known vulnerability density. Dependency chord showing third-party integration points. Ownership view to identify who to involve in security reviews.

**New Team Member / AI Agent:** The "explain this codebase" narrative mode — a guided walkthrough that progresses through: sunburst (overview structure) → chord diagram (how parts connect) → treemap (where the complexity lives) → evolution timeline (what's active). This could be automated as an onboarding sequence.

---

## Static Report Variant (Phase 3+, Future)

> **Note:** This section describes infrastructure that does not yet exist and represents a significant engineering effort orthogonal to the core visualization work. It depends on the visualization components being complete first. **Near-term alternative:** Mermaid's built-in SVG export already provides basic static diagram output and can serve as a starting point for static reports without the full pipeline described below.

The CI-generated periodic report should be a **snapshot** of the dashboard state, not a separate system. Key design decisions:

**Format:** HTML (for interactivity in a browser) or PDF (for archival/email distribution). The HTML variant can embed lightweight versions of the interactive visualisations (e.g., static SVG treemaps with hover tooltips). The PDF variant uses static renders.

**Content structure:**

1. **Executive summary:** One-paragraph health assessment. Key metrics: total LOC, average complexity, test coverage %, conformance score (if applicable), number of hotspots, bus-factor risk count.

2. **Delta since last report:** What changed. New violations, resolved violations, modules with significant metric movement (complexity increase, coverage decrease). This is the most actionable section.

3. **Visualisation snapshots:**
   - Treemap (hotspots preset) — static render with the top 10 hotspots labelled.
   - Chord diagram — current dependency coupling state, with violations highlighted if design-present.
   - Conformance summary (design-present only) — scores by component, trend chart.

4. **Detailed findings:** Ranked list of specific issues — new hotspots, new conformance violations, files exceeding complexity thresholds, declining coverage areas.

5. **Trend charts:** Key metrics over the last N reports, showing trajectory.

**Generation pipeline:** The data collectors (running periodically or triggered by CI) update CozoDB. The report generator queries CozoDB via Datalog, renders visualisations server-side (e.g., using Puppeteer for headless chart rendering, or server-side D3/node-canvas), and assembles the report. The report is versioned and stored, enabling historical comparison. CozoDB's time travel means the report can include "state at last report" vs. "state now" comparisons without the report generator maintaining its own history.

---

## Data Collection Architecture

### Collectors

Each data source has a dedicated collector that extracts data and writes it to CozoDB via the platform API.

| Collector              | Input                        | Relations populated                              | Status | Frequency         |
|:-----------------------|:-----------------------------|:-------------------------------------------------|:-------|:-------------------|
| Structure analyser     | Source code (AST parsing)    | `nodes` (kind: Component/Unit), `edges` (kind: Contains/Depends/Calls/Implements/Extends) | **Exists** — Rust, TS, Go, Python | On code change     |
| Dependency analyser    | Manifest files               | `nodes` (sub_kind: "external_dep"), `edges` (kind: Depends) | **Exists** — via cargo metadata, package.json | On manifest change |
| Metrics analyser       | Source code                  | Updates `nodes.metadata` with LOC, complexity, fan-in/fan-out | **Not yet built** — Phase 0 prerequisite | On code change     |
| Git history analyser   | Git log                      | `nodes` (sub_kind: "author", "commit"), `edges` (kind: DataFlow for authorship) | **Not yet built** | Periodic (daily)   |
| Coverage analyser      | Coverage reports (lcov etc.) | Updates `nodes.metadata` with coverage data      | **Not yet built** | On CI run          |
| Design doc parser      | Mermaid, C4, YAML, ADR       | `nodes` (provenance: Design), `edges` (provenance: Design), `constraints` | **Partially exists** — YAML design model ingestion works | On doc change   |
| Design-code mapper     | Design + analysis nodes      | `design_code_mapping` relation                   | **Not yet built** | On analysis run    |
| Conformance calculator | GraphStore (Rust evaluators) | Computed at query time via `ConstraintRegistry` — no separate storage | **Exists** — 4 evaluators | On-demand          |

### Incremental Updates

For the service variant (periodic data collection), collectors must support incremental updates — not full re-analysis on every run. CozoDB's native time travel (timestamped assertions and retractions) handles historical state reconstruction without maintaining separate snapshots. Collectors assert new facts and retract stale ones with timestamps; any Datalog query can then be scoped to a point in time for trend analysis.

### API Design Consideration

The platform API should accept collector output as a standardised event format (e.g., "node created", "edge created", "property updated") rather than collector-specific formats. This enables new collectors to be added without platform changes, and enables third-party tool integration (e.g., SonarQube results pushed via the same API).

---

## Implementation Priorities

These phases build incrementally on the existing system (see **Current State** section above). The structure analyzer, dependency analyzer, Mermaid diagrams, and Cytoscape.js graph already exist and are working.

### Phase 0: Metrics Foundation (prerequisite for new visualizations)

The new visualization types (treemap, chord, sunburst) require quantitative data that the analyzer does not yet extract.

1. **LOC extraction** — Compute lines of code per file/module from tree-sitter byte ranges (trivially derivable from existing AST parsing). Populate `nodes.metadata` with `{ "loc": N }`.
2. **Fan-in / fan-out counts** — Count incoming and outgoing `Depends` edges per node. Populate `nodes.metadata` with `{ "fan_in": N, "fan_out": N }`.
3. **API endpoint for metrics** — Expose per-node metrics via the server API so the web frontend can query them.
4. **Verify metadata round-trip** — Ensure `nodes.metadata` JSON survives CozoDB storage → API → frontend pipeline.

This phase works for *any* project with source code. No git history, no design docs, no CI integration required. LOC alone unlocks the treemap; fan-in/fan-out enriches the chord diagram.

### Phase 1: New analytical visualizations

5. **Treemap** (D3 `d3-hierarchy`) — area = LOC, colour = fan-out or depth. Highest new-information-per-effort ratio. Requires D3.js as a new frontend dependency.
6. **Chord diagram** (D3 `d3-chord`) — dependency coupling between top-level modules. Can use unweighted edge counts initially; LOC-weighted arcs when metrics are available.
7. **Sunburst** (D3 `d3-hierarchy` + `d3.partition()`) — hierarchical navigation with arc size proportional to LOC (or child count as fallback). Mode A (actual structure) only.
8. **Metric pair switching** on treemap — switchable presets using LOC, fan-in, fan-out, depth.

### Phase 2: Evolution and risk (add git history)

9. Git history analyser → treemap hotspots preset (churn × complexity)
10. Change coupling extraction → chord diagram toggle (static vs. change coupling)
11. Ownership extraction → sunburst coloured by author/team
12. Evolution timeline views (evaluate ECharts for calendar heatmap, LayerCake or D3 for trend lines)
13. Knowledge risk treemap preset

### Phase 3: Quality integration (add CI/test data)

14. Cyclomatic complexity extraction in analyzer → treemap complexity preset
15. Coverage analyser → treemap coverage preset
16. Static report generation (start with Mermaid SVG export; full Puppeteer pipeline is a later effort)

### Phase 4: Conformance visualization overlays

17. Design-to-code mapping relation and UI
18. Sunburst Mode B (design structure navigation)
19. Chord diagram conformance overlay (violation arcs, ghost arcs for missing dependencies)
20. Treemap conformance preset (colour = conformance score)
21. Conformance trend tracking over time

### Phase 5: Advanced

22. Community detection via CozoDB's built-in Louvain algorithm → force-directed graph community colouring (complements existing Cytoscape.js fCOSE layout)
23. AI-assisted design-to-code mapping (vector search via CozoDB HNSW)
24. Responsibility conformance (AI-assisted)
25. CI integration for periodic report generation (full pipeline)
26. Runtime telemetry integration
27. Cross-project/cross-service views

---

## Technology Recommendations

### Visualisation layer

The platform uses a **layered visualization approach** with three complementary libraries, each serving a distinct purpose:

**Layer 1 — Mermaid (diagram generation, exists):** Mermaid is the current primary visualization and handles static, exportable diagrams: flowcharts, data flow, sequence, and C4 diagrams. It requires no custom rendering code — diagrams are generated from text descriptions derived from the graph store. Mermaid excels at producing human-readable, version-control-friendly, and easily exportable output. It is the right tool for structured architectural diagrams and will continue to serve this role.

**Layer 2 — Cytoscape.js (interactive graph exploration, exists):** Cytoscape.js is the existing interactive graph engine with compound node support, multiple layout algorithms (ELK, Dagre, fCOSE), a minimap, context menus, and conformance/diff overlays. It handles the force-directed dependency graph view (V4) — the fCOSE layout already provides force simulation. Cytoscape.js should remain the tool for interactive graph exploration where users need to navigate, filter, focus, and drill into graph topology.

**Layer 3 — D3.js (analytical visualizations, new):** D3.js is recommended for the **new** visualization types that Mermaid and Cytoscape.js are not designed for: chord diagrams (`d3-chord`), treemaps (`d3-hierarchy`), and sunburst charts (`d3-hierarchy` + `d3.partition()`). D3 provides the layout algorithms and scales; Svelte handles the rendering.

| Visualization | Library | Rationale |
|:-------------|:--------|:----------|
| Flowchart, C4, Sequence, Data Flow | Mermaid | Already built, exportable, text-based |
| Interactive dependency graph (V4) | Cytoscape.js | Already built, compound nodes, layouts, overlays |
| Dependency chord diagram (V1) | D3.js | `d3-chord` layout, no Cytoscape equivalent |
| Quality/risk treemap (V2) | D3.js | `d3-hierarchy` squarified layout |
| Architecture sunburst (V3) | D3.js | `d3-hierarchy` partition layout |
| Evolution timeline (V5) | D3.js or ECharts | Evaluate both — see notes below |

**Svelte + D3 integration pattern:** Svelte and D3 coexist far more naturally than most frameworks. The key patterns:

- **D3 for computation, Svelte for rendering.** Use D3's layout algorithms (`d3-hierarchy` for treemap/sunburst, `d3-chord` for chord diagram) and scales, but render the resulting geometry using Svelte's `{#each}` blocks over SVG elements. This gives you Svelte-managed reactivity, transitions (`transition:` directives), and event handling on individual chart elements without fighting D3's selection/enter/exit model.
- **D3 for direct DOM manipulation when needed.** For complex interactive behaviours (zoom/pan via `d3-zoom`, brush selections via `d3-brush`), D3 can operate directly on a container element obtained via `bind:this`. Svelte does not use a virtual DOM, so there is no reconciliation conflict — D3's mutations are the ground truth. This is the right approach for the zoomable sunburst.
- **Reactive data flow.** Svelte stores (`writable`, `derived`) or Svelte 5 runes (`$state`, `$derived`) drive the data pipeline: raw query results → derived/transformed data → D3 layout computation → rendered SVG. When the user switches a treemap metric pair, the store updates, D3 recomputes the layout, and Svelte's reactivity re-renders the affected elements with transitions.

**Libraries to evaluate (not yet decided):**

- **LayerCake** — Svelte-native data visualisation framework. Potentially useful for simpler chart types (evolution timeline, metric trend lines). Needs evaluation against the project's Vite + Svelte 5 (runes) build setup before adoption.
- **ECharts** — framework-agnostic charting library. Worth evaluating for evolution timeline views (calendar heatmap, line charts) where D3's lower-level API adds unnecessary implementation cost. Works in Svelte via `bind:this`.
- **`vasturiano/sunburst-chart`** — framework-agnostic web component for sunburst charts. Needs evaluation for compatibility with Svelte 5 runes model before recommending over native D3 `d3.partition()`.
- **sigma.js / deck.gl** — WebGL rendering for very large codebases (100k+ files, 10k+ rendered elements). Evaluate only if SVG performance becomes a bottleneck.

**Component architecture:** Each visualisation should be a self-contained Svelte component that accepts data via props and emits interaction events (e.g., `dispatch('segment-click', { moduleId })`). A dashboard layout component orchestrates cross-visualisation interactions (clicking a module in the sunburst highlights it in the treemap, etc.) via shared Svelte stores. This keeps visualisation logic decoupled from coordination logic.

### Graph DB — CozoDB

CozoDB is the data layer. Its capabilities map directly to platform requirements:

| Platform requirement               | CozoDB capability                                                     |
|:------------------------------------|:----------------------------------------------------------------------|
| Dependency chain traversal          | Recursive Datalog with transitive closure                             |
| Cycle detection                     | Recursive queries with cycle-safe aggregation                         |
| Hub/authority identification        | Built-in PageRank algorithm                                           |
| Community detection (force graph)   | Built-in community detection algorithms (Louvain) within Datalog      |
| Trend analysis / historical state   | Native time travel (timestamped assertions and retractions)           |
| Conformance rule checking           | Datalog pattern matching across design and actual subgraphs           |
| AI-assisted design-to-code mapping  | HNSW vector search integrated with Datalog joins                      |
| Prototype (local file path)         | Embedded mode (RocksDB or in-memory backend)                          |
| Service (multi-project)             | Client-server mode with MVCC for concurrent writes                    |

**Schema approach:** CozoDB uses a relational model, not a labelled-property graph. Data is stored in relations (tables) with defined columns. Graph structure emerges from joins across relations. The platform uses a **generic schema** (see Current State section) where node and edge *kinds* are column values, not separate relations. This means new kinds can be added without schema changes.

```datalog
# Actual relations (from crates/core/src/store/cozo.rs):
# :create nodes { id: String, version: Int => canonical_path: String,
#     qualified_name?: String, kind: String, sub_kind: String, name: String,
#     language?: String, provenance: String, source_ref?: String, metadata?: Json }
# :create edges { id: String, version: Int => source: String, target: String,
#     kind: String, provenance: String, metadata?: Json }
# :create constraints { id: String, version: Int => kind: String, name: String,
#     scope: String, target?: String, params?: Json, message: String, severity: String }

# Example query: find all dependency edges between analysis nodes
?[src_name, tgt_name, src_path, tgt_path] :=
    *edges{ version: v, source: src_id, target: tgt_id, kind: "depends", provenance: "analysis" },
    *nodes{ id: src_id, version: v, name: src_name, canonical_path: src_path },
    *nodes{ id: tgt_id, version: v, name: tgt_name, canonical_path: tgt_path }

# Example query: transitive dependency chain
reachable[from_id, to_id] :=
    *edges{ version: v, source: from_id, target: to_id, kind: "depends" }
reachable[from_id, to_id] :=
    *edges{ version: v, source: from_id, target: mid_id, kind: "depends" },
    reachable[mid_id, to_id]

# Example query: PageRank for hub identification
?[node_name, rank] <~ PageRank(*edges[source, target], weight: 1)
```

**Time travel for evolution tracking:** Rather than maintaining separate historical snapshots, use CozoDB's timestamped fact assertions. When a collector runs, it asserts new facts and retracts stale ones with timestamps. Queries can specify a point in time to reconstruct historical state:

```datalog
# Metrics at a point in time (conceptual — exact syntax depends on CozoDB time travel API)
?[name, loc] := *nodes{ name, metadata } @ '2025-01-15T00:00:00Z',
    loc = get(metadata, "loc")
```

This enables the evolution timeline views and conformance trend tracking without duplicating the entire graph per collection run.

**Vector search for design mapping:** Store vector embeddings of design component descriptions and code module summaries (generated by an LLM) in a relation with an HNSW index. The AI-assisted mapping bootstrap can then query:

```datalog
# Find code modules most similar to a design component description
# (Assumes a future 'node_embeddings' relation with HNSW index)
?[module_name, dist] :=
    *nodes{ name: "AuthenticationService", provenance: "design", metadata: m },
    design_vec = get(m, "embedding"),
    ~node_embeddings:semantic{ module_name | query: design_vec, k: 5, ef: 50, bind_distance: dist }
```

### Report generation

Server-side rendering of D3 visualisations via Puppeteer (headless Chrome) or node-canvas. Alternatively, generate static SVGs server-side using D3 in Node.js without a browser. If using SvelteKit for the dashboard, its SSR capabilities can render chart components server-side — LayerCake's SSR support is particularly useful here. Assemble into HTML or PDF (via Puppeteer print-to-PDF or a dedicated PDF library).