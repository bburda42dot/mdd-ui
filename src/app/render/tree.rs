/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2026 Alexander Mohr
 */

use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::{border_style, expand_icon, render_scrollbar, row_style};
use crate::app::{App, FocusState};

impl App {
    /// Extract ECU name from the General node's detail sections
    fn get_ecu_name(&self) -> &str {
        self.tree
            .all_nodes
            .first()
            .and_then(|node| {
                if node.text != "General" {
                    return None;
                }

                node.detail_sections.first().and_then(|sec| {
                    if let crate::tree::DetailContent::PlainText(lines) = &sec.content {
                        lines.first()?.strip_prefix("ECU Name: ")
                    } else {
                        None
                    }
                })
            })
            .unwrap_or("Tree")
    }

    pub(in crate::app) fn draw_tree(&mut self, frame: &mut Frame, area: Rect) {
        let ecu_name = self.get_ecu_name();

        let tree_block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style(
                self.focus_state != FocusState::Detail,
                &self.theme,
            ))
            .title(format!(" {ecu_name} "));

        let tree_inner = tree_block.inner(area);
        frame.render_widget(tree_block, area);

        // Draw tree content
        let viewport_height = tree_inner.height as usize;
        self.ensure_cursor_visible(viewport_height);

        let lines: Vec<Line> = self
            .tree
            .visible
            .iter()
            .enumerate()
            .skip(self.tree.scroll_offset)
            .take(viewport_height)
            .filter_map(|(vi, &node_idx)| {
                let node = self.tree.all_nodes.get(node_idx)?;
                let row_style = row_style(node, vi == self.tree.cursor, &self.theme);

                let indent = "  ".repeat(node.depth);
                let icon = expand_icon(node);

                Some(Line::from(vec![
                    Span::styled(indent, row_style),
                    Span::styled(icon, row_style),
                    Span::styled(&node.text, row_style),
                ]))
            })
            .collect();

        frame.render_widget(Paragraph::new(lines), tree_inner);
        self.layout.tree_scrollbar_area = render_scrollbar(
            frame,
            area,
            self.tree.visible.len(),
            self.tree.cursor,
            viewport_height,
        );
    }
}
