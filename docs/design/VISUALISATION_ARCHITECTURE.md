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

**Implementation notes:** D3 `d3-chord` for maximum control. ECharts chord if rapid development is prioritised. The key implementation challenge is the dual-layer (static + change coupling) overlay; consider a toggle rather than simultaneous display to avoid visual overload.

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

**Implementation notes:** D3 `d3-hierarchy` treemap or ECharts treemap. The switchable metric pairs are the key UX challenge — pre-compute all metric values on the graph side so switching is instantaneous (no re-query). Consider WebGL rendering (e.g., via deck.gl) for very large codebases (100k+ files) where SVG performance degrades.

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

**Implementation notes:** D3 zoomable sunburst (Observable example is the reference implementation). The `vasturiano/sunburst-chart` web component is a solid alternative with React bindings. The critical challenge is the design-overlay mode, which requires a mapping function between designed components and actual code paths — this is part of the conformance model specification below.

---

### V4: Force-Directed Dependency Graph (recommended addition)

**Comprehension dimension:** Structure (topology and clustering)

**What it reveals:** The natural clustering and topology of the codebase — which modules form coherent groups, which are bridges between groups, which are isolated. Community detection algorithms (Louvain, Leiden) applied to this graph reveal the *actual* modularity, which may differ significantly from the *intended* modularity.

**Why it complements the chord diagram:** The chord diagram shows coupling magnitude between known groups. The force-directed graph discovers groups you didn't know existed. It answers "is this system actually modular?" rather than "how coupled are the modules I already defined?"

**Data source (graph query):** Same dependency edge data as the chord diagram, but at a finer granularity (file-to-file or class-to-class). Community detection algorithm labels each node with a cluster ID.

**Interaction model:**
- Nodes positioned by force simulation; edges as lines.
- Colour by detected community (auto-assigned) or by designed component (if available).
- Toggle between community colouring and designed-component colouring — discrepancies between these two views are a primary conformance signal.
- Click a node: highlight its immediate dependencies (1-hop), with option to expand to 2-hop.
- Hull rendering: toggle convex hulls around detected communities to see cluster boundaries.
- Search: find a specific file/class and centre the view on it.
- Filter: show/hide edges below a weight threshold, show/hide external dependencies.

**Implementation notes:** D3 force simulation, or for large graphs (1000+ nodes) consider WebCoLa or a WebGL-based renderer (sigma.js, Graphology with rendering adapter). The Emerge tool's approach is a good reference. Community detection should run server-side (graph DB query or dedicated algorithm) and provide cluster labels as node properties.

---

### V5: Evolution Timeline (recommended addition)

**Comprehension dimension:** Evolution (primary), Conformance (trend)

**What it reveals:** How the codebase has changed over time — activity patterns, growth trajectory, and whether quality/conformance metrics are improving or degrading.

**Sub-views:**

**Activity heatmap:** Calendar grid (weeks × days or months × weeks) with cells coloured by commit count or lines changed. Filterable by module, team, or author. Shows development rhythm, quiet periods, burst activity.

**Metric trends:** Line charts of key metrics over time (total LOC, average complexity, test coverage %, number of conformance violations, dependency count). Overlaid with release markers or milestone markers from design docs. The critical question: is the system getting healthier or sicker?

**Module-level activity stream:** For each major module, a horizontal timeline bar showing periods of activity vs. dormancy. Stacked or aligned to show which modules are being worked on in parallel. Reveals whether development is focused (one module at a time) or scattered (many modules touched per sprint).

**Implementation notes:** ECharts calendar heatmap for the activity view. Recharts or Plotly for metric trends. These are simpler visualisations but critical for the "evolution" comprehension dimension, which the three original diagram types don't cover well.

---

## Design Conformance Model

### Overview

Conformance checking is a first-class analytical capability, not a bolted-on feature. The model works by maintaining two parallel representations in the graph DB — the **designed architecture** and the **actual architecture** — and continuously computing the delta between them.

### Design Document Ingestion

**Supported formats (initial):**
- **Mermaid diagrams:** Parse component diagrams, flowcharts, and C4 diagrams expressed in Mermaid syntax. Extract nodes (components) and edges (relationships/dependencies).
- **C4 model (Structurizr DSL or JSON):** Native support for system context, container, component, and code-level views. This maps directly to the sunburst ring levels.
- **Structured text (Markdown/YAML):** A defined schema for expressing architectural intent in plain text — component names, responsibilities, allowed dependencies, ownership assignments. This is the lowest-friction input for projects that have *some* design thinking but no formal models.
- **ADRs (Architecture Decision Records):** Extract architectural constraints and rules (e.g., "the API gateway must not directly access the database"). These become conformance rules rather than structural definitions.

**Later additions:** ArchiMate, UML (XMI), OpenAPI specs (for service contract conformance), formal architecture models (AADL, SysML).

### Graph Representation

```
Design subgraph:
  (DesignComponent)-[:DESIGN_CONTAINS]->(DesignComponent)
  (DesignComponent)-[:DESIGN_DEPENDS_ON]->(DesignComponent)
  (DesignComponent)-[:DESIGN_FORBIDS_DEPENDENCY]->(DesignComponent)
  (DesignComponent {name, responsibility, layer, owner, source_doc})

Mapping layer:
  (DesignComponent)-[:MAPS_TO]->(ActualModule)
  (ActualModule)-[:UNMAPPED]  // no corresponding design element

Conformance edges (computed):
  (ActualModule)-[:CONFORMS {score, details}]->(DesignComponent)
  (ActualModule)-[:VIOLATES {rule, severity, details}]->(DesignComponent)
```

### Mapping: Design to Code

The mapping between designed components and actual code is the critical operational challenge. Three approaches, in order of preference:

1. **Explicit mapping file:** A configuration file (YAML or similar) maintained alongside the design docs that maps design component names to code paths, packages, or module patterns. Highest accuracy, highest maintenance cost.

2. **Convention-based inference:** Use naming conventions, directory structure, and package hierarchy to automatically map code to design components. Works well for well-structured projects, poorly for legacy code.

3. **AI-assisted mapping:** Use an LLM to read design docs and code structure, propose a mapping, and have a human review/approve it. This is likely the pragmatic middle ground for initial onboarding of a project.

The platform should support all three, with (1) as the source of truth when present, (2) as the default, and (3) as a bootstrap mechanism.

### Conformance Checks

Each check produces a conformance score (0.0–1.0) and a set of specific violations with severity and location.

**Structural conformance:**
- For each DesignComponent, does a corresponding ActualModule exist? (Missing implementation)
- For each ActualModule, does a corresponding DesignComponent exist? (Undocumented growth)
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

## Persona Views

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

## Static Report Variant

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

**Generation pipeline:** The data collectors (running periodically or triggered by CI) update the graph DB. The report generator queries the graph DB, renders visualisations server-side (e.g., using Puppeteer for headless chart rendering, or server-side D3/node-canvas), and assembles the report. The report is versioned and stored, enabling historical comparison.

---

## Data Collection Architecture

### Collectors

Each data source has a dedicated collector that extracts data and writes it to the graph DB via the platform API.

| Collector              | Input                        | Nodes created               | Edges created                     | Frequency         |
|:-----------------------|:-----------------------------|:----------------------------|:----------------------------------|:-------------------|
| Structure analyser     | Source code (AST parsing)    | File, Module, Class, Func   | IMPORTS, CALLS, CONTAINS          | On code change     |
| Dependency analyser    | Manifest files               | Package, ExternalDep        | DEPENDS_ON (external)             | On manifest change |
| Complexity analyser    | Source code                  | (Properties on existing)    | —                                 | On code change     |
| Git history analyser   | Git log                      | Author, Commit              | AUTHORED, CHANGED, CO_CHANGED     | Periodic (daily)   |
| Coverage analyser      | Coverage reports (lcov etc.) | (Properties on existing)    | —                                 | On CI run          |
| Design doc parser      | Mermaid, C4, YAML, ADR       | DesignComponent, DesignRule | DESIGN_CONTAINS, DESIGN_DEPENDS_ON| On doc change      |
| Conformance calculator | Graph DB (internal)          | (Edges)                     | CONFORMS, VIOLATES                | After any update   |

### Incremental Updates

For the service variant (periodic data collection), collectors must support incremental updates — not full re-analysis on every run. The graph DB should support temporal versioning so that historical states can be reconstructed for trend analysis.

### API Design Consideration

The platform API should accept collector output as a standardised event format (e.g., "node created", "edge created", "property updated") rather than collector-specific formats. This enables new collectors to be added without platform changes, and enables third-party tool integration (e.g., SonarQube results pushed via the same API).

---

## Implementation Priorities

### Phase 1: Core comprehension (no design docs required)

1. Structure analyser + dependency analyser → **chord diagram** (static dependencies)
2. Complexity analyser → **treemap** (complexity landscape preset)
3. File system hierarchy → **sunburst** (Mode A, actual structure)
4. Metric pair switching on treemap (complexity, LOC, fan-in/fan-out — all statically derivable)

This phase works for *any* project with source code. No git history, no design docs, no CI integration required.

### Phase 2: Evolution and risk (add git history)

5. Git history analyser → treemap hotspots preset (churn × complexity)
6. Change coupling extraction → chord diagram toggle (static vs. change coupling)
7. Ownership extraction → sunburst coloured by author/team
8. Evolution timeline views
9. Knowledge risk treemap preset

### Phase 3: Quality integration (add CI/test data)

10. Coverage analyser → treemap coverage preset
11. CI integration for periodic report generation
12. Static report generator

### Phase 4: Design conformance

13. Design document parser (Mermaid first, then C4, then structured text)
14. Design-to-code mapping (convention-based + explicit mapping file)
15. Conformance calculator
16. Sunburst Mode B (design structure navigation)
17. Chord diagram conformance overlay
18. Treemap conformance preset
19. Conformance trend tracking

### Phase 5: Advanced

20. Force-directed graph with community detection
21. AI-assisted design-to-code mapping
22. Responsibility conformance (AI-assisted)
23. Runtime telemetry integration
24. Cross-project/cross-service views

---

## Technology Recommendations

### Visualisation layer

**Primary library:** D3.js — provides the required control for all five visualisation types and handles the interactive behaviours (zoom, drill-down, toggle, hover) that are central to the comprehension model.

**Consider ECharts** for the evolution timeline views (calendar heatmap, line charts) where D3's lower-level API adds unnecessary implementation cost.

**For very large codebases (100k+ files):** Evaluate WebGL rendering via deck.gl (for the treemap) or sigma.js (for the force-directed graph). SVG-based D3 will degrade above ~10k rendered elements.

**React integration:** If the dashboard is React-based, use D3 for computation (layouts, scales, force simulations) but React for rendering (DOM management). This avoids the D3-vs-React DOM conflict. Alternatively, use dedicated React wrappers: `nivo` (treemap, sunburst), `vasturiano/sunburst-chart` (sunburst), `recharts` (timelines).

### Graph DB

The prototype already uses a graph DB, which is the right foundation. Ensure the schema supports:
- Temporal versioning (for trend analysis and historical state reconstruction).
- Efficient hierarchical traversal (for treemap and sunburst data extraction).
- Community detection algorithms (for the force-directed graph — Neo4j GDS, TigerGraph, or run Louvain externally).
- Sub-graph pattern matching (for conformance checking — "find all paths that violate this dependency rule").

### Report generation

Server-side rendering of D3 visualisations via Puppeteer (headless Chrome) or node-canvas. Alternatively, generate static SVGs server-side using D3 in Node.js without a browser. Assemble into HTML or PDF (via Puppeteer print-to-PDF or a dedicated PDF library).