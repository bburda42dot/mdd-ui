/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2026 Alexander Mohr
 */

use std::borrow::Cow;

use crate::{
    app::{App, FocusState, SCROLL_CONTEXT_LINES},
    tree::{ChildElementType, DetailRow, DetailSectionData, NodeType, TreeNode},
};

/// Resolved state of the currently selected detail-table row.
/// All references borrow from the owning [`App`], so the context must be
/// dropped before any `&mut self` call.
pub(super) struct SelectedRowContext<'a> {
    pub node_idx: usize,
    pub node: &'a TreeNode,
    pub section_idx: usize,
    pub section: &'a DetailSectionData,
    pub row_cursor: usize,
    pub sorted_rows: Cow<'a, [DetailRow]>,
    pub use_row_selection: bool,
}

impl SelectedRowContext<'_> {
    /// Access the selected row (bounds already verified by [`App::resolve_selected_row`]).
    pub fn selected_row(&self) -> Option<&DetailRow> {
        self.sorted_rows.get(self.row_cursor)
    }
}

impl App {
    /// Resolve the currently selected detail-table row in one step.
    ///
    /// Performs cursor bounds check, node lookup, section lookup,
    /// table-row extraction, row-cursor lookup, and sorting.
    /// Returns `None` when any step fails (cursor out of bounds, no
    /// table data, etc.).
    pub(super) fn resolve_selected_row(&self) -> Option<SelectedRowContext<'_>> {
        let &node_idx = self.tree.visible.get(self.tree.cursor)?;
        let node = self.tree.all_nodes.get(node_idx)?;
        let section_idx = self.get_section_index();
        let section = node.detail_sections.get(section_idx)?;
        let rows = section.content.table_rows()?;
        let use_row_selection = section.content.table_use_row_selection().unwrap_or(false);
        let &row_cursor = self.detail.section_cursors.get(section_idx)?;
        let sorted_rows = self.sort_rows(rows, section_idx);
        if row_cursor >= sorted_rows.len() {
            return None;
        }
        Some(SelectedRowContext {
            node_idx,
            node,
            section_idx,
            section,
            row_cursor,
            sorted_rows,
            use_row_selection,
        })
    }

    /// Expand section and update cursor position
    pub(super) fn expand_and_update_cursor(&mut self, node_idx: usize) {
        if let Some(node_mut) = self.tree.all_nodes.get_mut(node_idx) {
            node_mut.expanded = true;
        }
        self.rebuild_visible();

        if let Some(new_cursor) = self.tree.visible.iter().position(|&idx| idx == node_idx) {
            self.tree.cursor = new_cursor;
        }
    }

    /// Expand all ancestors of a node
    pub(super) fn expand_node_ancestors(&mut self, node_idx: usize) {
        let Some(target_node) = self.tree.all_nodes.get(node_idx) else {
            return;
        };
        let target_depth = target_node.depth;

        if target_depth == 0 {
            return;
        }

        let to_expand: Vec<usize> = (0..node_idx)
            .filter(|&i| {
                self.tree
                    .all_nodes
                    .get(i)
                    .is_some_and(|n| n.depth < target_depth && n.has_children)
            })
            .collect();
        for &i in &to_expand {
            if let Some(n) = self.tree.all_nodes.get_mut(i) {
                n.expanded = true;
            }
        }
    }

    /// Find a container (variant/functional group) by name
    pub(super) fn find_container_by_name(&self, name: &str) -> Option<usize> {
        self.tree.all_nodes.iter().position(|node| {
            if !matches!(node.node_type, NodeType::Container) {
                return false;
            }

            let node_name = node
                .text
                .find(" [")
                .map_or(node.text.as_str(), |idx| &node.text[..idx]);

            node_name == name
        })
    }

    /// Find the containing depth-1 Container node for a given node index
    /// by walking backwards from the node to its nearest Container ancestor.
    pub(super) fn find_current_container(&self, from_node_idx: usize) -> Option<usize> {
        let from_node = self.tree.all_nodes.get(from_node_idx)?;

        // If already a depth-1 container, return it
        if from_node.depth == 1 && matches!(from_node.node_type, NodeType::Container) {
            return Some(from_node_idx);
        }

        // Walk backwards to find the nearest depth-1 Container ancestor
        (0..from_node_idx).rev().find(|&i| {
            self.tree
                .all_nodes
                .get(i)
                .is_some_and(|n| n.depth == 1 && matches!(n.node_type, NodeType::Container))
        })
    }

    /// Find the enclosing top-level section header (depth 0) for a given
    /// node. Returns the node's own index when it is already at depth 0.
    pub(super) fn find_enclosing_section(&self, from_node_idx: usize) -> Option<usize> {
        let from_node = self.tree.all_nodes.get(from_node_idx)?;
        if from_node.depth == 0 {
            return Some(from_node_idx);
        }
        (0..from_node_idx)
            .rev()
            .find(|&i| self.tree.all_nodes.get(i).is_some_and(|n| n.depth == 0))
    }

    /// Get the subtree range (`start_idx` inclusive, `end_idx` exclusive)
    /// for a given node. The subtree includes the node itself and all
    /// children with deeper depth.
    pub(super) fn subtree_range(&self, node_idx: usize) -> (usize, usize) {
        let Some(node) = self.tree.all_nodes.get(node_idx) else {
            return (node_idx, node_idx);
        };
        let node_depth = node.depth;

        let end = self
            .tree
            .all_nodes
            .iter()
            .enumerate()
            .skip(node_idx.saturating_add(1))
            .find(|(_, n)| n.depth <= node_depth)
            .map_or(self.tree.all_nodes.len(), |(i, _)| i);

        (node_idx, end)
    }

    /// Search for a node within a subtree using a predicate.
    pub(super) fn find_in_subtree(
        &self,
        start: usize,
        end: usize,
        predicate: impl Fn(&TreeNode) -> bool,
    ) -> Option<usize> {
        self.tree
            .all_nodes
            .iter()
            .enumerate()
            .skip(start)
            .take(end.saturating_sub(start))
            .find(|(_, node)| predicate(node))
            .map(|(idx, _)| idx)
    }

    /// Search for a node at exactly `target_depth` within a range.
    pub(super) fn find_at_depth(
        &self,
        start: usize,
        end: usize,
        target_depth: usize,
        predicate: &impl Fn(&TreeNode) -> bool,
    ) -> Option<usize> {
        self.tree
            .all_nodes
            .iter()
            .enumerate()
            .skip(start.saturating_add(1))
            .take(end.saturating_sub(start).saturating_sub(1))
            .filter(|(_, n)| n.depth == target_depth)
            .find(|(_, n)| predicate(n))
            .map(|(i, _)| i)
    }

    /// Search parent-ref container subtrees for a matching node.
    /// `start`/`end` delimit the subtree whose parent-ref children to inspect.
    fn find_in_parent_ref_containers(
        &self,
        start: usize,
        end: usize,
        predicate: &impl Fn(&TreeNode) -> bool,
    ) -> Option<usize> {
        self.tree
            .all_nodes
            .iter()
            .enumerate()
            .skip(start.saturating_add(1))
            .take(end.saturating_sub(start).saturating_sub(1))
            .filter(|(_, n)| n.node_type == NodeType::ParentRefs)
            .flat_map(|(pr_idx, pr_node)| {
                let pr_depth = pr_node.depth;
                self.tree
                    .all_nodes
                    .iter()
                    .skip(pr_idx.saturating_add(1))
                    .take_while(move |n| n.depth > pr_depth)
                    .filter(move |n| n.depth == pr_depth.saturating_add(1))
                    .map(|n| {
                        n.text
                            .find(" [")
                            .map_or(n.text.clone(), |idx| n.text[..idx].to_string())
                    })
            })
            .find_map(|parent_name| {
                let (pc_idx, _) = self.tree.all_nodes.iter().enumerate().find(|(_, n)| {
                    matches!(n.node_type, NodeType::Container) && {
                        let name = n
                            .text
                            .find(" [")
                            .map_or(n.text.as_str(), |idx| &n.text[..idx]);
                        name == parent_name
                    }
                })?;
                let (pc_start, pc_end) = self.subtree_range(pc_idx);
                self.find_in_subtree(pc_start, pc_end, predicate)
            })
    }

    /// Search for a node following the database hierarchy — never flat-scan
    /// the tree. The search order mirrors the database structure:
    /// 1. Direct children of the current node (depth + 1).
    /// 2. Full subtree of the current node (deeper descendants).
    /// 3. Enclosing container's subtree (when deeper inside a container).
    /// 4. Parent-ref containers referenced by that container.
    ///
    /// No section-wide or global fallback.
    pub(super) fn find_in_hierarchy(&self, predicate: impl Fn(&TreeNode) -> bool) -> Option<usize> {
        let current_node_idx = self.tree.visible.get(self.tree.cursor).copied()?;
        let current_node = self.tree.all_nodes.get(current_node_idx)?;
        let current_depth = current_node.depth;
        let (node_start, node_end) = self.subtree_range(current_node_idx);

        // 1. Direct children of the current node (depth + 1)
        if let Some(found) = self.find_at_depth(
            node_start,
            node_end,
            current_depth.saturating_add(1),
            &predicate,
        ) {
            return Some(found);
        }

        // 2. Full subtree of the current node (deeper descendants)
        if let Some(found) =
            self.find_in_subtree(node_start.saturating_add(1), node_end, &predicate)
        {
            return Some(found);
        }

        // 3. Enclosing container's subtree (when current node is deeper
        //    than the container). Validate the container belongs to the
        //    same section — find_current_container walks backwards
        //    unconditionally and may return one from a preceding section.
        let section_idx = self.find_enclosing_section(current_node_idx)?;
        let (sec_start, sec_end) = self.subtree_range(section_idx);

        let c_idx = self.find_current_container(current_node_idx)?;
        if c_idx < sec_start || c_idx >= sec_end || c_idx == current_node_idx {
            return None;
        }

        let (c_start, c_end) = self.subtree_range(c_idx);
        if let Some(found) = self.find_in_subtree(c_start, c_end, &predicate) {
            return Some(found);
        }

        // 4. Parent-ref containers
        self.find_in_parent_ref_containers(c_start, c_end, &predicate)
    }

    /// Navigate to a child tree node whose text matches the given `ChildElementType`.
    /// Searches direct children (depth + 1) of the node at `parent_node_idx`.
    pub(super) fn navigate_to_child_element(
        &mut self,
        parent_node_idx: usize,
        parent_depth: usize,
        element_type: &ChildElementType,
    ) {
        let target_depth = parent_depth.saturating_add(1);

        let target_idx = self
            .tree
            .all_nodes
            .iter()
            .enumerate()
            .skip(parent_node_idx.saturating_add(1))
            .take_while(|(_, n)| n.depth > parent_depth)
            .find(|(_, n)| n.depth == target_depth && element_type.matches_node_text(&n.text))
            .map(|(i, _)| i);

        let Some(target_node_idx) = target_idx else {
            self.status = format!("Section '{element_type}' not found");
            return;
        };

        self.ensure_node_visible(target_node_idx);

        let Some(new_cursor) = self
            .tree
            .visible
            .iter()
            .position(|&idx| idx == target_node_idx)
        else {
            return;
        };

        self.push_to_history();
        self.focus_state = FocusState::Tree;
        self.tree.cursor = new_cursor;
        self.reset_detail_state();
        self.tree.scroll_offset = self.tree.cursor.saturating_sub(SCROLL_CONTEXT_LINES);
        self.status = format!("Navigated to: {element_type}");
    }

    /// Navigate to a Container node matching the given short name.
    /// Uses the database hierarchy: direct children first, then section,
    /// then global (needed for cross-section parent-ref navigation).
    pub(super) fn navigate_to_container_by_name(&mut self, target_short_name: &str) {
        let is_target = |n: &TreeNode| -> bool {
            matches!(n.node_type, NodeType::Container) && {
                let name = n
                    .text
                    .find(" [")
                    .map_or(n.text.as_str(), |idx| &n.text[..idx]);
                name == target_short_name
            }
        };

        let current_node_idx = self.tree.visible.get(self.tree.cursor).copied();

        // 1. Direct children of the current node
        let direct_child = current_node_idx.and_then(|idx| {
            let node = self.tree.all_nodes.get(idx)?;
            let (start, end) = self.subtree_range(idx);
            self.find_at_depth(start, end, node.depth.saturating_add(1), &is_target)
        });

        // 2. Section-scoped search
        let section_scoped = || {
            current_node_idx
                .and_then(|idx| self.find_enclosing_section(idx))
                .and_then(|sec_idx| {
                    let (start, end) = self.subtree_range(sec_idx);
                    self.find_in_subtree(start, end, is_target)
                })
        };

        // 3. Global fallback (cross-section parent-ref navigation)
        let global = || self.tree.all_nodes.iter().position(is_target);

        let found = if let Some(idx) = direct_child {
            Some((idx, false))
        } else if let Some(idx) = section_scoped() {
            Some((idx, false))
        } else {
            global().map(|idx| (idx, true))
        };

        let Some((container_node_idx, is_cross_section)) = found else {
            self.status = format!("Element '{target_short_name}' not found in tree");
            return;
        };

        self.navigate_to_node(container_node_idx);
        self.status = format!(
            "Navigated to: {target_short_name}{}",
            if is_cross_section {
                " (cross-section)"
            } else {
                ""
            },
        );
    }

    /// Navigate to a tree node whose text matches the given name.
    /// Scoped to the enclosing section via `find_in_hierarchy`.
    pub(super) fn navigate_to_tree_node_by_text(&mut self, target_name: &str) {
        if let Some(idx) = self.find_in_hierarchy(|node| node.text == target_name) {
            self.navigate_to_node(idx);
            self.status = format!("Navigated to: {target_name}");
        } else {
            self.status = format!("'{target_name}' not found in tree");
        }
    }
}
