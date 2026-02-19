use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Table,
    },
    Frame,
};

use crate::tree::{TreeNode, NodeType};
use crate::tree::{DetailSectionData, DetailRow};

use super::App;

// -----------------------------------------------------------------------
// Colour theme
// -----------------------------------------------------------------------

fn node_style(node: &TreeNode) -> Style {
    match node.node_type {
        NodeType::Ecu => style(Color::Cyan, true),
        NodeType::Container => style(Color::Blue, true),
        NodeType::SectionHeader => style(Color::Yellow, true),
        NodeType::Service => Style::default().fg(Color::White),
        NodeType::ParentRefService => Style::default().fg(Color::DarkGray), // Gray for inherited services
        NodeType::Request => Style::default().fg(Color::White),
        NodeType::PosResponse => Style::default().fg(Color::White),
        NodeType::NegResponse => Style::default().fg(Color::White),
        NodeType::Default => Style::default().fg(Color::White),
    }
}

fn style(fg: Color, bold: bool) -> Style {
    let s = Style::default().fg(fg);
    if bold {
        s.add_modifier(Modifier::BOLD)
    } else {
        s
    }
}

fn border(focused: bool) -> Style {
    Style::default().fg(if focused { Color::Cyan } else { Color::DarkGray })
}

fn row_style(node: &TreeNode, is_cursor: bool) -> Style {
    if is_cursor {
        Style::default().bg(Color::DarkGray).fg(Color::White)
    } else {
        node_style(node)
    }
}

fn expand_icon(node: &TreeNode) -> &'static str {
    if !node.has_children {
        "  "
    } else if node.expanded {
        "▼ "
    } else {
        "▶ "
    }
}

// -----------------------------------------------------------------------
// Drawing
// -----------------------------------------------------------------------

impl App {
    pub(super) fn draw_tree(&mut self, frame: &mut Frame, area: Rect) {
        let tree_block = Block::default()
            .borders(Borders::ALL)
            .border_style(border(!self.detail_focused))
            .title(" Tree ");

        let tree_inner = tree_block.inner(area);
        frame.render_widget(tree_block, area);
        
        // Draw tree content
        let viewport_height = tree_inner.height as usize;
        self.ensure_cursor_visible(viewport_height);

        let lines: Vec<Line> = self
            .visible
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(viewport_height)
            .map(|(vi, &node_idx)| {
                let node = &self.all_nodes[node_idx];
                let row_style = row_style(node, vi == self.cursor);

                let indent = "  ".repeat(node.depth);
                let icon = expand_icon(node);

                Line::from(vec![
                    Span::styled(indent, row_style),
                    Span::styled(icon, row_style),
                    Span::styled(&node.text, row_style),
                ])
            })
            .collect();

        frame.render_widget(Paragraph::new(lines), tree_inner);
        render_scrollbar(frame, area, self.visible.len(), self.scroll_offset, viewport_height);
    }

    pub(super) fn draw_detail(&mut self, frame: &mut Frame, area: Rect) {
        let node_idx = if let Some(&idx) = self.visible.get(self.cursor) {
            idx
        } else {
            let block = Block::default().borders(Borders::ALL).border_style(border(self.detail_focused)).title(" Details ");
            frame.render_widget(block, area);
            return;
        };

        let detail_sections = self.all_nodes[node_idx].detail_sections.clone();

        if !detail_sections.is_empty() {
            self.draw_detail_panes(frame, area, &detail_sections);
        } else {
            let block = Block::default().borders(Borders::ALL).border_style(border(self.detail_focused)).title(" Details ");
            frame.render_widget(block, area);
        }
    }

    pub(super) fn draw_status(&self, frame: &mut Frame, area: Rect) {
        use crate::app::SearchScope;
        
        let (text, st) = if self.searching {
            let scope_indicator = match self.search_scope {
                SearchScope::All => "",
                SearchScope::Variants => " [variants]",
                SearchScope::Services => " [services]",
                SearchScope::DiagComms => " [diag-comms]",
            };
            let current_search_info = if !self.search_stack.is_empty() {
                let stack_display: Vec<String> = self.search_stack.iter()
                    .map(|(term, _scope)| term.clone())
                    .collect();
                format!(" [active: {}]", stack_display.join(" → "))
            } else {
                String::new()
            };
            (
                format!(" /{}█{}{}  (Enter to add, Esc to cancel, Backspace to undo)", self.search, scope_indicator, current_search_info),
                Style::default().fg(Color::Yellow).bg(Color::DarkGray),
            )
        } else if !self.status.is_empty() {
            (format!(" {}", self.status), Style::default().fg(Color::Gray))
        } else {
            let focus = if self.detail_focused { "detail" } else { "tree" };
            
            let scope_indicator = match self.search_scope {
                SearchScope::All => "",
                SearchScope::Variants => " | scope: variants",
                SearchScope::Services => " | scope: services",
                SearchScope::DiagComms => " | scope: diag-comms",
            };
            
            let search_info = if !self.search_stack.is_empty() {
                let stack_display: Vec<String> = self.search_stack.iter()
                    .map(|(term, scope)| {
                        let scope_abbrev = match scope {
                            SearchScope::All => "",
                            SearchScope::Variants => "[V]",
                            SearchScope::Services => "[S]",
                            SearchScope::DiagComms => "[D]",
                        };
                        format!("{}{}", term, scope_abbrev)
                    })
                    .collect();
                format!(" | searches: {}", stack_display.join(" → "))
            } else {
                String::new()
            };
            
            (
                format!(
                    " {}/{} nodes | cursor: {} | focus: {focus}{}{}",
                    self.visible.len(),
                    self.all_nodes.len(),
                    self.cursor + 1,
                    scope_indicator,
                    search_info,
                ),
                Style::default().fg(Color::Gray),
            )
        };
        frame.render_widget(Paragraph::new(text).style(st), area);
    }

    pub(super) fn draw_help_popup(&self, frame: &mut Frame) {
        use ratatui::{
            layout::{Alignment, Rect},
            style::{Color, Style},
            widgets::{Block, Borders, Clear, Paragraph, Wrap},
        };
        
        // Calculate popup size and position (centered, 70% width, 80% height)
        let area = frame.area();
        let popup_width = (area.width * 70) / 100;
        let popup_height = (area.height * 80) / 100;
        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;
        
        let popup_rect = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height,
        };
        
        // Clear the area behind the popup
        frame.render_widget(Clear, popup_rect);
        
        // Draw the popup block
        let block = Block::default()
            .title(" Help - Press ? or Esc to close ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        
        let inner_area = block.inner(popup_rect);
        frame.render_widget(block, popup_rect);
        
        // Help content
        let help_text = vec![
            "NAVIGATION",
            "  ↑/↓ or k/j      Move cursor up/down",
            "  ←/→ or h/l      Collapse/expand node (or navigate tabs in detail)",
            "  PgUp/PgDn       Page up/down",
            "  Home/End        Jump to first/last",
            "  Space           Toggle expand/collapse current node",
            "  Tab             Switch focus between tree and detail pane",
            "",
            "TREE OPERATIONS",
            "  e               Expand all nodes",
            "  c               Collapse all nodes",
            "  s               Toggle DiagComm sort (by ID/name)",
            "",
            "SEARCH & FILTER",
            "  /               Start search (type, then Enter to add to stack)",
            "  Shift+S         Cycle search scope (All/Variants/Services/Diag-Comms)",
            "  Enter           Confirm search and add to stack",
            "  x               Clear all search filters",
            "  Backspace       Remove last search from stack (when search input empty)",
            "  Esc             Cancel current search input",
            "",
            "DETAIL PANE (when focused)",
            "  ↑/↓             Navigate rows in table",
            "  ←/→             Switch between tabs",
            "  Enter           Show DOP popup (if row has DOP reference)",
            "  [ / ]           Decrease/increase column width",
            "  , / .           Select previous/next column",
            "",
            "WINDOW",
            "  + / -           Increase/decrease tree pane width",
            "  m               Toggle mouse mode (enable/disable terminal text selection)",
            "  ?               Show this help",
            "  q or Esc        Quit application",
        ];
        
        let help_paragraph = Paragraph::new(help_text.join("\n"))
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: false })
            .alignment(Alignment::Left);
        
        frame.render_widget(help_paragraph, inner_area);
    }

    pub(super) fn draw_dop_popup(&self, frame: &mut Frame) {
        use ratatui::{
            layout::{Alignment, Rect},
            style::{Color, Style},
            widgets::{Block, Borders, Clear, Paragraph, Wrap},
        };
        
        let Some(popup_data) = &self.dop_popup else { return };
        
        // Calculate popup size and position (centered, 60% width, 50% height)
        let area = frame.area();
        let popup_width = (area.width * 60) / 100;
        let popup_height = (area.height * 50) / 100;
        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;
        
        let popup_rect = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height,
        };
        
        // Clear the area first
        frame.render_widget(Clear, popup_rect);
        
        // Create the popup block
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(format!(" DOP: {} ", popup_data.dop_name))
            .title_alignment(Alignment::Center)
            .title_bottom(" Press Esc to close ")
            .style(Style::default().bg(Color::Black));
        
        let inner = block.inner(popup_rect);
        frame.render_widget(block, popup_rect);
        
        // Render the content
        let content_text = popup_data.dop_details.join("\n");
        let paragraph = Paragraph::new(content_text)
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: true });
        
        frame.render_widget(paragraph, inner);
    }

    fn draw_detail_panes(&mut self, frame: &mut Frame, area: Rect, sections: &[DetailSectionData]) {
        use ratatui::{
            layout::{Constraint, Direction, Layout},
            widgets::Tabs,
        };

        if sections.is_empty() {
            return;
        }

        // Clamp selected_tab to valid range
        if self.selected_tab >= sections.len() {
            self.selected_tab = sections.len().saturating_sub(1);
        }

        // Ensure section_scrolls and section_cursors have enough entries
        while self.section_scrolls.len() < sections.len() {
            self.section_scrolls.push(0);
        }
        while self.section_cursors.len() < sections.len() {
            self.section_cursors.push(0);
        }

        let show_tabs = sections.len() > 1;
        let (tab_area, content_area) = if show_tabs {
            // Split area into tabs bar and content
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Tab bar height
                    Constraint::Min(0),    // Content area
                ])
                .split(area);
            (Some(chunks[0]), chunks[1])
        } else {
            (None, area)
        };

        // Cache tab area and titles for mouse handling
        self.tab_area = tab_area;
        if show_tabs {
            self.tab_titles = sections.iter().map(|s| s.title.clone()).collect();
        } else {
            self.tab_titles.clear();
        }

        // Render tabs if there are multiple sections
        if let Some(tab_area) = tab_area {
            let tab_titles: Vec<String> = sections.iter().map(|s| s.title.clone()).collect();
            let tabs = Tabs::new(tab_titles)
                .block(Block::default().borders(Borders::ALL).border_style(border(self.detail_focused)))
                .select(self.selected_tab)
                .style(Style::default().fg(Color::Gray))
                .highlight_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                );
            frame.render_widget(tabs, tab_area);
        }

        // Render the selected tab's content
        let section = &sections[self.selected_tab];
        let help_text = if self.detail_focused { 
            " h/l:tabs  j/k:row  ,/.:column  [/]:resize  Enter:DOP  Esc:close " 
        } else { 
            "" 
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border(self.detail_focused))
            .title_bottom(help_text);

        let inner = block.inner(content_area);
        frame.render_widget(block, content_area);

        // Cache the inner area for table content clicking
        self.table_content_area = Some(inner);

        // Handle different content types
        use crate::tree::DetailContent;
        match &section.content {
            DetailContent::PlainText(lines) => {
                let text = lines.join("\n");
                let para = Paragraph::new(text).style(Style::default().fg(Color::White));
                frame.render_widget(para, inner);
            }
            DetailContent::Table { header, rows, constraints } => {
                self.render_table_content(frame, inner, content_area, header, rows, constraints, self.selected_tab);
            }
            DetailContent::Composite(subsections) => {
                self.render_composite_content(frame, inner, subsections);
            }
        }
    }

    fn render_composite_content(&mut self, frame: &mut Frame, area: Rect, subsections: &[crate::tree::DetailSectionData]) {
        use ratatui::{
            layout::{Constraint, Direction, Layout},
            widgets::Block,
        };
        
        if subsections.is_empty() {
            return;
        }
        
        // Create vertical layout for subsections
        let subsection_count = subsections.len();
        let constraints: Vec<Constraint> = (0..subsection_count)
            .map(|_| Constraint::Percentage(100 / subsection_count as u16))
            .collect();
        
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);
        
        // Render each subsection in its own box
        for (i, subsection) in subsections.iter().enumerate() {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(format!(" {} ", subsection.title));
            
            let inner = block.inner(chunks[i]);
            frame.render_widget(block, chunks[i]);
            
            // Render the content of the subsection
            match &subsection.content {
                crate::tree::DetailContent::PlainText(lines) => {
                    let text = lines.join("\n");
                    let para = Paragraph::new(text).style(Style::default().fg(Color::White));
                    frame.render_widget(para, inner);
                }
                crate::tree::DetailContent::Table { header, rows, constraints } => {
                    // For composite tables, we don't track cursors/scrolling per subsection yet
                    // This is a simplified rendering
                    self.render_simple_table(frame, inner, header, rows, constraints);
                }
                crate::tree::DetailContent::Composite(_) => {
                    // Nested composites not supported
                    let text = "(Nested composites not supported)";
                    let para = Paragraph::new(text).style(Style::default().fg(Color::Red));
                    frame.render_widget(para, inner);
                }
            }
        }
    }
    
    fn render_simple_table(&self, frame: &mut Frame, area: Rect, header: &DetailRow, rows: &[DetailRow], constraints: &[crate::tree::ColumnConstraint]) {
        let max_columns = rows.iter().map(|r| r.cells.len()).max().unwrap_or(header.cells.len());
        
        // Convert constraints to ratatui Constraints
        let mut ratatui_constraints: Vec<Constraint> = constraints.iter()
            .map(|c| match c {
                crate::tree::ColumnConstraint::Fixed(w) => Constraint::Length(*w),
                crate::tree::ColumnConstraint::Percentage(p) => Constraint::Percentage(*p),
            })
            .collect();
        
        // Ensure we have enough constraints
        while ratatui_constraints.len() < max_columns {
            ratatui_constraints.push(Constraint::Percentage(10));
        }
        
        // Create header
        let header_cells: Vec<Cell> = header.cells.iter().map(|c| {
            Cell::from(c.as_str()).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        }).collect();
        let header_row = Row::new(header_cells);
        
        // Create data rows
        let data_rows: Vec<Row> = rows.iter().map(|row_data| {
            let indent_str = "  ".repeat(row_data.indent / 2);
            let mut cells: Vec<Cell> = row_data.cells.iter().enumerate().map(|(col_idx, cell_text)| {
                let text = if col_idx == 0 { 
                    format!("{}{}", indent_str, cell_text) 
                } else { 
                    cell_text.clone() 
                };
                Cell::from(text).style(Style::default().fg(Color::White))
            }).collect();
            
            while cells.len() < max_columns { 
                cells.push(Cell::from("")); 
            }
            Row::new(cells)
        }).collect();
        
        let table = Table::new(data_rows, ratatui_constraints)
            .column_spacing(1)
            .header(header_row);
        frame.render_widget(table, area);
    }

    fn render_table_content(&mut self, frame: &mut Frame, inner: Rect, area: Rect, header: &DetailRow, rows: &[DetailRow], constraints: &[crate::tree::ColumnConstraint], section_idx: usize) {
        use ratatui::text::Text;
        
        let viewport_height = inner.height as usize;
        let max_columns = rows.iter().map(|r| r.cells.len()).max().unwrap_or(header.cells.len());

        let rows_refs: Vec<&DetailRow> = rows.iter().collect();

        let row_count = rows.len();
        if row_count > 0 && self.section_cursors[section_idx] >= row_count {
            self.section_cursors[section_idx] = row_count.saturating_sub(1);
        }

        if row_count > 0 {
            let cursor_pos = self.section_cursors[section_idx];
            if cursor_pos < self.section_scrolls[section_idx] {
                self.section_scrolls[section_idx] = cursor_pos;
            } else if cursor_pos >= self.section_scrolls[section_idx] + viewport_height {
                self.section_scrolls[section_idx] = cursor_pos.saturating_sub(viewport_height).saturating_add(1);
            }
        }

        if row_count > viewport_height && self.section_scrolls[section_idx] >= row_count.saturating_sub(viewport_height) {
            self.section_scrolls[section_idx] = row_count.saturating_sub(viewport_height);
        }

        let focused_col = if self.focused_column >= max_columns { max_columns.saturating_sub(1) } else { self.focused_column };
        
        // Build visible rows with column-specific highlighting
        let visible_rows: Vec<Row<'static>> = rows_refs.iter().enumerate()
            .skip(self.section_scrolls[section_idx])
            .take(viewport_height)
            .map(|(idx, row_data)| {
                let indent_str = "  ".repeat(row_data.indent / 2);
                let is_selected_row = self.detail_focused && idx == self.section_cursors[section_idx];
                
                let mut cells: Vec<Cell> = row_data.cells.iter().enumerate().map(|(col_idx, cell_text)| {
                    let text = if col_idx == 0 { 
                        format!("{}{}", indent_str, cell_text) 
                    } else { 
                        cell_text.clone() 
                    };
                    
                    // Apply column-specific highlighting for selected cell
                    let style = if is_selected_row && col_idx == focused_col {
                        // Selected cell: highlight with blue background, keep white text
                        Style::default().fg(Color::White).bg(Color::Blue).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    
                    Cell::from(text).style(style)
                }).collect();
                
                while cells.len() < max_columns { 
                    cells.push(Cell::from("")); 
                }
                Row::new(cells)
            }).collect();

        // Get column widths for this section, or use defaults from constraints
        let column_widths = self.get_column_widths(section_idx, constraints);
        
        // Convert to ratatui Constraint using custom widths
        let ratatui_constraints: Vec<Constraint> = column_widths.iter()
            .map(|&w| Constraint::Percentage(w))
            .collect();
        
        // Cache the exact constraints for accurate click detection
        self.cached_ratatui_constraints = ratatui_constraints.clone();
        
        // Create header cells with arrow indicator for focused column
        let header_cells: Vec<Cell> = header.cells.iter().enumerate().map(|(idx, c)| {
            let text = if self.detail_focused && idx == focused_col {
                // Prepend arrow to the existing header text
                format!("▼\n{}", c)
            } else {
                c.to_string()
            };
            
            // Keep yellow color for all headers, focused column only gets the arrow
            let style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
            Cell::from(Text::from(text)).style(style)
        }).collect();
        let header_row = Row::new(header_cells).height(3);
        let table = Table::new(visible_rows, ratatui_constraints).column_spacing(3).header(header_row);
        frame.render_widget(table, inner);

        if row_count > viewport_height {
            render_scrollbar(frame, area, row_count, self.section_scrolls[section_idx], viewport_height);
        }
    }

    fn get_column_widths(&mut self, section_idx: usize, constraints: &[crate::tree::ColumnConstraint]) -> Vec<u16> {
        // Ensure we have enough entries in column_widths
        while self.column_widths.len() <= section_idx {
            self.column_widths.push(Vec::new());
        }
        
        // If we don't have custom widths for this section, initialize from constraints
        if self.column_widths[section_idx].is_empty() {
            // First pass: convert to initial widths
            let mut widths: Vec<u16> = constraints.iter().map(|c| match c {
                crate::tree::ColumnConstraint::Fixed(w) => {
                    // Convert fixed width to a reasonable percentage (roughly 1.5% per char)
                    (*w as u16 * 3 / 2).max(3).min(15)
                },
                crate::tree::ColumnConstraint::Percentage(p) => *p,
            }).collect();
            
            // Normalize to ensure total is exactly 100%
            let total: u16 = widths.iter().sum();
            if total > 0 && total != 100 {
                // Scale all widths proportionally to sum to 100
                widths = widths.iter().map(|&w| {
                    ((w as f32 / total as f32) * 100.0).round() as u16
                }).collect();
                
                // Handle rounding errors: adjust the largest column
                let new_total: u16 = widths.iter().sum();
                if new_total != 100 && !widths.is_empty() {
                    let max_idx = widths.iter().enumerate()
                        .max_by_key(|(_, w)| *w)
                        .map(|(idx, _)| idx)
                        .unwrap_or(0);
                    widths[max_idx] = widths[max_idx].saturating_add(100 - new_total);
                }
            }
            
            self.column_widths[section_idx] = widths;
        }
        
        self.column_widths[section_idx].clone()
    }
}

fn render_scrollbar(
    frame: &mut Frame,
    area: Rect,
    total: usize,
    position: usize,
    viewport_height: usize,
) {
    if total <= viewport_height {
        return;
    }
    let mut state = ScrollbarState::new(total)
        .position(position)
        .viewport_content_length(viewport_height);
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight),
        area,
        &mut state,
    );
}
