# Analysis Depth & Multi-Language Parity — Candidate Milestone Roadmap

**Date:** 2026-07-27
**Status:** Scoping (candidate milestones — not yet greenlit)
**Scope:** Rust method-call resolution, cross-language analyzer parity, and their sequencing relative to data-flow Phase D.

## Purpose

This document records a scoping analysis of two known capability gaps so they can be
weighed as milestones. It is **not** a design doc — it stops at staged increments,
sizes, and the decisions that will require ADRs. The full design step happens per
increment, if and when it is greenlit (`.claude/rules/design-first.md`).

Two gaps are covered:

- **Gap 1** — Rust method-call resolution sits at ~11.7% (dogfood ratchet floor in
  `crates/cli/tests/analysis_depth.rs`).
- **Gap 2** — Go/Python/TS/Java analyzer parity, including the data-flow Phase F
  prerequisites.

They are **not independent**: field extraction (R4/P5) is a shared prerequisite, a
generalised symbol table (P8) is their shared endpoint, and R1 is a Gap-1 fix whose
main beneficiary is data-flow quality.

## Load-bearing findings (verified against code, 2026-07-27)

1. **R1 — method-call edges are attributed to the wrong source node.** The
   `field_expression` arms at `crates/analyzer/src/languages/rust.rs:1990` and `:2004`
   use `build_qualified_name(module_context)` instead of the already-computed
   `caller_qn`. Every `self.m()`/`x.m()` edge is sourced from the enclosing **module**,
   not the calling function. Consequences:
   - `type_flow.rs` requires both endpoints of a `Calls` edge to have a function
     signature (`type_flow.rs:236-244`); a module QN has none, so **every resolved
     method call is invisible to data-flow Phase C**.
   - Entry/sink classification is a fan-in/fan-out heuristic (`core/src/roots.rs:107-115`);
     a function calling ten methods registers `calls_out = 0`. **Phase D built on
     today's graph would produce confidently wrong, plausible-looking entry points.**

2. **`is_project_local()` hardcodes `::` as the QN separator**
   (`crates/analyzer/src/type_flow.rs:355-357`). Even after non-Rust parsers emit type
   metadata, every non-Rust type is judged non-local and dropped → **zero data-flow
   edges for four of five languages**.

3. **The four non-Rust `*_TYPE_CONFIG`s are dead code.** `GO_/TYPESCRIPT_/JAVA_/PYTHON_TYPE_CONFIG`
   in `type_metadata.rs` have zero references outside their defining file. The
   return/param-type extraction machinery is built and tested but never called — so
   Phase F is "wire existing code + fix locality", not "write four parsers".

4. **Instrumentation is buggy but the headline is honest.** `self.m()` success
   increments neither counter, and counters travel as a parsed warning **string**
   (`rust.rs:404-411` → `lib.rs:453-483`). Correcting it moves 11.7% → ~13% — not a
   step change. Chained calls (`x.foo().bar()`) are ~44% of call sites and are the real
   blocker.

5. **PROGRESS.md parity-matrix corrections:** Go's `Depends` row should read *broken*
   (`/`-separated import targets can never match `::`-separated QNs — no import edges
   survive); Python's alias row understates it (`import x as y` is dropped *entirely*);
   "Type registry: Rust ✓" is mis-framed — it is a short-name map used only for impl
   reparenting, and `LanguageParser::post_process` has no `&mut self`, so no parser
   *can* accumulate workspace state without a trait change.

## Gap 1 — staged increments (ordered by value-to-cost)

| ID | Increment | Size | Schema/API impact | Notes |
|----|-----------|------|-------------------|-------|
| R0 | Trustworthy typed instrumentation (per-shape, emitted-vs-surviving) | ~0.5d | none | Prerequisite for measuring everything below |
| R1 | Fix method-call source attribution (use `caller_qn`) | ~0.5d | none | **Correctness gate for Phase D**; unblocks Phase C |
| R2 | Broaden `infer_type_from_value` (`&x`, `x?`, `.await`, `.clone()`, if/match arms); scope `local_type_map` | ~2–3d | none | Targets ~35% simple-receiver bucket; the flat map is a latent correctness bug |
| R3 | Return-type index + single-step chain resolution | ~1wk | none | Attacks ~44% chained bucket; depends on R2 |
| R4 | Field type map (needs field extraction) | ~3–4d | `sub_kind: "field"` nodes | **Shared prerequisite with P5**; node-count inflation |
| R5 | Trait-impl index; `dyn`/generic fan-out | ~1–2wk | edge `resolution_confidence` property → **ADR** | Precision-vs-recall / edge-explosion fork |
| R6 | External-symbol policy (stub nodes vs. drop) | ~3–4d | `External` node kind or marker → affects fan-in/out & roots | Fold into R5's ADR |

**Blocker for any edge-property work (R5):** `AnalysisRelation` has no metadata field.
Every edge property the data-flow design specifies (`mechanism`, `via_function`,
`is_fallible`, `source_type`, `direction`) is unimplemented; `type_flow.rs` computes
direction then discards it. Edge metadata must be plumbed through
`AnalysisRelation` → `mapping.rs` first.

## Gap 2 — staged increments (ordered by value-to-cost)

| ID | Increment | Size | Schema/API impact | Notes |
|----|-----------|------|-------------------|-------|
| P0 | **Phase F**: wire the 4 dead type-configs + separator-aware `is_project_local` | ~5d | none | **Best value-to-cost in either gap.** Turns on data flow for 4 languages |
| P1 | Go import/call target QN normalisation (relation-rewrite pass in `post_process`) | ~2–3d | none | Today Go import edges are guaranteed dangling — a defect |
| P2 | Python import aliases (`import x as y` currently dropped entirely) | ~1–2d | none | Silent data loss |
| P3 | Extends/Implements for Go (embedded types) and Python (superclasses/ABC/Protocol) | ~2–3d each | none (existing edge kinds) | Tighten `go.rs:1126-1146` rather than adding a parallel test |
| P4 | Constructors (Python `__init__` tag; Go `NewFoo` heuristic) | ~1d each | `sub_kind: "constructor"` (precedented) | Go convention needs a confidence decision |
| P5 | Fields (Go/Python) | ~3–4d each | `sub_kind: "field"` nodes | **Shared prerequisite with R4**; largest inflation — sequence after UI collapse story |
| P6 | Enums (Go const/`iota`; Python `Enum` subclass) | ~2d each | enum/variant sub_kinds (precedented) | Lowest value-to-cost |
| P7 | Nested-definition traversal (make top-level loops recursive) | ~1–2d | none | Cheap recall win for everything above |
| P8 | Generalised cross-unit symbol table (QN-keyed) | ~1–2wk | **orchestrator/plugin API change → ADR** | Shared endpoint with Gap 1; needs `&mut self` on the trait. Do not start until R0–R3 / P0–P2 show what it must answer |

## Sequencing relative to data-flow Phase D

Phase D (entry/sink classification + path highlighting) is currently **unimplementable
at useful quality**:

1. **R0 + R1 before anything** (~1d combined) — R1 is a hard **correctness gate**;
   without it Phase D emits confidently-wrong entry points. R0 makes the result
   measurable.
2. **P0 before Phase D** *if Phase D must be multi-language* — it classifies over
   data-flow edges, and four of five languages currently emit zero. This is a
   **coverage gate**.
3. **R2 before Phase D** *if entry/sink quality matters* rather than mere correctness —
   a **quality gate**; resolution rate bounds how much of the call graph Phase D sees.
4. **R3–R6, P3–P8 after Phase D** — recall upside; Phase D's heuristics can be
   validated at ~13–25% resolution and improve automatically as resolution rises.
   Blocking Phase D on chained-call resolution (R3, ~1wk) would be over-sequencing.

**Recommended first increment: R0 + R1 + P0, shipped together.** Two are hours of work;
the third is the largest untapped payoff. Together they establish a trustworthy
baseline, unblock data flow for Rust method calls, turn on data flow for four
languages, and clear the Phase D correctness gate — before any expensive resolution
work is committed.

## Decisions that will require ADRs

- **Trait-object / generic call representation** (R5) — precision vs. recall, edge
  explosion, `resolution_confidence` edge-property semantics; lasting impact on
  conformance rules and Flow View.
- **Cross-unit symbol-table API** (P8) — modifies the orchestrator/plugin boundary,
  which `.claude/rules/architecture.md` designates hard and versioned. Competing
  designs: per-language vs. shared registry; short-name vs. fully-qualified keys.

Everything else (R0–R4, P0–P3, P7) refines existing documented mechanisms and needs
only a design doc or inline design when greenlit — not an ADR.
