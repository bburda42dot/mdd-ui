# mdd-ui Code Review


## Medium Priority

### 5. Code Duplication in Response Sections
**Location:** `src/tree/elements/variants/responses.rs`

`add_pos_responses_section` and `add_neg_responses_section` are nearly identical. Similarly, `add_pos_response_service` and `add_neg_response_service` duplicate logic.

**Suggested Improvement:** Extract a generic helper:
```rust
fn add_response_section<'a, F>(
    b: &mut TreeBuilder,
    layer: &DiagLayer<'a>,
    depth: usize,
    variant_parent_refs: Option<impl Iterator<Item = ParentRef<'a>> + 'a>,
    label: &str,
    node_type: NodeType,
    service_list_type: ServiceListType,
    get_responses: F,
) where
    F: Fn(&DiagService<'_>) -> Option<impl Iterator<...>>,
```

---

### 6. Duplicated Cell Comparison Logic
**Location:** `src/app/render.rs` (lines 1024-1035) and `src/app/mod.rs` (lines 486-499)

`compare_cells` in render.rs and `compare_cells_by_column` in mod.rs implement identical logic.

**Suggested Improvement:** Move to a single location (e.g., `sort.rs`) and reuse.

---

### 7. Inconsistent Status Message Setting
**Location:** Multiple files

Status messages are set using `"message".clone_into(&mut self.status)` in some places and `self.status = "message".to_string()` in others.

**Suggested Improvement:** Standardize on one approach:
```rust
// Instead of
"No table data".clone_into(&mut self.status);
// Use
self.status = "No table data".into();
```

---

### 8. Too Many Function Arguments
**Location:** `src/app/render.rs` (lines 1072, 1153, 1201)

Multiple functions take 7-10 parameters, requiring `#[allow(clippy::too_many_arguments)]`.

**Suggested Improvement:** Introduce parameter structs:
```rust
struct TableRenderParams<'a> {
    header: &'a DetailRow,
    rows: &'a [DetailRow],
    constraints: &'a [ColumnConstraint],
    section_idx: usize,
    use_row_selection: bool,
}
```

---

### 9. Magic Numbers
**Location:** Multiple locations

Numbers like `3` (column spacing), `20` (page size), `100` (max history), `5` (scroll offset) are scattered throughout.

**Suggested Improvement:** Define constants:
```rust
const COLUMN_SPACING: u16 = 3;
const PAGE_SIZE: usize = 20;
const MAX_HISTORY_ENTRIES: usize = 100;
const SCROLL_CONTEXT_LINES: usize = 5;
```

---

### 10. `mouse.rs` is Large
**Location:** `src/app/mouse.rs` (1026 lines)

Handles all mouse interactions in one file.

**Suggested Improvement:** Consider splitting by interaction type.

---

## Low Priority

### 11. Missing Documentation
**Location:** Most public types and functions

Types like `TreeNode`, `DetailContent`, `NodeType` lack doc comments explaining their purpose and usage.

**Suggested Improvement:** Add `///` documentation to all public items:
```rust
/// Represents a single node in the tree view.
/// 
/// Contains display text, hierarchy information, and associated detail sections
/// that are shown when the node is selected.
pub struct TreeNode {
    /// Indentation level in the tree hierarchy (0 = root)
    pub depth: usize,
    // ...
}
```

---


### 13. Cloning Rows for Sorting
**Location:** `src/app/mod.rs` (line 473) `apply_table_sort`

`rows.to_vec()` clones all rows even when no sorting is needed.

**Suggested Improvement:** Return `Cow<[DetailRow]>` to avoid cloning when unsorted:
```rust
use std::borrow::Cow;

fn apply_table_sort<'a>(&self, rows: &'a [DetailRow], section_idx: usize) -> Cow<'a, [DetailRow]> {
    let Some(sort_state) = self.table_sort_state.get(section_idx).and_then(|s| s.as_ref()) else {
        return Cow::Borrowed(rows);
    };
    // ... sort and return Cow::Owned(sorted)
}
```

---

### 14. `SearchScope::Subtree` Contains Redundant Data
**Location:** `src/app/mod.rs` (lines 41-45)

`Subtree` stores both `start_idx`, `end_idx`, and `root_name`. The indices could become stale after tree modifications.

**Suggested Improvement:** Store only the root node path (similar to history) and resolve indices dynamically.

---
