/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2026 Alexander Mohr
 */

use crate::{
    app::App,
    tree::{DetailSectionType, NodeType, RowMetadata},
};

impl App {
    /// Handle Enter key for functional class nodes
    pub(super) fn handle_functional_class_enter(&mut self) {
        match self.table.focused_column {
            0 => self.try_navigate_to_service_from_functional_class(),
            5 => self.try_navigate_to_layer_from_functional_class(),
            _ => {}
        }
    }

    /// Navigate to a layer from functional class detail view
    /// The layer name is extracted from the "Layer" column of the selected row
    pub(crate) fn try_navigate_to_layer_from_functional_class(&mut self) {
        if self.tree.cursor >= self.tree.visible.len() {
            return;
        }

        let Some(&node_idx) = self.tree.visible.get(self.tree.cursor) else {
            return;
        };
        let Some(node) = self.tree.all_nodes.get(node_idx) else {
            return;
        };

        // Verify we're on a functional class node
        if !matches!(node.node_type, NodeType::FunctionalClass) {
            self.status = "Not a functional class node".into();
            return;
        }

        // Get the actual section index
        let section_idx = self.get_section_index();

        if section_idx >= node.detail_sections.len() {
            return;
        }

        let Some(section) = node.detail_sections.get(section_idx) else {
            return;
        };

        // We should be in a Services section (the table showing services for this functional class)
        if section.section_type != DetailSectionType::Services {
            self.status = "Not in a services section".into();
            return;
        }

        // Get table rows
        let Some(rows) = section.content.table_rows() else {
            return;
        };

        // Get the selected row cursor
        let row_cursor = if section_idx < self.detail.section_cursors.len() {
            *self.detail.section_cursors.get(section_idx).unwrap_or(&0)
        } else {
            return;
        };

        // Apply sorting if active for this section
        let sorted_rows = self.sort_rows(rows, section_idx);

        if row_cursor >= sorted_rows.len() {
            return;
        }

        let Some(selected_row) = sorted_rows.get(row_cursor) else {
            return;
        };

        // The table has columns: ShortName | Type | SID_RQ | Semantic | Addressing | Layer
        // Layer name is in column 5 (index 5)
        let layer_column_index = 5;

        if selected_row.cells.len() <= layer_column_index {
            self.status = "Invalid row structure".into();
            return;
        }

        let Some(layer_name) = selected_row.cells.get(layer_column_index).cloned() else {
            return;
        };

        // Navigate to the container using the shared helper
        self.navigate_to_container_by_name(&layer_name);
    }

    /// Navigate to a variant from the Variants overview table
    pub(crate) fn try_navigate_to_variant(&mut self) {
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
        let sorted_rows = self.sort_rows(rows, section_idx);

        let Some(selected_row) = sorted_rows.get(row_cursor) else {
            return;
        };

        // Extract the variant name from the first column
        let Some(target_variant_name) = selected_row.cells.first().cloned() else {
            self.status = "Invalid variant row".into();
            return;
        };

        self.navigate_to_container_by_name(&target_variant_name);
    }

    /// Navigate from variant overview to a child element
    pub(crate) fn try_navigate_from_variant_overview(&mut self) {
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

        let Some(section) = node.detail_sections.get(section_idx) else {
            return;
        };

        // Only handle Overview section type
        if section.section_type != DetailSectionType::Overview {
            return;
        }

        // Get table rows
        let Some(rows) = section.content.table_rows() else {
            return;
        };

        // Get the selected row cursor
        let Some(&row_cursor) = self.detail.section_cursors.get(section_idx) else {
            return;
        };

        // Apply sorting if active for this section
        let sorted_rows = self.sort_rows(rows, section_idx);

        let Some(selected_row) = sorted_rows.get(row_cursor) else {
            return;
        };

        // Check if this row is a child element row with metadata
        if let Some(RowMetadata::ChildElement { element_type }) = &selected_row.metadata {
            let element_type = element_type.clone();
            let depth = node.depth;
            self.navigate_to_child_element(node_idx, depth, &element_type);
        }
    }
}
