<!--
SPDX-License-Identifier: Apache-2.0
SPDX-FileCopyrightText: 2026 Alexander Mohr
-->

# Code Review

## Summary

The codebase is well-structured, follows the style guide closely, and compiles clean under
`cargo +nightly clippy`. The module decomposition is logical and file sizes are reasonable.
The findings below range from bugs/logic issues to style deviations and minor structural notes.

---

## Bugs / Logic Issues

### 1. `compare_cells` mixed-type ordering is inconsistent (`render/table.rs`)

```rust
match (a_cell.parse::<f64>(), b_cell.parse::<f64>()) {
    (Ok(a_num), Ok(b_num)) => a_num.total_cmp(&b_num),
    (Ok(_), Err(_)) | (Err(_), Ok(_) | Err(_)) => a_cell.cmp(b_cell),
}
```

When one cell is numeric and the other is a string, `a_cell.cmp(b_cell)` is used — a
lexicographic comparison of the raw strings (e.g. `"123"` vs `"abc"`). This means the sort
order for mixed columns is non-deterministic depending on which direction the comparison runs.
Explicit branching should make numbers sort before (or after) strings consistently:

```rust
match (a_cell.parse::<f64>(), b_cell.parse::<f64>()) {
    (Ok(a_num), Ok(b_num)) => a_num.total_cmp(&b_num),
    (Ok(_), Err(_)) => std::cmp::Ordering::Less,    // numbers before strings
    (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
    (Err(_), Err(_)) => a_cell.cmp(b_cell),
}
```

### 2. Misleading O(N) comment in `visibility.rs`

```rust
// Pass 2: Include parents using a depth-indexed stack for O(N) total
```

The inner loop `for d in (0..node.depth).rev()` iterates up to `depth` times per node,
making the overall complexity O(N × D) where D is the maximum tree depth. The comment should
be corrected. For the typical shallowness of these trees this is not a performance concern,
but the claim is wrong.

---

## Style Guide Violations

### 3. `as` casts instead of `usize::from` / `u16::from` (`mouse/clicks.rs`)

```rust
let relative_row = (row.saturating_sub(area.y)) as usize;
let available_width = tab_area.width as usize;
```

`u16` fits in `usize` on all supported platforms — use the infallible, self-documenting
conversion:

```rust
let relative_row = usize::from(row.saturating_sub(area.y));
let available_width = usize::from(tab_area.width);
```

### 4. `display_name()` on `ChildElementType` should be `Display` (`tree/types.rs`)

Per the style guide: _"Implement `Display`, Not Custom `to_string()` Methods"_, the same
principle applies to named display helpers:

```rust
// Current
pub fn display_name(&self) -> &'static str { ... }

// Preferred
impl fmt::Display for ChildElementType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self { ... })
    }
}
```

Call sites can then use `element_type.to_string()` or `format!("{element_type}")`, and
`matches_node_text` can use `text.starts_with(&element_type.to_string())`.

### 5. Magic string literals for structural prefixes

The tree-node text format prefixes appear as bare string literals in multiple places:

| Literal | File |
|---|---|
| `"[Service] "` | `navigation/parameter.rs` |
| `"[Job] "` | `navigation/service.rs` |
| `" > "` (breadcrumb sep) | `render/mod.rs` |
| `3` (breadcrumb separator width) | `render/mod.rs` |

These should be done via strongly typed enums instead.
---

## Architecture / Design

### 6. `get_section_index` and `get_section_offset` duplicate the same condition (`app/mod.rs`)

Both functions check:

```rust
if sections.len() > 1
    && first_section.render_as_header
    && matches!(&first_section.content, DetailContent::PlainText(_))
```

and differ only in their return value. Merge into one:

```rust
fn section_offset(&self) -> usize { ... } // returns 0 or 1

fn get_section_index(&self) -> usize {
    self.detail.selected_tab.saturating_add(self.section_offset())
}
```

This eliminates the three-copy divergence and any future risk of the two functions drifting.

### 7. `LayoutCache.breadcrumb_segments` uses an opaque tuple

```rust
pub breadcrumb_segments: Vec<(String, usize, u16, u16)>,
```

The meaning of the four fields is not obvious at a glance. A small named struct would
communicate intent:

```rust
pub struct BreadcrumbSegment {
    pub text: String,
    pub node_idx: usize,
    pub start_col: u16,
    pub end_col: u16,
}
pub breadcrumb_segments: Vec<BreadcrumbSegment>,
```

### 8. `get_ecu_name` in `render/tree.rs` couples rendering to tree structure

```rust
fn get_ecu_name(&self) -> &str {
    self.tree.all_nodes.first()
        .and_then(|node| {
            if node.text != "General" { return None; }
            node.detail_sections.first().and_then(|sec| {
                if let DetailContent::PlainText(lines) = &sec.content {
                    lines.first()?.strip_prefix("ECU Name: ")
                } else { None }
            })
        })
        .unwrap_or("Tree")
}
```

This encodes knowledge of the specific tree layout ("first node must be General", "first
section first line starts with `ECU Name: `") inside the renderer. If `build_tree` changes
the structure, this breaks silently. The ECU name should be stored on `App` directly and set
from `DatabaseData` at construction time.
Furthermore, this _again_ is string based logic which should be replaced with data driven approach

### 9. History `remove(0)` is O(N) — use `VecDeque` (`app/history.rs`)

```rust
if self.history.entries.len() > MAX_HISTORY {
    self.history.entries.remove(0);
    ...
}
```

`Vec::remove(0)` shifts all remaining elements. `MAX_HISTORY = 100` keeps this negligible in
practice, but `VecDeque` is the idiomatic structure for a capped FIFO and removes this
concern entirely:

```rust
use std::collections::VecDeque;
pub entries: VecDeque<HistoryEntry>,
```

### 11. `collapse_all` leaves root node expanded by relying on index (`app/sort.rs`)

```rust
self.tree
    .all_nodes
    .iter_mut()
    .enumerate()
    .filter(|(_, n)| n.has_children)
    .for_each(|(i, n)| n.expanded = i == 0);
```

This keeps the root (index 0) expanded by position rather than by semantic role. If the
tree ever places the root at a different index, or if the meaning of "leave the first one
open" changes, this silently misbehaves. Prefer an explicit predicate:

```rust
.for_each(|(_, n)| n.expanded = n.depth == 0);
```

---

## Minor Notes

### 12. `render/detail.rs` `calculate_header_height` caps at `u16::MAX` before `min()`

```rust
let height = u16::try_from(lines.len())
    .unwrap_or(u16::MAX)   // fallback before the real cap
    .max(1)
    .min(outer_inner.height / 4);
```

The `unwrap_or(u16::MAX)` intermediate is never actually visible after the `.min()`, but
reads as if `u16::MAX` is a valid fallback. A tighter `unwrap_or` makes intent clearer:

```rust
let height = u16::try_from(lines.len())
    .unwrap_or(outer_inner.height / 4)
    .clamp(1, outer_inner.height / 4);
```

### 13. `visibility.rs` `is_under_section_type` is called per-node during search

`node_matches_scope_and_query` calls `is_under_section_type` for every node during search
filtering. `is_under_section_type` itself walks backward through all ancestors (O(depth)).
For the scoped search modes (Variants, FunctionalGroups, etc.) this is called in a tight
loop. Caching the section ancestor for each node (e.g. a `Vec<Option<SectionType>>` parallel
to `all_nodes`) would make filtering O(1) per node.

### 14. `find_in_hierarchy` collects parent ref names into a `Vec<String>` unnecessarily

```rust
let parent_refs_names: Vec<String> = self.tree.all_nodes
    .iter()
    .enumerate()
    ...
    .flat_map(|(pr_idx, pr_node)| { ... })
    .collect();

parent_refs_names.iter().find_map(...)
```

The intermediate `Vec` can be replaced with a single iterator chain (no collect), avoiding
the allocation on the common path.
