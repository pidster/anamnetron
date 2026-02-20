# Analysis Depth: Rust Method Call Resolution -- Design

## Goal

Resolve `self.method()` calls inside Rust `impl` blocks by propagating the impl type through the tree-sitter walk. This recovers ~60-70% of currently-unresolved method call edges and improves `must_not_depend`, `boundary`, and `max_fan_in` constraint accuracy.

## Current State

The Rust analyzer cannot resolve method calls (`x.foo()`) because tree-sitter provides no type information for the receiver. All method calls are counted and reported as aggregated warnings (~30 per file). This means:

- `Calls` edges are ~40-50% complete
- `Depends` edges derived from method calls are missing
- Conformance constraints that depend on call/dependency edges have reduced accuracy

## Approach: Impl Type Propagation

**Key insight:** Inside `impl Foo { fn bar(&self) { self.baz() } }`, the receiver `self` is always of type `Foo`. This is the most common Rust pattern for method calls.

### Changes

1. **Add `impl_type: Option<String>` parameter** to the recursive walk functions: `visit_children`, `visit_node`, `visit_mod_item`, `visit_impl_item`, `visit_call_expressions`. This carries the qualified name of the type being impl'd (e.g., `"svt_core::model::Node"`).

2. **Extract impl type in `visit_impl_item`** from `node.child_by_field_name("type")`. Construct the qualified name by combining `module_context` + type name. Pass as `impl_type` when descending into the impl body.

3. **Resolve self.method() in `visit_call_expressions`** — when the function is a `field_expression`, extract the receiver. If receiver is `self`, emit a `Calls` relation: source = current function's qualified name, target = `impl_type::method_name`. Otherwise, continue counting as unresolved.

4. **Parent methods under their type** — when emitting function/method items inside an impl block (where `impl_type` is `Some`), set `parent_qualified_name` to the impl type. This improves the containment hierarchy so methods appear under their type in the graph.

### What This Does NOT Change

- Calls on non-self receivers (`some_var.method()`) remain unresolved
- External trait method dispatch remains unresolved
- No new data structures or types needed
- No changes to other language analyzers

### Error Handling

If `impl_type` is `None` (outside an impl block), method calls remain unresolved as today. No behavior change for non-impl code.

### Testing

- `self_method_call_generates_calls_relation` -- verify `self.foo()` inside `impl Bar` emits `Calls` from current fn to `Bar::foo`
- `non_self_method_call_remains_unresolved` -- verify `x.foo()` still counts as unresolved
- `impl_method_parented_under_type` -- verify methods in impl blocks have parent set to the type
- Update existing `method_call_generates_warning` test to reflect reduced warning count
- Dog-food: verify reduced warning count on self-analysis
