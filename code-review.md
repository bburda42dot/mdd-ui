<!--
SPDX-License-Identifier: Apache-2.0
SPDX-FileCopyrightText: 2026 Alexander Mohr
-->

# Code Review — Full Codebase

**Date:** 2026-03-01
**Branch:** `feature` (PR #11)

---

## Summary

The codebase is generally well-structured with good error handling — no `unwrap()`/`expect()` in
application logic, proper `?` operator usage, and clean module organization. The main recurring
themes are:

- **For-loops instead of iterator chains** (style guide preference)
- **Clippy `#[allow]` directives** that can be removed by fixing the underlying code
- **One user-facing bug** (double history push on double-click)
- **One unused/broken file** (`dtcs.rs`)
- **One performance concern** (per-frame deep clone of `detail_sections`)

| Severity | Count |
|----------|-------|
| High     | 3     |
| Medium   | 17    |
| Low      | 40+   |

---

## High Severity

### H1. Per-frame deep clone of `detail_sections`

**File:** `src/app/render/detail.rs` ~L38-39

`selected_node.detail_sections.clone()` performs a recursive deep clone of
`Vec<DetailSectionData>` (which contains `Vec<DetailRow>`, etc.) on **every frame render**. For
large nodes with many parameters this creates significant allocation pressure.

**Suggestion:** Use `Rc<[DetailSectionData]>` to make clones cheap, or restructure borrows to
avoid cloning entirely (e.g., index-based lookup to split the borrow from `&mut self`).

---

### H2. Double history push on double-click

**File:** `src/app/mouse/mod.rs` ~L75-83

On a double-click, `handle_click` is invoked first (L77), which calls `push_to_history()` for
detail-area clicks. Then `handle_double_click` (L79) calls `handle_enter_in_detail_pane()` →
`navigate_to_node` → `push_to_history()` again. This pushes **two** history entries for one
navigation action, forcing the user to press Back twice.

**Suggestion:** Either skip `push_to_history` inside `handle_click` when a double-click follows,
or have `handle_double_click` pop the duplicate entry before navigating.

---

### H3. `dtcs.rs` is dead code that won't compile

**File:** `src/tree/elements/dtcs.rs`

This file exists but is never included in the module tree (`elements/mod.rs` has no
`pub mod dtcs;`). It references `SectionType::DTCs` which doesn't exist in the `SectionType`
enum — the file would fail to compile if added.

**Suggestion:** Either finish the implementation (add `DTCs` to `SectionType`, add `pub mod dtcs`,
wire it into the builder) or delete the file.

---

## Medium Severity

### M1. Missing `tab_active_fg` in `ResolvedTheme`

**File:** `src/app/config.rs` ~L62, L140-170

`TableColors` defines `tab_active_fg` (L62) with a default of `"white"` (L117), but
`ResolvedTheme` has no corresponding field and never parses it. The config value is silently
ignored.

**Suggestion:** Add `pub tab_active_fg: Color` to `ResolvedTheme` and parse it in the `From` impl.

---

### M2. `#[allow(clippy::upper_case_acronyms)]` on `NodeType`

**File:** `src/tree/types.rs` ~L42

Suppresses clippy instead of fixing the lint for `DOP` and `SDG` variants. The project
instructions say not to disable clippy warnings.

**Suggestion:** If these must remain all-caps as domain abbreviations, document the exception.
Otherwise rename to `Dop` / `Sdg` and update all usages.

---

### M3. `#[allow(clippy::cast_possible_truncation)]` directives (5 occurrences)

| File | Line | Notes |
|------|------|-------|
| `src/app/column_widths.rs` | ~L188 | Float→int; replace with integer-only arithmetic |
| `src/app/render/tabs.rs` | ~L132 | Use `u16::try_from(...).unwrap_or(u16::MAX)` |
| `src/app/render/tabs.rs` | ~L150 | Same as above |
| `src/app/render/table.rs` | ~L670 | Clamp then cast, or use `try_from` |
| `src/app/mouse/drag.rs` | ~L85 | Value clamped to 20-80; use intermediate `u32` |

**Suggestion:** Replace each `as` cast with safe conversion logic and remove the `#[allow]`.

---

### M4. Potential truncation bug in scroll calculation

**File:** `src/app/mouse/drag.rs` ~L105-106

`as u16` truncates *before* `.min(max_scroll)` is applied. If
`relative_x * max_scroll / (width-1)` exceeds 65535, silent wrapping occurs.

**Suggestion:** Compute `.min()` on the `u32` result first, then cast:
```rust
let result = numerator
    .checked_div(divisor)
    .unwrap_or(0)
    .min(u32::from(max_scroll)) as u16;
```

---

### M5. String comparison for logic

**File:** `src/app/navigation/mod.rs` ~L108

```rust
section.title == "Parent References"
```

The style guide says "Do not use String comparisons for logic. Use Enums or Structs instead."
`DetailSectionType::RelatedRefs` is already available and checked on L107.

**Suggestion:** Remove the string comparison and rely solely on the enum.

---

### M6. `NodeType` should derive `Copy`

**File:** `src/tree/types.rs` ~L40

`NodeType` is an enum of unit variants. Multiple files do `.node_type.clone()` unnecessarily.
Adding `Copy` eliminates these clones.

**Suggestion:** `#[derive(Copy, Clone, Debug, PartialEq, Eq)]`

---

### M7. Returns `&Vec<T>` instead of `&[T]`

**File:** `src/tree/types.rs` ~L281, L290

`table_rows()` returns `Option<&Vec<DetailRow>>` and `table_constraints()` returns
`Option<&Vec<ColumnConstraint>>`.

**Suggestion:** Change to `Option<&[DetailRow]>` and `Option<&[ColumnConstraint]>`.

---

### M8. `if let` instead of `let...else`

**File:** `src/app/render/mod.rs` ~L92-120

The entire `build_breadcrumb_segments` body is wrapped in `if let Some(...)` with an
`else { Vec::new() }`. Should use `let...else`:
```rust
let Some(&node_idx) = self.tree.visible.get(self.tree.cursor) else {
    return Vec::new();
};
```

---

### M9. `App::new` doesn't use `..Default::default()`

**File:** `src/app/mod.rs` ~L345-406

All sub-structs derive `Default` but the constructor explicitly lists every field. Use struct
update syntax per the style guide:
```rust
tree: TreeState {
    all_nodes: nodes,
    diagcomm_sort_by_id: true,
    ..TreeState::default()
},
```

---

### M10. Wildcard match in `toggle_table_column_sort`

**File:** `src/app/sort.rs` ~L349-365

`_ =>` catches both `None` and `Some(state)` where `state.column != column`.

**Suggestion:** Be explicit:
```rust
Some(state) if state.column == column => { /* toggle */ },
Some(_) | None => Some(TableSortState { ... }),
```

---

### M11. Wildcard match in `responses_of!` macro

**File:** `src/tree/elements/variants/responses.rs` ~L52-58

All `DetailSectionType` variants should be listed explicitly. Alternatively, restructure so
`ResponseKind` carries the accessor directly, eliminating the match.

---

### M12. Wildcard match in `try_navigate_parent_ref_from_detail`

**File:** `src/app/navigation/parent_ref.rs` ~L155-161

Match on `DetailSectionType` uses `_ => return` with 15 variants. Enumerate the remaining 11.

---

### M13. `sort_by` allocates in comparator

**File:** `src/tree/elements/variants/state_charts.rs` ~L103, L117

Uses `sort_by` with `.to_lowercase()` allocation per comparison. The rest of the codebase uses
`sort_by_cached_key` for this pattern.

**Suggestion:**
```rust
transitions.sort_by_cached_key(|row| {
    row.cells.first().map(|s| s.to_lowercase())
});
```

---

### M14. Duplicated column-width initialization logic

**Files:** `src/app/render/table.rs` ~L628-632 and `src/app/render/detail.rs` ~L191-197

The `while self.table.column_widths.len() ...` pattern is duplicated.

**Suggestion:** Extract a single `ensure_capacity(section_idx)` method on the table state.

---

### M15. `ensure_*_capacity` uses while-push loops

**File:** `src/app/mod.rs` ~L224-238

Three methods use `while vec.len() <= idx { vec.push(0); }`.

**Suggestion:** Use `Vec::resize`:
```rust
self.section_scrolls.resize(idx.saturating_add(1), 0);
```

---

### M16. Redundant double `.get()` lookups in parent_ref navigation

**File:** `src/app/navigation/parent_ref.rs` ~L112-123

Calls `.get(i)` twice on the same index checking different properties each time.

**Suggestion:** Combine into a single `.get(i).is_some_and(|n| ...conditions...)`.

---

### M17. Repetitive `parent_refs_vec.as_ref().map(...)` pattern (6x)

**File:** `src/tree/elements/layers.rs` ~L63-110

The same expression repeated 6 times.

**Suggestion:** Extract a closure:
```rust
let parent_ref_iter = || parent_refs_vec.as_ref().map(|v| v.iter().cloned());
```

---

## Low Severity

### Style: For-loops → iterator chains

| File | Lines | Description |
|------|-------|-------------|
| `src/app/sort.rs` | ~L83-95 | `expand_all` / `collapse_all` → `.for_each()` |
| `src/app/sort.rs` | ~L67-75 | `try_collapse_or_parent` backward search → `.find()` |
| `src/app/visibility.rs` | ~L24-40 | `is_under_section_type` → `.any()` chain |
| `src/app/history.rs` | ~L22-33 | `build_node_path` → `.scan()` |
| `src/app/mod.rs` | ~L547-559 | `jump_to_matching_row` → `.position()` |
| `src/app/sort.rs` | ~L165-177 | `sort_diagcomm_nodes_in_place` section boundary → `.position()` |
| `src/app/navigation/helpers.rs` | ~L86-98 | `expand_node_ancestors` |
| `src/app/navigation/helpers.rs` | ~L197-216 | `find_in_hierarchy` → `.find_map()` |
| `src/tree/elements/variants/sdgs.rs` | ~L41-72 | Collecting sdg_data → `filter_map().collect()` |
| `src/tree/elements/variants/tables.rs` | ~L35-78 | Outer+inner loops → `flat_map` |
| `src/tree/elements/variants/unit_spec.rs` | ~L33-85 | Triple-nested for-loops |
| `src/tree/elements/variants/services/details.rs` | ~L139 | `rows.extend(iter.map(...))` |
| `src/tree/elements/variants/functional_classes.rs` | ~L121, ~L175 | Map/collect patterns |

---

### Style: `map_or` preference

| File | Lines | Current | Suggested |
|------|-------|---------|-----------|
| `src/app/navigation/parameter.rs` | ~L30-34 | `.copied().unwrap_or(CellType::Text)` | `.map_or(CellType::Text, \|&ct\| ct)` |
| `src/app/navigation/parameter.rs` | ~L35-38 | `.cloned().unwrap_or_default()` | `.map_or_else(<_>::default, Clone::clone)` |
| `src/app/navigation/parameter.rs` | ~L231 | `.first().cloned().unwrap_or_default()` | `.first().map_or_else(...)` |
| `src/app/navigation/mod.rs` | ~L162 | `.get(nav_col).cloned().unwrap_or_default()` | `.get(nav_col).map_or_else(...)` |
| `src/app/navigation/service.rs` | ~L52-54 | `.copied().unwrap_or(0)` | `.map_or(0, \|&c\| c)` |
| `src/app/navigation/parent_ref.rs` | ~L74-76 | `.copied().unwrap_or(0)` | `.map_or(0, \|&c\| c)` |

---

### Quality: Unnecessary clones

| File | Lines | Description |
|------|-------|-------------|
| `src/tree/mod.rs` | ~L34 | `ecu_details.clone()` unnecessary — value not used after |

---

### Quality: Redundant bound checks

| File | Lines | Description |
|------|-------|-------------|
| `src/app/navigation/service.rs` | ~L15-18, L73-76 | Manual bounds check before `.get()` which already returns `None` |
| `src/app/navigation/service.rs` | ~L26-28 | Same pattern for section index |
| `src/app/sort.rs` | ~L16-25 | Double `.get()` then `.get_mut()` on same index |

---

### Quality: Code organization

| File | Description |
|------|-------------|
| `src/app/search.rs` ~L109 | `toggle_mouse_mode` has nothing to do with search — move to `input.rs` |
| `src/tree/elements/variants/placeholders.rs` | Uses fully-qualified paths (`crate::tree::types::DetailRow::normal`) instead of `use` imports |

---

### Quality: Miscellaneous

| File | Lines | Description |
|------|-------|-------------|
| `src/app/sort.rs` | ~L300 | `find('(').is_some()` → use `contains('(')` |
| `src/tree/types.rs` | ~L313-314 | Duplicate doc comment on `table_header()` |
| `src/tree/elements/variants/dops/mod.rs` | ~L169 | Pointless rename: `let unique_dops = all_dops;` |
| `src/tree/elements/variants/dops/mod.rs` | ~L240 | Redundant `!dops.is_empty()` — already filtered |
| `src/tree/elements/variants/mod.rs` | ~L47, L52 | Nested `format!()` → extract local variable |
| `src/app/render/table.rs` | ~L112 | `partial_cmp().unwrap_or(Equal)` — use `total_cmp()` for NaN safety |
| `src/app/column_widths.rs` | ~L186 | `normalize_column_widths` accepts `&mut Vec<u16>` → `&mut [u16]` |
| `src/app/render/composite.rs` | ~L69 | `lines.clone()` for `PlainText` block — borrow instead |

---

### Style: `as` casts that should use `From`/`try_from`

| File | Lines |
|------|-------|
| `src/app/render/tree.rs` | ~L53 |
| `src/app/render/table.rs` | ~L162 |
| `src/app/render/tabs.rs` | ~L53, L91 |
| `src/app/render/detail.rs` | ~L314 |

All are `u16 → usize` which is always safe — use `usize::from(...)` for consistency.

---

### Style: Obvious comments to remove

| File | Lines | Comment text |
|------|-------|-------------|
| `src/app/render/mod.rs` | ~L97, L101, L116 | "Walk up the tree", "Find parent", "Reverse to get root-to-leaf" |
| `src/app/mouse/mod.rs` | ~L93-94 | "Stop dragging when mouse button is released" |
| `src/app/mouse/clicks.rs` | ~L160-164 | "Check if click is in breadcrumb area first", "Click in tree area" |
| `src/tree/elements/variants/mod.rs` | ~L82, L100, L104, L109 | "Add Variants section", "Add each variant", etc. |

---

### Style: `&String` → `&str`

| File | Lines | Description |
|------|-------|-------------|
| `src/app/render/tabs.rs` | ~L65-66 | `Vec<Vec<(usize, &String)>>` → `Vec<Vec<(usize, &str)>>` |

---

### Style: Wildcard matches on small enums

| File | Lines | Description |
|------|-------|-------------|
| `src/app/navigation/parent_ref.rs` | ~L85-88 | `_` in `RowMetadata` match (3 variants) — enumerate explicitly |
| `src/app/render/detail.rs` | ~L160 | `_` in `DetailContent` match (3 variants) |
| `src/app/render/detail.rs` | ~L231 | `_` in tuple match — enumerate cases |
| `src/app/render/table.rs` | ~L114 | `_` in `Result` match — enumerate `Ok`/`Err` combos |
