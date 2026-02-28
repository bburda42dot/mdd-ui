/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2026 Alexander Mohr
 */

use crate::{
    app::{App, FocusState},
    tree::{CellType, DetailRow, DetailSectionType, NodeType, TreeNode},
};

impl App {
    /// Navigate from a parameter table (Request/Response) based on the focused cell.
    /// For `DiagComm` Service nodes: `ParameterName` navigates to the counterpart service.
    /// For Request/Response nodes: uses per-cell jump target metadata.
    pub(crate) fn try_navigate_from_param_table(&mut self) {
        // Early validation
        if self.tree.cursor >= self.tree.visible.len() {
            return;
        }

        let Some(&node_idx) = self.tree.visible.get(self.tree.cursor) else {
            return;
        };
        let Some(node) = self.tree.all_nodes.get(node_idx) else {
            return;
        };
        let section_idx = self.get_section_index();

        // Get table data or return early
        let Some((rows, use_row_selection)) = Self::get_table_rows(node, section_idx) else {
            return;
        };

        let row_cursor = self
            .detail
            .section_cursors
            .get(section_idx)
            .copied()
            .unwrap_or(0);
        let sorted_rows = self.apply_table_sort(rows, section_idx);

        let Some(selected_row) = sorted_rows.get(row_cursor) else {
            return;
        };

        // Determine focused column and cell
        let focused_col = self.get_focused_column(use_row_selection, &selected_row.cell_types);
        let cell_type = selected_row
            .cell_types
            .get(focused_col)
            .copied()
            .unwrap_or(CellType::Text);
        let cell_value = selected_row
            .cells
            .get(focused_col)
            .map_or("", std::string::String::as_str);

        if cell_value.is_empty() {
            self.status = "Empty cell".into();
            return;
        }

        // DiagComm Service view: ParameterName navigates to the counterpart
        if matches!(cell_type, CellType::ParameterName)
            && matches!(
                node.node_type,
                NodeType::Service | NodeType::ParentRefService
            )
        {
            self.navigate_to_request_response_counterpart(node_idx, section_idx);
            return;
        }

        // Use per-cell jump target metadata for everything else
        let jump_target = selected_row
            .cell_jump_targets
            .get(focused_col)
            .cloned()
            .flatten();
        self.execute_cell_jump(jump_target, cell_value);
    }

    /// Get table rows from section
    pub(super) fn get_table_rows(
        node: &TreeNode,
        section_idx: usize,
    ) -> Option<(&Vec<DetailRow>, bool)> {
        let section = node.detail_sections.get(section_idx)?;
        let rows = section.content.table_rows()?;
        let use_row_selection = section.content.table_use_row_selection().unwrap_or(false);
        Some((rows, use_row_selection))
    }

    /// Navigate from a `DiagComm` service to the corresponding Request/Response node.
    /// The `DiagComm` node text has a `[Service] ` prefix that the counterpart lacks.
    pub(super) fn navigate_to_request_response_counterpart(
        &mut self,
        node_idx: usize,
        section_idx: usize,
    ) {
        let Some(node) = self.tree.all_nodes.get(node_idx) else {
            self.status = "Invalid node index".into();
            return;
        };

        let service_name = node
            .text
            .strip_prefix("[Service] ")
            .unwrap_or(&node.text)
            .to_owned();

        let section_type = node
            .detail_sections
            .get(section_idx)
            .map(|s| s.section_type);

        let Some(target_node_type) = section_type.and_then(|st| match st {
            DetailSectionType::Requests => Some(NodeType::Request),
            DetailSectionType::PosResponses => Some(NodeType::PosResponse),
            DetailSectionType::NegResponses => Some(NodeType::NegResponse),
            DetailSectionType::Header
            | DetailSectionType::Overview
            | DetailSectionType::Services
            | DetailSectionType::ComParams
            | DetailSectionType::States
            | DetailSectionType::RelatedRefs
            | DetailSectionType::FunctionalClass
            | DetailSectionType::Custom => None,
        }) else {
            self.status = "Cannot navigate from this section type".into();
            return;
        };

        let found = self
            .find_in_hierarchy(|n| n.node_type == target_node_type && n.text == service_name)
            .or_else(|| {
                self.tree
                    .all_nodes
                    .iter()
                    .position(|n| n.node_type == target_node_type && n.text == service_name)
            });

        match found {
            Some(idx) => self.navigate_to_node(idx),
            None => {
                self.status = format!("No matching node found for {service_name}");
            }
        }
    }

    /// Determine which column is focused based on selection mode
    pub(super) fn get_focused_column(
        &self,
        _use_row_selection: bool,
        cell_types: &[CellType],
    ) -> usize {
        self.table
            .focused_column
            .min(cell_types.len().saturating_sub(1))
    }

    /// Navigate to a DOP node by name.
    /// Scopes the search to the current container's subtree first, then
    /// walks up through parent ref containers before falling back to a
    /// global search.
    pub(super) fn navigate_to_dop(&mut self, dop_name: &str) {
        let found_idx = self
            .find_in_hierarchy(|node| node.text == dop_name)
            .or_else(|| {
                self.tree
                    .all_nodes
                    .iter()
                    .position(|node| node.text == dop_name)
            });

        match found_idx {
            Some(dop_idx) => {
                self.navigate_to_node(dop_idx);
                self.status = format!("Navigated to DOP: {dop_name}");
            }
            None => {
                self.status = format!("DOP '{dop_name}' not found in tree");
            }
        }
    }

    /// Navigate to a parameter node by its ID.
    /// Scopes the search to the current node's subtree first to avoid
    /// jumping to a same-named parameter in a different section.
    pub(super) fn navigate_to_parameter_by_id(&mut self, param_id: u32) {
        // Get current node index to scope the search
        let current_node_idx = self.tree.visible.get(self.tree.cursor).copied();

        // Find parameter node by ID, preferring children of the current node
        let param_idx = current_node_idx
            .and_then(|node_idx| self.find_param_in_subtree(node_idx, param_id))
            .or_else(|| self.find_param_by_id(param_id));

        let Some(param_idx) = param_idx else {
            self.status = format!("Parameter with ID {param_id} not found");
            return;
        };

        // Expand ancestors to make parameter visible
        self.ensure_node_visible(param_idx);

        // Navigate to parameter
        if let Some(new_cursor) = self.tree.visible.iter().position(|&idx| idx == param_idx) {
            self.push_to_history();
            self.focus_state = FocusState::Tree;
            self.tree.cursor = new_cursor;
            self.reset_detail_state();
            self.tree.scroll_offset = self.tree.cursor.saturating_sub(5);
            self.status = format!("Navigated to parameter (ID: {param_id})");
        } else {
            self.status = format!("Parameter found but not visible (ID: {param_id})");
        }
    }

    /// Find parameter node by `param_id` within a node's subtree
    pub(super) fn find_param_in_subtree(&self, parent_idx: usize, param_id: u32) -> Option<usize> {
        let parent = self.tree.all_nodes.get(parent_idx)?;
        let parent_depth = parent.depth;

        self.tree
            .all_nodes
            .iter()
            .enumerate()
            .skip(parent_idx.saturating_add(1))
            .take_while(|(_, node)| node.depth > parent_depth)
            .find(|(_, node)| node.param_id == Some(param_id))
            .map(|(idx, _)| idx)
    }

    /// Find parameter node by `param_id`
    pub(super) fn find_param_by_id(&self, param_id: u32) -> Option<usize> {
        self.tree
            .all_nodes
            .iter()
            .position(|node| node.param_id == Some(param_id))
    }

    /// Navigate from DIAG-DATA-DICTIONARY-SPEC or DOP category overview to a child node.
    /// For DIAG-DATA-DICTIONARY-SPEC: rows are categories
    /// like "DTC-DOPS", navigates to the category child node.
    /// For DOP category nodes: rows are individual DOPs, navigates to the DOP child node.
    pub(crate) fn try_navigate_to_dop_child(&mut self) {
        if self.tree.cursor >= self.tree.visible.len() {
            return;
        }

        let Some(&node_idx) = self.tree.visible.get(self.tree.cursor) else {
            return;
        };
        let Some(node) = self.tree.all_nodes.get(node_idx) else {
            return;
        };

        // Get the actual section index
        let section_idx = self.get_section_index();

        if section_idx >= node.detail_sections.len() {
            return;
        }

        let Some(section) = node.detail_sections.get(section_idx) else {
            return;
        };

        // Get table rows
        let Some(rows) = section.content.table_rows() else {
            return;
        };

        // Get the selected row cursor
        let Some(&row_cursor) = self.detail.section_cursors.get(section_idx) else {
            return;
        };

        // Apply sorting if active for this section
        let sorted_rows = self.apply_table_sort(rows, section_idx);

        if row_cursor >= sorted_rows.len() {
            return;
        }

        let Some(selected_row) = sorted_rows.get(row_cursor) else {
            return;
        };

        // Check if the selected row has a DopReference cell — navigate to that DOP
        let dop_ref = selected_row
            .cell_types
            .iter()
            .zip(selected_row.cells.iter())
            .find(|(ct, _)| **ct == CellType::DopReference)
            .map(|(_, cell)| cell.clone());

        if let Some(dop_name) = dop_ref {
            self.navigate_to_dop(&dop_name);
            return;
        }

        // Get the first cell text (category name or DOP name)
        let target_name = match selected_row.cells.first() {
            Some(name) if !name.is_empty() => name.clone(),
            _ => return,
        };

        // Find the child node that matches the target name
        let current_depth = node.depth;
        let target_depth = current_depth.saturating_add(1);

        let target_idx = self
            .tree
            .all_nodes
            .iter()
            .enumerate()
            .skip(node_idx.saturating_add(1))
            .take_while(|(_, child)| child.depth > current_depth)
            .find(|(_, child)| child.depth == target_depth && child.text.starts_with(&target_name))
            .map(|(i, _)| i);

        if let Some(target_node_idx) = target_idx {
            self.navigate_to_node(target_node_idx);
            self.status = format!("Navigated to: {target_name}");
        } else {
            self.status = format!("'{target_name}' not found");
        }
    }
}
