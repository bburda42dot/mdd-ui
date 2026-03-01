/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2026 Alexander Mohr
 */

use ratatui::layout::{Direction, Layout, Position};

use crate::app::{App, COLUMN_SPACING};

/// Check whether a point falls inside an optional area.
fn point_in_optional_rect(area: Option<ratatui::layout::Rect>, column: u16, row: u16) -> bool {
    area.is_some_and(|a| a.contains(Position::new(column, row)))
}

impl App {
    pub(super) fn is_in_tree_area(&self, column: u16, row: u16) -> bool {
        self.layout.tree_area.contains(Position::new(column, row))
    }

    pub(super) fn is_in_detail_area(&self, column: u16, row: u16) -> bool {
        self.layout.detail_area.contains(Position::new(column, row))
    }

    pub(super) fn is_in_tab_area(&self, column: u16, row: u16) -> bool {
        point_in_optional_rect(self.layout.tab_area, column, row)
    }

    pub(super) fn is_in_table_content_area(&self, column: u16, row: u16) -> bool {
        point_in_optional_rect(self.layout.table_content_area, column, row)
    }

    /// Check if the mouse is near the divider between tree and detail panes.
    /// The divider is considered to be within 1-2 columns of the tree pane's right edge.
    pub(super) fn is_near_divider(&self, column: u16) -> bool {
        let divider_col = self
            .layout
            .tree_area
            .x
            .saturating_add(self.layout.tree_area.width);
        column >= divider_col.saturating_sub(1) && column <= divider_col.saturating_add(1)
    }

    pub(super) fn is_in_breadcrumb_area(&self, column: u16, row: u16) -> bool {
        self.layout
            .breadcrumb_area
            .contains(Position::new(column, row))
    }

    /// Check if the mouse is in the tree scrollbar area.
    pub(super) fn is_in_tree_scrollbar(&self, column: u16, row: u16) -> bool {
        point_in_optional_rect(self.layout.tree_scrollbar_area, column, row)
    }

    /// Check if the mouse is in the detail scrollbar area.
    pub(super) fn is_in_detail_scrollbar(&self, column: u16, row: u16) -> bool {
        point_in_optional_rect(self.layout.detail_scrollbar_area, column, row)
    }

    /// Check if the mouse is in the horizontal scrollbar area.
    pub(super) fn is_in_detail_hscrollbar(&self, column: u16, row: u16) -> bool {
        point_in_optional_rect(self.layout.detail_hscrollbar_area, column, row)
    }

    /// Check if the mouse is near a column border in the table header area.
    /// Returns the index of the column to the left of the border.
    pub(super) fn find_column_border(&self, column: u16, row: u16) -> Option<usize> {
        let area = self.layout.table_content_area?;

        // Only detect on the header rows (first 3 rows of the table)
        if row < area.y || row >= area.y.saturating_add(3) {
            return None;
        }

        if self.table.cached_ratatui_constraints.len() < 2 {
            return None;
        }

        let column_areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(self.table.cached_ratatui_constraints.clone())
            .spacing(COLUMN_SPACING)
            .split(area);

        // Check each border between adjacent columns (in the spacing gap)
        // A border exists at the right edge of each column (except the last)
        column_areas
            .iter()
            .enumerate()
            .take(column_areas.len().saturating_sub(1))
            .find(|(_, col_area)| {
                let border = col_area.x.saturating_add(col_area.width);
                // ±2 pixels of the border
                column >= border.saturating_sub(1) && column <= border.saturating_add(3)
            })
            .map(|(idx, _)| idx)
    }
}
