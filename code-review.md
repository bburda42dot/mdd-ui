# Code Review — mdd-ui

**Date:** 2026-03-01
**Branch:** `feature` (PR #11 — improvements)
**Codebase:** ~15,800 lines across 59 Rust source files

---

## Executive Summary

The codebase is well-structured for a TUI application, with a clean separation between tree building, database reading, rendering, navigation, and input handling. The project follows its own style guide closely — no `unwrap()`/`expect()`/`panic!()` calls, no `unsafe`, proper error handling, and strong use of `saturating_*` arithmetic. The modular decomposition under `app/` (render, navigation, mouse, etc.) is logical and maintainable.

The suggestions below are grouped by severity and area.

---

## 1. Architecture & Design

### 1.1 `App` is a God Object (Medium)

`App` holds **all** application state and has `impl App` blocks spread across 15+ files. While the file-level splitting is good, every method still takes `&mut self` to the entire `App`, which makes it impossible to reason about which sub-states a function actually touches.

**Suggestion:** Consider introducing trait-based delegation or explicit sub-state borrows. For example, navigation methods could accept `(&mut TreeState, &mut HistoryState, &LayoutCache)` rather than `&mut self`. This would:
- Make dependencies explicit
- Allow the borrow checker to catch accidental coupling
- Make future refactoring safer

This is a long-term refactor and not urgent, but worth tracking.

### 1.2 `detail_sections` Is Cloned on Every Frame (Medium)

In `draw_detail()` ([detail.rs](src/app/render/detail.rs#L39-L40)):

```rust
let node_text = selected_node.text.clone();
let detail_sections = selected_node.detail_sections.clone();
```

`detail_sections` contains deeply nested `Vec<DetailRow>` with `Vec<String>` cells. This clone happens **every frame** (~60 fps if the terminal sends events). For large tables this could be a noticeable allocation.

**Suggestion:** Refactor `draw_detail_panes` to accept a reference or an index into `all_nodes` instead of owned data. The clone was likely needed to satisfy the borrow checker (can't borrow `self.tree.all_nodes` immutably while calling `&mut self` methods), which ties back to suggestion 1.1.

### 1.3 Flat Tree with Linear Scans (Low)

The tree is stored as a flat `Vec<TreeNode>` and parent/child relationships are determined by walking backwards looking at `depth`. Several operations do O(n) reverse scans: `is_under_section_type`, `is_dop_category_node`, `find_current_container`, `build_node_path`, `ensure_node_visible`, etc.

For the current data sizes (~thousands of nodes) this is fine, but if the data grows:

**Suggestion:** Consider adding a `parent_idx: Option<usize>` field to `TreeNode`. This would make parent lookups O(1) instead of O(n). The index would need invalidation when `sort_children_by_name` or `sort_diagcomm_nodes_in_place` rearranges nodes, so it's a trade-off.

---

## 2. Correctness Issues

### 2.1 `navigate_to_node` Has Redundant Branches (Low)

In [mouse/mod.rs](src/app/mouse/mod.rs#L368-L400), `navigate_to_node` first checks if the node is in `visible`, then calls `ensure_node_visible`, then finds it again. The `if let Some(_visible_pos)` branch *also* calls `ensure_node_visible` and re-searches. The first `position()` call is wasted work.

**Suggestion:** Simplify to always call `ensure_node_visible` then search once:

```rust
pub(crate) fn navigate_to_node(&mut self, target_node_idx: usize) {
    self.ensure_node_visible(target_node_idx);
    let Some(visible_pos) = self.tree.visible.iter().position(|&idx| idx == target_node_idx) else {
        return;
    };
    self.push_to_history();
    self.focus_state = FocusState::Tree;
    self.tree.cursor = visible_pos;
    self.reset_detail_state();
    self.tree.scroll_offset = self.tree.cursor.saturating_sub(SCROLL_CONTEXT_LINES);
}
```

### 2.2 `detail.focused_section` vs `section_idx` Inconsistency (Medium)

In the detail scrollbar drag handler ([drag.rs](src/app/mouse/drag.rs#L205-L267)), the code uses `self.detail.focused_section` to index into `section_scrolls` and `section_cursors`, but elsewhere the code consistently uses `self.get_section_index()` (which accounts for header offsets). If a node has a PlainText header section, `focused_section` and the actual section index can differ, potentially indexing the wrong section during scrollbar drag.

**Suggestion:** Audit all uses of `self.detail.focused_section` to verify they match the semantics of `get_section_index()`. Consider removing `focused_section` if it always equals `get_section_index()` minus the offset, or document when they diverge.

### 2.3 `sort_diagcomm_nodes_in_place` Modifies Nodes While Iterating by Index (Low)

The method at [sort.rs](src/app/sort.rs#L218-L311) uses `drain()` to remove children, sorts them, then `splice()` them back. Index `i` is updated to skip past the re-inserted block, but the calculation `section_start + (section_end - section_start)` simplifies to `section_end` — which was the *original* end before drain. After drain + splice, the inserted length could change (due to deduplication via `retain`), so `i` could skip too far or not far enough.

**Suggestion:** Recalculate the skip distance using the actual number of re-inserted services:

```rust
let inserted_count = services.len();
// ... splice ...
i = section_start.saturating_add(inserted_count);
```

---

## 3. Style Guide Compliance

### 3.1 `for` Loops That Could Be Iterator Chains

Per the style guide, iterator chains are preferred over `for` loops. Several `for` loops in the tree-building code (e.g., `variants/mod.rs`, `services.rs`, `requests.rs`, `responses.rs`) push to a mutable `Vec` and could be converted to `.filter_map().collect()` or `.flat_map()`.

Examples from `add_variants` in [variants/mod.rs](src/tree/elements/variants/mod.rs#L96):

```rust
for (vi, variant) in variants.iter().enumerate() { ... }
```

These are borderline since the loop body has complex side effects (calling `b.push_details_structured`, `b.add_diag_layer_structured`). Converting them would require significant restructuring and may hurt readability. Recommend leaving the tree-building loops as-is but converting simpler data-transform loops.

### 3.2 Missing `Default` Derives on Sub-State Structs (Low)

`App::new` manually initializes every field of every sub-state struct. Most sub-states already derive `Default`, and the App constructor sets them to default values with minor overrides.

**Suggestion:** Use `..Default::default()` for `App::new`:

```rust
let mut app = Self {
    tree: TreeState {
        all_nodes: nodes,
        diagcomm_sort_by_id: true,
        ..TreeState::default()
    },
    theme,
    ..Self::default()  // if App derives Default
};
```

This requires adding `Default` to `App` (with a dummy `ResolvedTheme::default()`), but reduces ~50 lines of boilerplate in the constructor.

### 3.3 `SearchScope::Display` Inconsistent Capitalization (Nit)

In [mod.rs](src/app/mod.rs#L69-L80), `Display` for `SearchScope`:

```rust
SearchScope::All => write!(f, "All"),           // Title case
SearchScope::Variants => write!(f, "Variants"), // Title case
SearchScope::FunctionalGroups => write!(f, "functional groups"), // lowercase
SearchScope::EcuSharedData => write!(f, "ECU shared data"),      // mixed
```

**Suggestion:** Standardize to consistent capitalization (Title Case for all or lowercase for all).

---

## 4. Duplication & DRY

### 4.1 Repetitive Section Index/Cursor/Scroll Initialization

Across multiple files, this pattern repeats:

```rust
while self.detail.section_scrolls.len() <= section_idx {
    self.detail.section_scrolls.push(0);
}
while self.detail.section_cursors.len() <= section_idx {
    self.detail.section_cursors.push(0);
}
```

This appears in `mouse/mod.rs`, `input.rs`, `column_widths.rs`, `render/detail.rs`, `render/composite.rs`, and various navigation files.

**Suggestion:** Extract a helper like `ensure_detail_capacity(section_idx: usize)` on `DetailState` that grows all parallel vectors to the required size in one call.

### 4.2 `handle_double_click` Duplicates `handle_enter_in_detail_pane` Logic (Medium)

`handle_double_click` in [mouse/mod.rs](src/app/mouse/mod.rs#L196-L281) and `handle_enter_in_detail_pane` in [navigation/mod.rs](src/app/navigation/mod.rs#L18-L93) contain largely the same dispatch logic: check if node is a service list, variant, functional class, DOP, etc., and call the appropriate navigation method.

**Suggestion:** Unify into a single `dispatch_detail_action()` method and call it from both. The only difference is that double-click needs to first handle the table click (positioning the cursor).

### 4.3 `navigate_to_node` Exists in Two Places

`navigate_to_node` is defined in [mouse/mod.rs](src/app/mouse/mod.rs#L363) and `navigate_to_node_by_idx` in [navigation/helpers.rs](src/app/navigation/helpers.rs#L97). They do nearly the same thing (expand ancestors, find visible position, push history, update cursor). The mouse version also calls `ensure_node_visible` redundantly.

**Suggestion:** Remove one and have all callers use a single `navigate_to_node` method. The `navigation/helpers.rs` version is simpler and more correct; move `ensure_node_visible` into it if needed.

---

## 5. Performance

### 5.1 `is_in_subtree` Is O(n) per Node (Low)

`is_in_subtree` ([visibility.rs](src/app/visibility.rs#L51-L66)) checks every node between `root_idx+1` and `node_idx` to verify they all have depth > root. For a subtree of size S at depth D, each call is O(S). When iterating all nodes during search, the total cost could become O(n*S).

**Suggestion:** Pre-compute subtree end boundaries (a one-pass scan) or use the `subtree_range` helper from `navigation/helpers.rs` to only check `node_idx < end`.

### 5.2 `find_in_hierarchy` Collects Parent Ref Names into `Vec<String>` (Low)

In [navigation/helpers.rs](src/app/navigation/helpers.rs#L175-L205), `find_in_hierarchy` allocates a `Vec<String>` of parent ref names, then iterates them searching for containers. This allocates on every navigation action.

**Suggestion:** Use an iterator chain or search-and-break approach instead of collecting first. For example, use `find_map` through the parent refs.

### 5.3 `rebuild_visible` Called Frequently Without Caching

`rebuild_visible()` is called on nearly every interaction (expand, collapse, search, sort, navigate). Each call does a full O(n) pass. Consider adding a dirty flag to skip rebuilds when the tree hasn't changed.

---

## 6. File Organization


### 6.3 `mouse/mod.rs` Has 414 Lines of Mixed Concerns

This file contains event dispatch, double-click logic, `navigate_to_node`, and `ensure_node_visible`. The last two are navigation concerns, not mouse concerns.

**Suggestion:** Move `navigate_to_node` and `ensure_node_visible` to `navigation/helpers.rs`. The mouse module should only translate mouse events into logical actions.

---

## 7. Robustness

### 7.1 `_use_row_selection` Parameter Ignored (Low)

In `get_focused_column` ([parameter.rs](src/app/navigation/parameter.rs#L125-L131)):

```rust
pub(super) fn get_focused_column(
    &self,
    _use_row_selection: bool,
    cell_types: &[CellType],
) -> usize {
```

The `_use_row_selection` parameter is prefixed with `_` and ignored. Per the project style guide: *"Do not prefix parameters with _ to silence warnings. Fix the underlying issue."*

**Suggestion:** Either use the parameter (row selection mode should perhaps always focus column 0) or remove it from the signature.

### 7.2 Magic Column Index `5` in `handle_functional_class_enter` (Medium)

In [navigation/variant.rs](src/app/navigation/variant.rs#L16-L19):

```rust
fn handle_functional_class_enter(&mut self) {
    match self.table.focused_column {
        0 => self.try_navigate_to_service_from_functional_class(),
        5 => self.try_navigate_to_layer_from_functional_class(),
        _ => {}
    }
}
```

Column 5 is a magic number that means "Layer column." If the table schema changes, this silently breaks.

**Suggestion:** Make this data driven instead. No logic should be based on indexes or strings, but should deduce the action from a type attached to the cell

### 7.3 `_ => {}` Catchall in Match Arms

Several places use `_ => {}` in match statements, which the style guide discourages. Examples:

- `handle_functional_class_enter` (above)
- `handle_detail_column_key` in [input.rs](src/app/input.rs#L237-L248) — has `_ => {}` but the outer match already constrains the keys
- Various mouse event handlers

Most of these are actually safe (the outer match narrows the set of possible values), but they could mask future additions. Evaluate on a case-by-case basis.

---

## 8. API Design

### 8.2 `HistoryEntry.node_idx` May Go Stale (Low)

`HistoryEntry` stores both a `node_idx` and a `node_path`. The code tries `node_idx` first and falls back to path-based resolution. However, `sort_diagcomm_nodes_in_place` and `sort_children_by_name` reorder `all_nodes`, silently invalidating stored `node_idx` values in the history.

The path-based fallback handles this, but only when the fast path's staleness is detected by checking `n.text == path.last()`. If a different node happens to have the same text at the old index, it navigates to the wrong node.

**Suggestion:** Invalidate history `node_idx` values when sorting, or add a generation counter to detect stale indices.

---

## 9. Minor Nits

| Location | Issue |
|----------|-------|
| [mod.rs](src/app/mod.rs#L54-L60) | `SearchScope` variants have trailing comments that duplicate the enum doc. Remove the comments or convert to `///` doc comments for consistency. |
| [sort.rs](src/app/sort.rs#L371) | `fn extract_service_id` returns `0` on parse failure. This could sort unrecognized services to the front. Consider returning `u32::MAX` to sort them to the end. |
| [config.rs](src/app/config.rs#L231) | `parse_color` silently falls back to `Color::White` for unrecognized names. A startup warning would help users debug typos in config files. |
| [render/detail.rs](src/app/render/detail.rs) | `build_detail_layout` returns `Vec<Rect>` from `split().to_vec()`. The `to_vec()` is an unnecessary allocation since ratatui returns an `Rc<[Rect]>`. Use indexing directly. |
| [mod.rs](src/app/mod.rs#L426-L432) | `is_dop_category_node` and `is_individual_dop_node` both do reverse walks. They could share the "find parent" logic. |
| [types.rs](src/tree/types.rs#L75) | `TreeNode` does not derive `Debug`. Adding `Debug` would help during development. Consider `#[derive(Clone, Debug)]` (the `detail_sections` field already implements `Debug`). |
| [mouse/clicks.rs](src/app/mouse/clicks.rs#L103) | `HEADER_HEIGHT` is defined as `const` inside two separate functions. Extract to a module-level constant. |

---

## 10. Documentation

### 10.2 Some Helper Functions Lack Context

Functions like `get_section_index`, `get_section_offset`, `get_table_section_idx` have comments explaining *what* they do but not *why* the header offset exists. A brief comment explaining the header-above-tabs pattern would help future contributors.

---

## Summary of Priorities

| Priority | Items |
|----------|-------|
| **High** | 2.2 (focused_section inconsistency), 4.2 (double-click duplication), 4.3 (duplicate navigate_to_node) |
| **Medium** | 1.2 (per-frame clone), 2.1 (redundant navigate_to_node), 2.3 (sort skip calculation), 6.1-6.3 (large files), 7.2 (magic column 5), 3.2 (Default derives) |
| **Low** | 1.3 (flat tree scans), 4.1 (repetitive initialization), 5.1-5.3 (performance), 7.1 (unused parameter), 7.3 (catchall matches), 8.1-8.2 (API design), 9.x (nits) |

Overall the code quality is high for a project of this size. The strict Clippy/fmt configuration and comprehensive lint settings in `Cargo.toml` (`unwrap_used = "deny"`, `indexing_slicing = "deny"`, `arithmetic_side_effects = "deny"`) enforce a solid baseline. The main areas for improvement are reducing duplication in the navigation dispatch logic and splitting the larger files.
