/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2026 Alexander Mohr
 */

use crate::{
    app::{App, FocusState, SCROLL_CONTEXT_LINES},
    tree::{CellType, DetailSectionType, NodeType},
};

impl App {
    /// Navigate from a parameter table (Request/Response) based on the focused cell.
    /// For `DiagComm` Service nodes: `ParameterName` navigates to the counterpart service.
    /// For Request/Response nodes: uses per-cell jump target metadata.
    pub(crate) fn try_navigate_from_param_table(&mut self) {
        let (node_idx, section_idx, node_type, cell_type, cell_value, jump_target) = {
            let Some(ctx) = self.resolve_selected_row() else {
                return;
            };
            let node_type = ctx.node.node_type;
            let Some(selected_row) = ctx.selected_row() else {
                return;
            };
            let focused_col = self.get_focused_column(&selected_row.cell_types);
            let cell_type = selected_row
                .cell_types
                .get(focused_col)
                .map_or(CellType::Text, |&ct| ct);
            let cell_value = selected_row
                .cells
                .get(focused_col)
                .map_or_else(Default::default, Clone::clone);
            let jump_target = selected_row
                .cell_jump_targets
                .get(focused_col)
                .cloned()
                .flatten();
            (
                ctx.node_idx,
                ctx.section_idx,
                node_type,
                cell_type,
                cell_value,
                jump_target,
            )
        };

        if cell_value.is_empty() {
            self.status = "Empty cell".into();
            return;
        }

        if matches!(cell_type, CellType::ParameterName)
            && matches!(node_type, NodeType::Service | NodeType::ParentRefService)
        {
            self.navigate_to_request_response_counterpart(node_idx, section_idx);
            return;
        }

        self.execute_cell_jump(jump_target, &cell_value);
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
            | DetailSectionType::NotInheritedDiagComms
            | DetailSectionType::NotInheritedDops
            | DetailSectionType::NotInheritedTables
            | DetailSectionType::NotInheritedVariables
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

    /// Determine which column is focused, clamped to the available cell count
    pub(super) fn get_focused_column(&self, cell_types: &[CellType]) -> usize {
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
            self.tree.scroll_offset = self.tree.cursor.saturating_sub(SCROLL_CONTEXT_LINES);
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
    pub(crate) fn try_navigate_to_dop_child(&mut self) {
        let (node_idx, current_depth, dop_ref, target_name) = {
            let Some(ctx) = self.resolve_selected_row() else {
                return;
            };
            let Some(selected_row) = ctx.selected_row() else {
                return;
            };

            let dop_ref = selected_row
                .cell_types
                .iter()
                .zip(selected_row.cells.iter())
                .find(|(ct, _)| **ct == CellType::DopReference)
                .map(|(_, cell)| cell.clone());

            let target_name = selected_row
                .cells
                .first()
                .map_or_else(Default::default, Clone::clone);

            (ctx.node_idx, ctx.node.depth, dop_ref, target_name)
        };

        if let Some(dop_name) = dop_ref {
            self.navigate_to_dop(&dop_name);
            return;
        }

        if target_name.is_empty() {
            return;
        }

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
