use ratatui::{
    layout::{Constraint, Flex, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Table,
    },
    Frame,
};

use crate::tree::TreeNode;

use super::App;

// -----------------------------------------------------------------------
// Colour theme
// -----------------------------------------------------------------------

fn node_style(node: &TreeNode) -> Style {
    let t = &node.text;
    if t.starts_with("ECU:") {
        return style(Color::Cyan, true);
    }
    if t.contains("[base]") {
        return style(Color::Green, true);
    }
    if is_section_header(t) {
        return style(Color::Yellow, true);
    }
    if t.starts_with("Request") || t.starts_with("PosResponse") || t.starts_with("NegResponse") {
        return Style::default().fg(Color::Magenta);
    }
    if t.contains("[CodedConst]") || t.contains("[Value]") || t.contains("[NrcConst]") {
        return Style::default().fg(Color::White);
    }
    if t.starts_with("DOP:")
        || t.starts_with("CodedValue:")
        || t.starts_with("PhysDefault:")
        || t.starts_with("PhysConstValue:")
    {
        return Style::default().fg(Color::DarkGray);
    }
    Style::default().fg(Color::Gray)
}

fn is_section_header(t: &str) -> bool {
    t.starts_with("Services")
        || t.starts_with("Variants")
        || t.starts_with("Functional Groups")
        || t.starts_with("DTCs")
        || t.starts_with("Single ECU Jobs")
        || t.starts_with("State Charts")
        || t.starts_with("ComParam Refs")
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

// -----------------------------------------------------------------------
// Drawing
// -----------------------------------------------------------------------

impl App {
    pub(super) fn draw_tree(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border(!self.detail_focused))
            .title(" Tree ")
            .title_bottom(" /:search  Tab:focus  +/-:resize  e/c:expand/collapse ");

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let viewport_height = inner.height as usize;
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

        frame.render_widget(Paragraph::new(lines), inner);
        render_scrollbar(frame, area, self.visible.len(), self.scroll_offset, viewport_height);
    }

    pub(super) fn draw_detail(&mut self, frame: &mut Frame, area: Rect) {
        // Clone details to avoid borrow checker issues
        let details: Vec<String> = self
            .visible
            .get(self.cursor)
            .map(|&idx| self.all_nodes[idx].details.clone())
            .unwrap_or_default();

        if details.is_empty() {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(border(self.detail_focused))
                .title(" Details ");
            let inner = block.inner(area);
            frame.render_widget(block, area);
            frame.render_widget(
                Paragraph::new("Select a node to view details")
                    .style(Style::default().fg(Color::DarkGray)),
                inner,
            );
            return;
        }

        // Split details into sections based on "---" separators
        let sections = split_into_sections(&details);
        
        if sections.len() <= 1 {
            // No sections or just one section - render normally
            self.draw_detail_single_pane(frame, area, &details);
        } else {
            // Multiple sections - create vertical sub-panes
            self.draw_detail_multi_pane(frame, area, &sections);
        }
    }

    fn draw_detail_single_pane(&mut self, frame: &mut Frame, area: Rect, details: &[String]) {
        let help_text = if self.detail_focused {
            " Tab:focus  j/k:row  Home/End:jump "
        } else {
            " Tab:focus  j/k:scroll "
        };
        
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border(self.detail_focused))
            .title(" Details ")
            .title_bottom(help_text);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let detail_count = details.len();
        let viewport_height = inner.height as usize;
        self.clamp_detail_scroll(detail_count, viewport_height);

        // Parse lines into table rows
        let table_rows: Vec<TableRowData> = details
            .iter()
            .skip(self.detail_scroll)
            .take(viewport_height)
            .map(|l| parse_line_to_table_row(l))
            .collect();

        // Determine if we have multi-column tables
        let has_multi_column = table_rows.iter().any(|r| r.is_multi_column);
        let max_columns = table_rows.iter().map(|r| r.cells.len()).max().unwrap_or(2);

        // Create table rows with proper styling
        let mut rows: Vec<Row> = Vec::new();
        for row_data in table_rows.iter() {
            let indent_str = "  ".repeat(row_data.indent / 2);
            
            // Check if this is a parameter header
            let first_cell = row_data.cells.first().map(|s| s.as_str()).unwrap_or("");
            let is_param_header = first_cell.contains('[') && first_cell.contains('@');
            
            // Add spacing before parameter headers (except the first one)
            if is_param_header && !rows.is_empty() {
                let mut separator_cells = Vec::new();
                for _ in 0..max_columns {
                    separator_cells.push(Cell::from("─────────────").style(Style::default().fg(Color::DarkGray)));
                }
                rows.push(Row::new(separator_cells));
            }
            
            // Build row cells
            let mut cells = Vec::new();
            for (col_idx, cell_text) in row_data.cells.iter().enumerate() {
                let text = if col_idx == 0 {
                    format!("{}{}", indent_str, cell_text)
                } else {
                    cell_text.clone()
                };
                
                // Style based on column and content
                let cell_style = if row_data.is_multi_column && col_idx == 0 {
                    // First column in multi-column table
                    Style::default().fg(Color::Cyan)
                } else if is_param_header {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else if row_data.cells.len() == 1 {
                    // Single cell (header line)
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                } else if col_idx == 0 {
                    // Key in key-value pair
                    Style::default().fg(Color::Cyan)
                } else {
                    // Value or data cell
                    Style::default().fg(Color::Gray)
                };
                
                cells.push(Cell::from(text).style(cell_style));
            }
            
            // Pad row to match max columns if needed
            while cells.len() < max_columns {
                cells.push(Cell::from(""));
            }
            
            rows.push(Row::new(cells));
        }

        // Create constraints based on column count
        let constraints: Vec<Constraint> = if has_multi_column && max_columns > 2 {
            // Multi-column table: distribute evenly
            (0..max_columns)
                .map(|_| Constraint::Percentage(100 / max_columns as u16))
                .collect()
        } else {
            // Two-column key-value layout
            vec![Constraint::Percentage(40), Constraint::Percentage(60)]
        };

        let table = Table::new(rows, constraints)
            .column_spacing(2);

        frame.render_widget(table, inner);
        render_scrollbar(frame, area, detail_count, self.detail_scroll, viewport_height);
    }

    fn draw_detail_multi_pane(&mut self, frame: &mut Frame, area: Rect, sections: &[DetailSection]) {
        use ratatui::layout::{Constraint, Direction, Layout};

        // Clamp focused_section to valid range
        if self.focused_section >= sections.len() {
            self.focused_section = sections.len().saturating_sub(1);
        }

        // Ensure section_scrolls and section_cursors have enough entries
        while self.section_scrolls.len() < sections.len() {
            self.section_scrolls.push(0);
        }
        // todo duplicated?!
        while self.section_cursors.len() < sections.len() {
            self.section_cursors.push(0);
        }

        // Create equal-height constraints for each section
        let section_count = sections.len();
        let constraints: Vec<Constraint> = (0..section_count)
            .map(|_| Constraint::Ratio(1, section_count as u32))
            .collect();

        let section_areas = Layout::default()
            .direction(Direction::Vertical)
            .flex(Flex::Legacy)
            .constraints(constraints)
            .split(area);

        for (i, section) in sections.iter().enumerate() {
            let is_focused = self.detail_focused && i == self.focused_section;
            
            let help_text = if is_focused {
                " h/l:sections  j/k:row  Enter:DOP  Esc:close "
            } else {
                ""
            };
            
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(border(is_focused))
                .title(format!(" {} ", section.title))
                .title_bottom(help_text);

            let inner = block.inner(section_areas[i]);
            frame.render_widget(block, section_areas[i]);

            let viewport_height = inner.height as usize;
            
            // Parse all lines first to determine table structure
            let parsed_rows: Vec<TableRowData> = section.lines.iter()
                .map(|line| parse_line_to_table_row(line))
                .collect();
            
            let has_multi_column = parsed_rows.iter().any(|r| r.is_multi_column);
            let max_columns = parsed_rows.iter().map(|r| r.cells.len()).max().unwrap_or(2);
            
            // Build table rows with spacing
            let mut all_rows: Vec<Row> = Vec::new();
            
            for row_data in &parsed_rows {
                let indent_str = "  ".repeat(row_data.indent / 2);
                
                // Check if this is a parameter header
                let first_cell = row_data.cells.first().map(|s| s.as_str()).unwrap_or("");
                let is_param_header = first_cell.contains('[') && first_cell.contains('@');
                
                // Add spacing before parameter headers (except the first one)
                if is_param_header && !all_rows.is_empty() {
                    let mut separator_cells = Vec::new();
                    for _ in 0..max_columns {
                        separator_cells.push(Cell::from("─────────────").style(Style::default().fg(Color::DarkGray)));
                    }
                    all_rows.push(Row::new(separator_cells));
                }
                
                // Build row cells
                let mut cells = Vec::new();
                for (col_idx, cell_text) in row_data.cells.iter().enumerate() {
                    let text = if col_idx == 0 {
                        format!("{}{}", indent_str, cell_text)
                    } else {
                        cell_text.clone()
                    };
                    
                    // Style based on column and content
                    let cell_style = if row_data.is_multi_column && col_idx == 0 {
                        // First column in multi-column table
                        Style::default().fg(Color::Cyan)
                    } else if is_param_header {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else if row_data.cells.len() == 1 {
                        // Single cell (header line)
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                    } else if col_idx == 0 {
                        // Key in key-value pair
                        Style::default().fg(Color::Cyan)
                    } else {
                        // Value or data cell
                        Style::default().fg(Color::Gray)
                    };
                    
                    cells.push(Cell::from(text).style(cell_style));
                }
                
                // Pad row to match max columns if needed
                while cells.len() < max_columns {
                    cells.push(Cell::from(""));
                }
                
                all_rows.push(Row::new(cells));
            }

            // Clamp cursor for this section
            let row_count = all_rows.len();
            if self.section_cursors[i] >= row_count {
                self.section_cursors[i] = row_count.saturating_sub(1);
            }

            // Auto-scroll to keep cursor visible
            let cursor_pos = self.section_cursors[i];
            if cursor_pos < self.section_scrolls[i] {
                self.section_scrolls[i] = cursor_pos;
            } else if cursor_pos >= self.section_scrolls[i] + viewport_height {
                self.section_scrolls[i] = cursor_pos.saturating_sub(viewport_height).saturating_add(1);
            }

            // Clamp scroll for this section
            if self.section_scrolls[i] >= row_count.saturating_sub(viewport_height) && row_count > viewport_height {
                self.section_scrolls[i] = row_count.saturating_sub(viewport_height);
            }

            // Apply scrolling and highlight selected row
            let visible_rows: Vec<Row> = all_rows
                .into_iter()
                .enumerate()
                .skip(self.section_scrolls[i])
                .take(viewport_height)
                .map(|(idx, row)| {
                    if is_focused && idx == cursor_pos {
                        // Highlight selected row
                        row.style(Style::default().bg(Color::DarkGray).fg(Color::White))
                    } else {
                        row
                    }
                })
                .collect();

            // Create constraints based on column count
            let constraints: Vec<Constraint> = if has_multi_column && max_columns > 2 {
                // Multi-column table: distribute evenly
                (0..max_columns)
                    .map(|_| Constraint::Percentage(100 / max_columns as u16))
                    .collect()
            } else {
                // Two-column key-value layout
                vec![Constraint::Percentage(40), Constraint::Percentage(60)]
            };

            let table = Table::new(visible_rows, constraints)
                .column_spacing(2);

            frame.render_widget(table, inner);
            
            // Render scrollbar if needed
            if row_count > viewport_height {
                render_scrollbar(frame, section_areas[i], row_count, self.section_scrolls[i], viewport_height);
            }
        }
    }

    pub(super) fn draw_status(&self, frame: &mut Frame, area: Rect) {
        let (text, st) = if self.searching {
            (
                format!(" /{}█  (Enter to confirm, Esc to cancel)", self.search),
                Style::default().fg(Color::Yellow).bg(Color::DarkGray),
            )
        } else if !self.status.is_empty() {
            (format!(" {}", self.status), Style::default().fg(Color::Gray))
        } else {
            let focus = if self.detail_focused { "detail" } else { "tree" };
            (
                format!(
                    " {}/{} nodes | cursor: {} | focus: {focus}",
                    self.visible.len(),
                    self.all_nodes.len(),
                    self.cursor + 1,
                ),
                Style::default().fg(Color::Gray),
            )
        };
        frame.render_widget(Paragraph::new(text).style(st), area);
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
}

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

struct DetailSection {
    title: String,
    lines: Vec<String>,
}

#[derive(Debug)]
struct TableRowData {
    cells: Vec<String>,  // Multiple columns for pipe-separated data
    indent: usize,
    is_multi_column: bool,  // True if this is a pipe-separated row
}

fn parse_line_to_table_row(line: &str) -> TableRowData {
    // Count leading spaces for indent
    let indent = line.chars().take_while(|c| *c == ' ').count();
    let trimmed = line.trim();
    
    // Check if this is a pipe-separated multi-column row
    if trimmed.contains(" | ") {
        let cells: Vec<String> = trimmed
            .split(" | ")
            .map(|s| s.trim().to_string())
            .collect();
        TableRowData {
            cells,
            indent,
            is_multi_column: true,
        }
    } else if let Some(pos) = trimmed.find(':') {
        // Split on first ':' to get key-value pairs
        let key = trimmed[..pos].trim().to_string();
        let value = trimmed[pos + 1..].trim().to_string();
        TableRowData {
            cells: vec![key, value],
            indent,
            is_multi_column: false,
        }
    } else {
        // No colon or pipe, treat entire line as single cell
        TableRowData {
            cells: vec![trimmed.to_string()],
            indent,
            is_multi_column: false,
        }
    }
}

fn split_into_sections(details: &[String]) -> Vec<DetailSection> {
    let mut sections = Vec::new();
    let mut current_title = String::from("Details");
    let mut current_lines = Vec::new();

    for line in details {
        if line.starts_with("---") && line.ends_with("---") {
            // Save the current section if it has content
            if !current_lines.is_empty() || !sections.is_empty() {
                sections.push(DetailSection {
                    title: current_title.clone(),
                    lines: current_lines.clone(),
                });
                current_lines.clear();
            }
            // Extract title from "--- Title ---"
            current_title = line.trim_start_matches("---").trim_end_matches("---").trim().to_string();
        } else {
            current_lines.push(line.clone());
        }
    }

    // Add the last section
    if !current_lines.is_empty() || !sections.is_empty() {
        sections.push(DetailSection {
            title: current_title,
            lines: current_lines,
        });
    }

    // If we ended up with no sections, create one default section
    if sections.is_empty() {
        sections.push(DetailSection {
            title: "Details".to_string(),
            lines: details.to_vec(),
        });
    }

    sections
}

fn row_style(node: &TreeNode, is_cursor: bool) -> Style {
    if is_cursor {
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
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