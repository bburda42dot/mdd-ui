/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2026 Alexander Mohr
 */

mod areas;
mod clicks;
mod drag;

use std::time::{Duration, Instant};

use crossterm::event::{MouseButton, MouseEventKind};

use super::{App, DOUBLE_CLICK_MS, DragState, FocusState, SCROLL_CONTEXT_LINES, input::Action};
use crate::tree::{DetailRowType, DetailSectionType, NodeType};

impl App {
    pub(super) fn handle_mouse_event(
        &mut self,
        kind: MouseEventKind,
        column: u16,
        row: u16,
    ) -> Action {
        // If popup is open, only close on click
        if self.detail.popup.is_some() {
            if matches!(kind, MouseEventKind::Down(_)) {
                self.detail.popup = None;
            }
            return Action::Continue;
        }

        match kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Check if clicking on a scrollbar to start drag
                if self.is_in_tree_scrollbar(column, row) {
                    self.mouse.drag_state = DragState::TreeScrollbar;
                    self.handle_scrollbar_drag(row);
                    return Action::Continue;
                } else if self.is_in_detail_scrollbar(column, row) {
                    self.mouse.drag_state = DragState::DetailScrollbar;
                    self.handle_scrollbar_drag(row);
                    return Action::Continue;
                } else if self.is_in_detail_hscrollbar(column, row) {
                    self.mouse.drag_state = DragState::DetailHScrollbar;
                    self.handle_hscrollbar_drag(column);
                    return Action::Continue;
                }

                // Check if clicking near the divider to start drag
                if self.is_near_divider(column) {
                    self.mouse.drag_state = DragState::Divider;
                    return Action::Continue;
                }

                // Check if clicking near a column border in the table header
                if let Some(col_idx) = self.find_column_border(column, row) {
                    self.mouse.drag_state = DragState::ColumnBorder(col_idx);
                    return Action::Continue;
                }

                // Check for double-click (within threshold and same position)
                let is_double_click = if let Some(last_time) = self.mouse.last_click_time {
                    let elapsed = last_time.elapsed();
                    elapsed < Duration::from_millis(DOUBLE_CLICK_MS)
                        && self.mouse.last_click_pos == (column, row)
                } else {
                    false
                };

                if is_double_click {
                    // First handle the click to update cursor position
                    self.handle_click(column, row);
                    // Then handle the double-click action
                    self.handle_double_click(column, row);
                    // Reset click tracking to avoid triple-click being detected
                    // as another double-click
                    self.mouse.last_click_time = None;
                } else {
                    self.handle_click(column, row);
                    // Track this click for double-click detection
                    self.mouse.last_click_time = Some(Instant::now());
                    self.mouse.last_click_pos = (column, row);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                // Stop dragging when mouse button is released
                self.mouse.drag_state = DragState::None;
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // Handle drag to scroll via scrollbar
                if matches!(
                    self.mouse.drag_state,
                    DragState::TreeScrollbar | DragState::DetailScrollbar
                ) {
                    self.handle_scrollbar_drag(row);
                } else if self.mouse.drag_state == DragState::DetailHScrollbar {
                    self.handle_hscrollbar_drag(column);
                }
                // Handle drag to resize tree pane
                else if self.mouse.drag_state == DragState::Divider {
                    self.handle_divider_drag(column);
                }
                // Handle drag to resize table columns
                else if let DragState::ColumnBorder(col_idx) = self.mouse.drag_state {
                    self.handle_column_border_drag(col_idx, column);
                }
            }
            MouseEventKind::ScrollDown => {
                if self.is_in_tree_area(column, row) {
                    self.move_down();
                } else if self.is_in_detail_area(column, row) {
                    self.focus_state = FocusState::Detail;
                    if self.is_current_section_composite() {
                        let section_idx = self.get_section_index();
                        while self.detail.composite_scroll.len() <= section_idx {
                            self.detail.composite_scroll.push(0);
                        }
                        if let Some(scroll) = self.detail.composite_scroll.get_mut(section_idx) {
                            *scroll = scroll.saturating_add(1);
                        }
                    } else {
                        let section_idx = self.get_section_index();
                        if let Some(cursor) = self.detail.section_cursors.get_mut(section_idx) {
                            *cursor = cursor.saturating_add(3);
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if self.is_in_tree_area(column, row) {
                    self.move_up();
                } else if self.is_in_detail_area(column, row) {
                    self.focus_state = FocusState::Detail;
                    if self.is_current_section_composite() {
                        let section_idx = self.get_section_index();
                        while self.detail.composite_scroll.len() <= section_idx {
                            self.detail.composite_scroll.push(0);
                        }
                        if let Some(scroll) = self.detail.composite_scroll.get_mut(section_idx) {
                            *scroll = scroll.saturating_sub(1);
                        }
                    } else {
                        let section_idx = self.get_section_index();
                        if let Some(cursor) = self.detail.section_cursors.get_mut(section_idx) {
                            *cursor = cursor.saturating_sub(3);
                        }
                    }
                }
            }
            MouseEventKind::ScrollLeft => {
                if self.is_in_detail_area(column, row) {
                    self.focus_state = FocusState::Detail;
                    self.scroll_horizontal(-5);
                }
            }
            MouseEventKind::ScrollRight => {
                if self.is_in_detail_area(column, row) {
                    self.focus_state = FocusState::Detail;
                    self.scroll_horizontal(5);
                }
            }
            // Mouse back button not supported by crossterm 0.29 (only Left/Right/Middle)
            _ => {}
        }
        Action::Continue
    }

    fn handle_click(&mut self, column: u16, row: u16) {
        // Check if click is in breadcrumb area first
        if self.is_in_breadcrumb_area(column, row) {
            self.handle_breadcrumb_click(column);
        } else if self.is_in_tree_area(column, row) {
            // Click in tree area
            self.focus_state = FocusState::Tree;
            self.handle_tree_click(row);
        } else if self.is_in_detail_area(column, row) {
            // Record current position so backspace returns here after a jump
            self.push_to_history();
            self.focus_state = FocusState::Detail;
            // Check if click is on tab area
            if self.is_in_tab_area(column, row) {
                self.handle_tab_click(column, row);
            } else if self.is_in_table_content_area(column, row) {
                self.handle_table_click(column, row);
            }
        }
    }

    fn handle_double_click(&mut self, column: u16, row: u16) {
        const HEADER_HEIGHT: usize = 3;
        // Double-click in table content area should trigger navigation or DOP popup
        if self.is_in_table_content_area(column, row) {
            self.focus_state = FocusState::Detail;

            // Check if double-click is on table header and ignore it
            if let Some(area) = self.layout.table_content_area {
                let relative_row = (row.saturating_sub(area.y)) as usize;
                if relative_row < HEADER_HEIGHT {
                    // Ignore double-clicks on header
                    return;
                }
            }

            // Check what type of node we're on
            if self.tree.cursor < self.tree.visible.len() {
                let Some(&node_idx) = self.tree.visible.get(self.tree.cursor) else {
                    return;
                };
                let Some(node) = self.tree.all_nodes.get(node_idx) else {
                    return;
                };

                // Check if this is a service list header (generic check)
                let is_service_list = Self::is_service_list_section(node);

                // Check if this is the Variants overview section
                let is_variants_section =
                    matches!(node.section_type, Some(crate::tree::SectionType::Variants))
                        && node
                            .detail_sections
                            .first()
                            .is_some_and(|s| s.content.has_table());

                // Check if this is any service-related node type (generic check)
                let is_service_node = matches!(
                    node.node_type,
                    NodeType::Service
                        | NodeType::ParentRefService
                        | NodeType::Request
                        | NodeType::PosResponse
                        | NodeType::NegResponse
                );

                // Check if this is a functional class node
                let is_functional_class = matches!(node.node_type, NodeType::FunctionalClass);

                // Check if this is a DOP node (DIAG-DATA-DICTIONARY-SPEC,
                // DOP category, or individual DOP with children)
                let is_dop_node = matches!(node.node_type, NodeType::DOP)
                    || self.is_dop_category_node(node_idx)
                    || self.is_individual_dop_node(node_idx);

                if is_variants_section {
                    // Navigate to selected variant from the Variants overview table
                    self.try_navigate_to_variant();
                } else if is_service_list {
                    // Navigate to selected service from service list table
                    self.try_navigate_to_service();
                } else if is_functional_class {
                    // For functional class nodes, check which column is focused
                    // Column 0 (ShortName): navigate to service/job
                    // Column 5 (Layer): navigate to variant/layer
                    if self.table.focused_column == 0 {
                        self.try_navigate_to_service_from_functional_class();
                    } else if self.table.focused_column == 5 {
                        self.try_navigate_to_layer_from_functional_class();
                    }
                } else if is_dop_node {
                    // Navigate to child DOP element instead of showing popup
                    self.try_navigate_to_dop_child();
                } else if matches!(node.node_type, NodeType::ParentRefs) {
                    // Navigate to parent ref target from overview
                    self.try_navigate_to_parent_ref();
                } else if is_service_node {
                    self.handle_service_node_double_click(node_idx);
                } else {
                    // Check if we're in a Parent References section
                    let section_idx = self.get_section_index();
                    if let Some(section) = node.detail_sections.get(section_idx) {
                        if section.section_type == DetailSectionType::RelatedRefs
                            && section.title == "Parent References"
                        {
                            self.try_navigate_to_parent_ref();
                            return;
                        } else if section.title.starts_with("Not Inherited") {
                            // Navigate to the selected not-inherited element
                            self.try_navigate_to_not_inherited_element();
                            return;
                        }
                    }

                    self.try_navigate_from_detail_row();
                }
            }
        }
    }

    /// Handle double-click on a service-related node type.
    fn handle_service_node_double_click(&mut self, node_idx: usize) {
        let Some(node) = self.tree.all_nodes.get(node_idx) else {
            return;
        };

        // Check if we're on the "Inherited From" row in Overview
        let mut should_navigate_to_parent = false;
        let mut should_navigate_from_param_table = false;

        // Get the actual section index accounting for header section offset
        let section_idx = self.get_section_index();

        if section_idx < node.detail_sections.len() {
            let Some(section) = node.detail_sections.get(section_idx) else {
                return;
            };

            // Check if this is a request/response parameter table
            if matches!(
                section.section_type,
                DetailSectionType::Requests
                    | DetailSectionType::PosResponses
                    | DetailSectionType::NegResponses
            ) {
                should_navigate_from_param_table = true;
            } else if section.section_type == DetailSectionType::Overview
                && let Some(rows) = section.content.table_rows()
            {
                let row_cursor = self
                    .detail
                    .section_cursors
                    .get(section_idx)
                    .copied()
                    .unwrap_or(0);

                // Apply sorting if active for this section
                let sorted_rows = self.apply_table_sort(rows, section_idx);

                if let Some(selected_row) = sorted_rows.get(row_cursor)
                    && selected_row.row_type == DetailRowType::InheritedFrom
                {
                    should_navigate_to_parent = true;
                }
            }
        }

        if should_navigate_from_param_table {
            self.try_navigate_from_param_table();
        } else if should_navigate_to_parent {
            self.try_navigate_to_inherited_parent();
        } else {
            self.try_navigate_from_detail_row();
        }
    }

    /// Navigate to a specific node by its index in `all_nodes`.
    pub(crate) fn navigate_to_node(&mut self, target_node_idx: usize) {
        // Find the position of this node in visible
        if let Some(_visible_pos) = self
            .tree
            .visible
            .iter()
            .position(|&idx| idx == target_node_idx)
        {
            // Ensure the target node is expanded if needed
            self.ensure_node_visible(target_node_idx);

            // Find the updated position after expanding
            if let Some(new_pos) = self
                .tree
                .visible
                .iter()
                .position(|&idx| idx == target_node_idx)
            {
                self.push_to_history(); // Store old position before jumping
                self.focus_state = FocusState::Tree;
                self.tree.cursor = new_pos;
                self.reset_detail_state();
                self.tree.scroll_offset = self.tree.cursor.saturating_sub(SCROLL_CONTEXT_LINES);
            }
        } else {
            // Node is not currently visible (might be collapsed), try to make it visible
            self.ensure_node_visible(target_node_idx);

            // Try to find it again after expanding parents
            if let Some(visible_pos) = self
                .tree
                .visible
                .iter()
                .position(|&idx| idx == target_node_idx)
            {
                self.push_to_history(); // Store old position before jumping
                self.focus_state = FocusState::Tree;
                self.tree.cursor = visible_pos;
                self.reset_detail_state();
                self.tree.scroll_offset = self.tree.cursor.saturating_sub(SCROLL_CONTEXT_LINES);
            }
        }
    }

    /// Ensure a node is visible by expanding only its direct ancestors.
    pub(crate) fn ensure_node_visible(&mut self, target_node_idx: usize) {
        let Some(target_node) = self.tree.all_nodes.get(target_node_idx) else {
            return;
        };

        let mut needed_depth = target_node.depth;

        // Walk backwards, expanding only the direct ancestor at each level
        for i in (0..target_node_idx).rev() {
            let Some(node) = self.tree.all_nodes.get_mut(i) else {
                continue;
            };
            let node_depth = node.depth;

            if node_depth < needed_depth {
                node.expanded = true;
                needed_depth = node_depth;

                if node_depth == 0 {
                    break;
                }
            }
        }

        // Rebuild visible list to reflect expansions
        self.rebuild_visible();
    }
}
