<!--
SPDX-License-Identifier: Apache-2.0
SPDX-FileCopyrightText: 2026 Alexander Mohr
-->

# Code Review

Reviewed commit: current `feature` branch HEAD (post-refactor).
Files reviewed: all files under `src/`.

Findings are grouped by severity: **Bug**, **Design**, **Performance**, **Style**.

---

## Bugs / Correctness

### B-1 · `toggle_mouse_mode` mutates state before checking for error

**File:** `src/app/input.rs` · `toggle_mouse_mode()`

`self.mouse.enabled` is flipped first, then the `execute!` call is made. If the
`execute!` call returns `Err`, the flag remains in the wrong state — the UI will
say "enabled" but mouse capture is still disabled (or vice versa).

```rust
// current — wrong: flag flipped before confirming success
self.mouse.enabled = !self.mouse.enabled;
let result = if self.mouse.enabled { execute!(..., EnableMouseCapture) }
             else                   { execute!(..., DisableMouseCapture) };
```

Fix: store the desired state, attempt the `execute!`, and only mutate the field
on success:

```rust
let desired = !self.mouse.enabled;
let result = if desired { execute!(..., EnableMouseCapture) }
             else       { execute!(..., DisableMouseCapture) };
if result.is_ok() {
    self.mouse.enabled = desired;
    self.status = format!("Mouse: {}", if desired { "enabled" } else { "disabled" });
} else {
    self.status = "Failed to toggle mouse mode".into();
}
```

---

### B-2 · Breadcrumb hit-testing uses byte length instead of display width

**File:** `src/app/render/mod.rs` · `draw_breadcrumb()`

```rust
let text_len = u16::try_from(text.len()).unwrap_or(u16::MAX);
```

`str::len()` returns the *byte* count, not the display column width.
Any node name containing multi-byte UTF-8 characters (e.g. accented letters,
Japanese, box-drawing characters) will produce incorrect `start_col`/`end_col`
values, causing breadcrumb mouse clicks to resolve to the wrong node.

Fix: use `unicode_width::UnicodeWidthStr::width(&*text)` (or count
`text.chars()` as an approximate fallback if a dependency is not desired):

```rust
let text_len = u16::try_from(text.chars().count()).unwrap_or(u16::MAX);
```

---

### B-3 · `usize::MAX` used as a magic sentinel for "End" navigation

**Files:** `src/app/input.rs` · `handle_detail_navigation()` and `handle_navigation_key()`

Both `*cursor = usize::MAX` and `*scroll = usize::MAX` are written directly into
state vectors on `End` key press, relying on `clamp_section_cursor_and_scroll`
to correct the value on the next render frame. This is fragile because:

1. Any code path that reads the cursor *before* the next draw (bug or future
   feature) will see garbage.
2. Composite scroll uses the same sentinel but goes through a different clamp
   path that may not always fire.

Fix: resolve the final index at the call site (the row count is always
available) and write the real value:

```rust
KeyCode::End => {
    if let Some(cursor) = self.detail.section_cursors.get_mut(section_idx) {
        *cursor = row_count.saturating_sub(1);
    }
}
```

---

### B-4 · `extract_service_id` uses `unwrap_or` against style guide

**File:** `src/app/sort.rs` · `extract_service_id()`

```rust
return u32::from_str_radix(id_str, 16).unwrap_or(u32::MAX);
```

The style guide prohibits `unwrap_or` (prefers `map_or`) and expects `Result`
errors to be handled explicitly, not silently replaced with `u32::MAX`.
`u32::MAX` as a sentinel here also means malformed IDs sort after all real
services, which may hide data quality issues.

Fix:

```rust
return u32::from_str_radix(id_str, 16).map_or(u32::MAX, |id| id);
```

Or, if placing malformed IDs at the end is intentional, document it in a comment.

---

## Design

### D-1 · `build_breadcrumb_segments` returns an opaque `Vec<(String, usize)>`

**File:** `src/app/render/mod.rs`

`build_breadcrumb_segments` returns `Vec<(String, usize)>` and `draw_breadcrumb`
immediately converts each element into a `BreadcrumbSegment`. This is the same
problem that was already solved for the breadcrumb bar: `BreadcrumbSegment`
already exists precisely for this purpose.

```rust
// current — double allocation, caller must know tuple layout
fn build_breadcrumb_segments(&self) -> Vec<(String, usize)> { ... }

// in draw_breadcrumb:
for (i, (text, node_idx)) in segments.iter().enumerate() { ... }
```

Fix: return `Vec<BreadcrumbSegment>` directly (without `start_col`/`end_col`,
which are computed from pixel positions and cannot be known without the `Rect`).
Move the column-position computation into the method by accepting `area: Rect`
as a parameter, or split into a pure path-building function (returns segments
with `node_idx` only) and a separate layout pass that applies positions.

---

### D-2 · `SearchState.stack` uses an opaque tuple

**File:** `src/app/mod.rs`

```rust
pub stack: Vec<(String, SearchScope)>,
```

The same pattern the `BreadcrumbSegment` refactor addressed: a `(String, SearchScope)`
tuple forces every use site to destructure by position. A named struct makes
intent clear at all use sites:

```rust
pub struct SearchEntry {
    pub query: String,
    pub scope: SearchScope,
}
pub stack: Vec<SearchEntry>,
```

---

### D-3 · `TreeBuilder` has four near-identical `push_*` methods

**File:** `src/tree/builder.rs`

`push_details_structured`, `push_param`, `push_service_list_header`, and
`push_section_header` all construct `TreeNode` with the same set of fields,
varying only `section_type`, `service_list_type`, `param_id`, `depth`,
`expanded`, and `node_type`. Each method manually sets the rest to `None` /
`false`.

Consider a single internal `push_node(config: NodeConfig)` (where `NodeConfig`
derives `Default`) and thin wrappers that populate only the relevant fields:

```rust
#[derive(Default)]
struct NodeConfig {
    depth: usize,
    text: String,
    expanded: bool,
    has_children: bool,
    sections: Vec<DetailSectionData>,
    node_type: NodeType,
    section_type: Option<SectionType>,
    service_list_type: Option<ServiceListType>,
    param_id: Option<u32>,
}
```

This eliminates the repeated `section_type: None, service_list_type: None, param_id: None`
boilerplate in every push call.

---

### D-4 · `DetailContent` has four near-identical accessor methods

**File:** `src/tree/types.rs`

`table_rows()`, `table_constraints()`, `table_use_row_selection()`, and
`table_header()` all perform the same three-way match with the same
`Composite`-traversal logic. Any future change to the traversal must be
applied to all four.

Fix: introduce a single private helper that locates the first `Table` variant:

```rust
fn first_table(&self) -> Option<(&DetailRow, &[DetailRow], &[ColumnConstraint], bool)> {
    match self {
        DetailContent::Table { header, rows, constraints, use_row_selection } =>
            Some((header, rows, constraints, *use_row_selection)),
        DetailContent::Composite(subs) =>
            subs.iter().find_map(|s| s.content.first_table()),
        DetailContent::PlainText(_) => None,
    }
}
```

Then each public accessor delegates to `first_table()`.

---

### D-5 · `get_table_section_idx` is a redundant alias

**File:** `src/app/mod.rs`

```rust
fn get_table_section_idx(&self) -> usize {
    self.get_section_index()
}
```

This method does nothing but delegate to `get_section_index`. The name suggests
it may return a different value, which is misleading. Remove the alias and call
`get_section_index()` directly at all 5 call sites.

---

### D-6 · Nested `Composite` sections silently dropped

**File:** `src/app/render/composite.rs` · `build_composite_blocks()`

```rust
DetailContent::Composite(_) => {
    // Nested composites not supported
}
```

A nested composite is silently swallowed with no user-visible indication. This
is an invisible data loss bug for any database that produces nested composites.

Add a `debug_assert!(false, "nested Composite not supported")` or log a warning
so the case is visible during development, and document the limitation in
`tree/types.rs` on the `Composite` variant.

---

### D-7 · `handle_enter_in_detail_pane` and `handle_generic_detail_enter` duplicate guard + node resolution

**File:** `src/app/navigation/mod.rs`

Both methods begin with identical guard code:

```rust
if self.tree.cursor >= self.tree.visible.len() { return; }
let Some(&node_idx) = self.tree.visible.get(self.tree.cursor) else { return; };
let Some(node) = self.tree.all_nodes.get(node_idx) else { return; };
```

`handle_enter_in_detail_pane` calls `handle_generic_detail_enter` in the fall-through
case, so the guard runs twice. Consider extracting the resolution into a helper
that returns `Option<(usize, &TreeNode)>` and calling it once in the outer
function before dispatching.

---


## Performance

### P-1 · `is_under_section_type` is O(N) per call, called inside O(N) search filter

**File:** `src/app/visibility.rs`

`is_under_section_type` walks backwards through all nodes to find an ancestor.
It is called from `node_matches_scope_and_query` which is called for every node
in `apply_search_filter`, making the overall complexity O(N²) for any
scope-filtered search.

Fix: precompute a `Vec<Option<SectionType>>` (one entry per node index,
`None` for nodes that are not inside a top-level section, or the `SectionType`
of their root ancestor) once during `rebuild_visible` or after loading the tree.
The per-node lookup then becomes O(1).

---

### P-2 · Breadcrumb parent traversal is O(N) per ancestor level per frame

**File:** `src/app/render/mod.rs` · `build_breadcrumb_segments()`

For each ancestor step, the code does a reverse linear scan:

```rust
let parent_idx = (0..current_idx).rev().find(|&i| {
    self.tree.all_nodes.get(i).is_some_and(|n| n.depth < current_depth)
});
```

For a node at depth D, this is O(N × D) per rendered frame. With large trees
and deep hierarchies this becomes noticeable.

Fix: store a `parent_idx: Option<usize>` field on `TreeNode` (populated in
`TreeBuilder`) to make parent lookup O(1), or cache the breadcrumb for the
current cursor index and invalidate only on cursor change.

---

### P-3 · `sort_children_by_name` drains and splices the main node vector

**File:** `src/app/sort.rs` · `sort_children_by_name()`

`Vec::drain` + `Vec::splice` move every element after the sorted range twice
(O(N) in the total node count, not just the children count). For a tree with
tens of thousands of nodes and a sorted range near the start, this is expensive.

This is acceptable for an infrequent user action but worth noting if sort
becomes hot (e.g., called during tree building). A segmented internal data
structure (e.g., a slab) would avoid the O(N) shifts.

---

## Style

### S-1 · Repeated six-way cursor-mutation guards in `handle_detail_navigation`

**File:** `src/app/input.rs` · `handle_detail_navigation()`

The pattern

```rust
if let Some(cursor) = self.detail.section_cursors.get_mut(section_idx) {
    *cursor = cursor.saturating_sub(1);
}
```

appears six times in a row (Up, Down, PageUp, PageDown, Home, End) differing
only in the arithmetic. Extract a helper:

```rust
fn adjust_section_cursor(&mut self, section_idx: usize, f: impl Fn(usize) -> usize) {
    if let Some(c) = self.detail.section_cursors.get_mut(section_idx) {
        *c = f(*c);
    }
}
```

Then: `self.adjust_section_cursor(section_idx, |c| c.saturating_sub(1));`

---

### S-2 · `move_up` / `move_down` / `page_up` / `page_down` duplicate the cursor-change + reset pattern

**File:** `src/app/cursor.rs`

All four tree-navigation methods check whether the cursor actually moved and
then call `reset_detail_state`. Extract:

```rust
fn set_tree_cursor(&mut self, new_cursor: usize) {
    if self.tree.cursor != new_cursor {
        self.tree.cursor = new_cursor;
        self.reset_detail_state();
    }
}
```

---

### S-3 · `restore_tab_from_section_type` shadows `sections` with an implicit re-borrow

**File:** `src/app/cursor.rs` · `restore_tab_from_section_type()`

```rust
let mut sections = node.detail_sections.iter().enumerate().filter(...);
// ... first pass using sections via by_title ...
let by_type = sections.find(...); // second pass on same iterator
```

The first pass (`by_title`) creates a new iterator internally, so `sections` is
not actually consumed. However the intent is easy to misread (it looks like
`sections` might already be partially consumed). Create two clearly named
iterators from the same source to remove the ambiguity.

---

### S-4 · `convert_to_absolute_widths` duplicates capacity-growth logic

**File:** `src/app/column_widths.rs`

```rust
while self.table.column_widths_absolute.len() <= section_idx {
    self.table.column_widths_absolute.push(false);
}
```

`TableState::ensure_column_width_capacity` already grows both `column_widths`
and `column_widths_absolute` together. `convert_to_absolute_widths` should call
`ensure_column_width_capacity(section_idx)` instead of re-implementing the
growth inline.

---

### S-5 · Status string is unbounded — may overflow the single-line status bar

**File:** `src/app/mod.rs` and throughout

`self.status` is set to arbitrarily long strings (e.g., a full path or a long
search stack display). The status bar is `Constraint::Length(1)` — ratatui will
clip the text, but long strings may also produce wide Unicode or ANSI sequences
that confuse terminal emulators.

Consider truncating the status string to the terminal width before rendering, or
using `Paragraph::new(...).wrap(Wrap { trim: true })` in the status bar render.

---

### S-6 · `HistoryState::position` is decremented on `pop_front` but not in the simplest way

**File:** `src/app/history.rs` · `push_to_history()`

```rust
self.history.entries.push_back(entry);
self.history.position = self.history.entries.len();
if self.history.entries.len() > MAX_HISTORY {
    self.history.entries.pop_front();
    self.history.position = self.history.entries.len();
}
```

The assignment `self.history.position = self.history.entries.len()` is written
twice. Move the cap logic before the position assignment:

```rust
self.history.entries.push_back(entry);
if self.history.entries.len() > MAX_HISTORY {
    self.history.entries.pop_front();
}
self.history.position = self.history.entries.len();
```

---

### S-7 · `compare_cells` uses `f64` for numeric sorting — precision loss for large integers

**File:** `src/app/render/table.rs` · `compare_cells()`

Parsing as `f64` loses precision for integers larger than 2⁵³ (e.g. long CAN
IDs or large hex addresses). Try `i64`/`u64` first:

```rust
fn compare_cells(a: &DetailRow, b: &DetailRow, col: usize) -> std::cmp::Ordering {
    let a_cell = a.cells.get(col).map_or("", String::as_str);
    let b_cell = b.cells.get(col).map_or("", String::as_str);

    // Try integer comparison first to avoid f64 precision loss
    if let (Ok(ai), Ok(bi)) = (a_cell.parse::<i64>(), b_cell.parse::<i64>()) {
        return ai.cmp(&bi);
    }
    match (a_cell.parse::<f64>(), b_cell.parse::<f64>()) {
        (Ok(af), Ok(bf)) => af.total_cmp(&bf),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        (Err(_), Err(_)) => a_cell.cmp(b_cell),
    }
}
```

---

## Summary Table

| ID  | Severity    | Location                                 | One-line description                                     |
|-----|-------------|------------------------------------------|----------------------------------------------------------|
| B-1 | Bug         | `input.rs` · `toggle_mouse_mode`         | State mutated before confirming syscall success          |
| B-2 | Bug         | `render/mod.rs` · `draw_breadcrumb`      | `text.len()` (bytes) used instead of display width       |
| B-3 | Bug         | `input.rs` · `handle_detail_navigation`  | `usize::MAX` sentinel relies on deferred clamping        |
| B-4 | Style/Bug   | `sort.rs` · `extract_service_id`         | `unwrap_or` violates style guide                         |
| D-1 | Design      | `render/mod.rs`                          | `build_breadcrumb_segments` returns opaque tuple         |
| D-2 | Design      | `mod.rs` · `SearchState`                 | `stack: Vec<(String, SearchScope)>` opaque tuple         |
| D-3 | Design      | `tree/builder.rs`                        | Four near-identical `push_*` methods                     |
| D-4 | Design      | `tree/types.rs` · `DetailContent`        | Four near-identical table accessor methods               |
| D-5 | Design      | `mod.rs`                                 | `get_table_section_idx` is a misleading alias            |
| D-6 | Design      | `render/composite.rs`                    | Nested `Composite` silently dropped                      |
| D-7 | Design      | `navigation/mod.rs`                      | Double node-resolution guard in enter handler            |
| D-8 | Design      | `history.rs` · `resolve_path`            | Text-based path matching ambiguous for duplicate names   |
| P-1 | Performance | `visibility.rs` · `is_under_section_type`| O(N²) during scope-filtered search                       |
| P-2 | Performance | `render/mod.rs` · `build_breadcrumb_segments` | O(N × depth) parent scan per rendered frame         |
| P-3 | Performance | `sort.rs` · `sort_children_by_name`      | O(N) drain+splice over entire node vector                |
| S-1 | Style       | `input.rs` · `handle_detail_navigation`  | Six-way cursor-mutation guard repeated                   |
| S-2 | Style       | `cursor.rs`                              | Cursor-change + reset pattern duplicated four times      |
| S-3 | Style       | `cursor.rs` · `restore_tab_from_section_type` | Shadowed `sections` iterator misleads readers       |
| S-4 | Style       | `column_widths.rs`                       | Capacity-growth logic duplicated from `ensure_*`         |
| S-5 | Style       | throughout                               | Status bar string is unbounded                           |
| S-6 | Style       | `history.rs` · `push_to_history`         | Double position assignment                               |
| S-7 | Style       | `render/table.rs` · `compare_cells`      | `f64` loses precision for large integers                 |
