/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2026 Alexander Mohr
 */

use ratatui::layout::{Direction, Layout};

use crate::{
    app::{App, COLUMN_SPACING, DIVIDER_MAX_PCT, DIVIDER_MIN_PCT, DragState},
    tree::DetailContent,
};

impl App {
    /// Handle dragging a column border to resize table columns.
    pub(super) fn handle_column_border_drag(&mut self, col_idx: usize, column: u16) {
        let Some(area) = self.layout.table_content_area else {
            return;
        };

        if self.table.cached_ratatui_constraints.len() < 2 {
            return;
        }

        let column_areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(self.table.cached_ratatui_constraints.clone())
            .spacing(COLUMN_SPACING)
            .split(area);

        let Some(left_area) = column_areas.get(col_idx) else {
            return;
        };

        let section_idx = self.get_section_index();

        // Ensure we're in absolute mode
        let is_absolute = self
            .table
            .column_widths_absolute
            .get(section_idx)
            .copied()
            .unwrap_or(false);
        if !is_absolute {
            self.convert_to_absolute_widths(section_idx);
        }

        // Calculate new width for the dragged column based on mouse position
        let new_width = column.saturating_sub(left_area.x).max(3);

        let current_width = left_area.width;
        if new_width == current_width {
            return;
        }

        if let Some(widths) = self.table.column_widths.get_mut(section_idx)
            && let Some(lw) = widths.get_mut(col_idx)
        {
            *lw = new_width;
        }

        self.table.focused_column = col_idx;
        self.save_column_widths_to_persistent(section_idx);
    }

    /// Handle dragging the divider to resize tree pane.
    pub(super) fn handle_divider_drag(&mut self, column: u16) {
        // Get the total width of main area (tree + detail)
        let total_width = self
            .layout
            .tree_area
            .width
            .saturating_add(self.layout.detail_area.width);
        if total_width == 0 {
            return;
        }

        // Calculate the new tree width based on mouse position
        // The column is relative to the start of tree_area
        let new_tree_width = column.saturating_sub(self.layout.tree_area.x);

        // Calculate percentage (clamped between 20% and 80%)
        let percentage_f32 = (f32::from(new_tree_width) / f32::from(total_width)) * 100.0;
        let clamped = percentage_f32.clamp(f32::from(DIVIDER_MIN_PCT), f32::from(DIVIDER_MAX_PCT));
        // clamped percentage (20..=80) always fits in u16
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let percentage = clamped.round().clamp(0.0, 100.0) as u16;
        self.layout.tree_width_percentage = percentage;
    }

    /// Handle dragging on the horizontal scrollbar.
    pub(super) fn handle_hscrollbar_drag(&mut self, column: u16) {
        let Some(area) = self.layout.detail_hscrollbar_area else {
            return;
        };
        let total_width = self.table.cached_total_table_width;
        let viewport_width = area.width;
        if total_width <= viewport_width {
            return;
        }
        let max_scroll = total_width.saturating_sub(viewport_width);
        let relative_x = column.saturating_sub(area.x);
        let new_scroll = if area.width > 1 {
            let numerator = u32::from(relative_x).saturating_mul(u32::from(max_scroll));
            let divisor = u32::from(area.width.saturating_sub(1));
            #[allow(clippy::cast_possible_truncation)]
            let result = numerator.checked_div(divisor).unwrap_or(0) as u16;
            result.min(max_scroll)
        } else {
            0
        };
        let section_idx = self.get_section_index();
        self.table.ensure_horizontal_scroll_capacity(section_idx);
        if let Some(hs) = self.table.horizontal_scroll.get_mut(section_idx) {
            *hs = new_scroll;
        }
    }

    /// Handle dragging on a scrollbar to scroll.
    pub(super) fn handle_scrollbar_drag(&mut self, row: u16) {
        if !matches!(
            self.mouse.drag_state,
            DragState::TreeScrollbar | DragState::DetailScrollbar
        ) {
            return;
        }

        if self.mouse.drag_state == DragState::TreeScrollbar {
            // Dragging tree scrollbar
            if let Some(area) = self.layout.tree_scrollbar_area {
                let visible_count = self.tree.visible.len();
                let viewport_height = area.height as usize;

                if visible_count <= viewport_height {
                    return;
                }

                // Map mouse position to cursor position in the full list
                let relative_y = row.saturating_sub(area.y) as usize;
                let max_cursor = visible_count.saturating_sub(1);

                let new_cursor = if area.height > 1 {
                    let divisor = usize::from(area.height.saturating_sub(1));
                    relative_y
                        .saturating_mul(max_cursor)
                        .checked_div(divisor)
                        .unwrap_or(0)
                } else {
                    0
                };

                self.tree.cursor = new_cursor.min(max_cursor);

                // Adjust scroll_offset to keep cursor centered in view
                let half_viewport = viewport_height.saturating_div(2);
                self.tree.scroll_offset = self.tree.cursor.saturating_sub(half_viewport);
                let max_scroll = visible_count.saturating_sub(viewport_height);
                self.tree.scroll_offset = self.tree.scroll_offset.min(max_scroll);
            }
        } else {
            // Dragging detail scrollbar
            if let Some(area) = self.layout.detail_scrollbar_area {
                let section_idx = self.get_section_index();

                // Handle composite sections — map drag to composite_scroll
                if self.is_current_section_composite() {
                    let relative_y = usize::from(row.saturating_sub(area.y));
                    let max_scroll = self.detail.composite_max_scroll;
                    self.detail.ensure_composite_capacity(section_idx);
                    let new_scroll = if area.height > 1 {
                        let divisor = usize::from(area.height.saturating_sub(1));
                        relative_y
                            .saturating_mul(max_scroll)
                            .checked_div(divisor)
                            .unwrap_or(0)
                            .min(max_scroll)
                    } else {
                        0
                    };
                    if let Some(scroll) = self.detail.composite_scroll.get_mut(section_idx) {
                        *scroll = new_scroll;
                    }
                    return;
                }

                if self.detail.focused_section >= self.detail.section_scrolls.len() {
                    return;
                }

                // Get the current section's details
                let Some(&node_idx) = self.tree.visible.get(self.tree.cursor) else {
                    return;
                };

                let Some(node) = self.tree.all_nodes.get(node_idx) else {
                    return;
                };
                let sections = &node.detail_sections;
                if self.detail.focused_section >= sections.len() {
                    return;
                }

                let Some(section) = sections.get(self.detail.focused_section) else {
                    return;
                };
                let row_count = match &section.content {
                    DetailContent::Table { rows, .. } => rows.len(),
                    DetailContent::PlainText(lines) => lines.len(),
                    DetailContent::Composite(subs) => subs
                        .iter()
                        .find_map(|s| s.content.table_rows())
                        .map_or(0, Vec::len),
                };
                let viewport_height = area.height as usize;

                if row_count <= viewport_height {
                    return;
                }

                // Map mouse position to cursor position in the section
                let relative_y = row.saturating_sub(area.y) as usize;
                let max_cursor = row_count.saturating_sub(1);

                let new_cursor = if area.height > 1 {
                    let divisor = usize::from(area.height.saturating_sub(1));
                    relative_y
                        .saturating_mul(max_cursor)
                        .checked_div(divisor)
                        .unwrap_or(0)
                } else {
                    0
                };

                let Some(section_cursor) = self
                    .detail
                    .section_cursors
                    .get_mut(self.detail.focused_section)
                else {
                    return;
                };
                *section_cursor = new_cursor.min(max_cursor);

                // Adjust scroll to keep cursor centered
                let half_viewport = viewport_height.saturating_div(2);
                let Some(&cursor_val) =
                    self.detail.section_cursors.get(self.detail.focused_section)
                else {
                    return;
                };
                let new_scroll = cursor_val.saturating_sub(half_viewport);
                let max_scroll = row_count.saturating_sub(viewport_height);
                let Some(section_scroll) = self
                    .detail
                    .section_scrolls
                    .get_mut(self.detail.focused_section)
                else {
                    return;
                };
                *section_scroll = new_scroll.min(max_scroll);
            }
        }
    }
}
