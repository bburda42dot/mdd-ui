mod input;
mod render;

use std::io;

use crossterm::event::{self, Event, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal,
    layout::{Constraint, Direction, Layout},
    Frame,
};

use crate::tree::TreeNode;

use input::Action;

// -----------------------------------------------------------------------
// Application state
// -----------------------------------------------------------------------

pub struct App {
    all_nodes: Vec<TreeNode>,
    visible: Vec<usize>,
    cursor: usize,
    scroll_offset: usize,
    detail_scroll: usize,
    pub(crate) search: String,
    pub(crate) searching: bool,
    pub(crate) search_matches: Vec<usize>,
    search_match_cursor: usize,
    pub(crate) status: String,
    pub(crate) detail_focused: bool,
    pub(crate) focused_section: usize, // Which detail pane section is focused (0 = first)
    pub(crate) section_scrolls: Vec<usize>, // Scroll position for each section
    pub(crate) section_cursors: Vec<usize>, // Selected row in each section
    pub(crate) dop_popup: Option<DopPopupData>, // DOP popup state
    pub(crate) tree_width_percentage: u16, // Tree pane width (0-100)
}

#[derive(Clone)]
pub struct DopPopupData {
    pub dop_name: String,
    pub dop_details: Vec<String>,
}

impl App {
    pub fn new(nodes: Vec<TreeNode>) -> Self {
        let mut app = Self {
            all_nodes: nodes,
            visible: Vec::new(),
            cursor: 0,
            scroll_offset: 0,
            detail_scroll: 0,
            search: String::new(),
            searching: false,
            search_matches: Vec::new(),
            search_match_cursor: 0,
            status: String::new(),
            detail_focused: false,
            focused_section: 0,
            section_scrolls: Vec::new(),
            section_cursors: Vec::new(),
            dop_popup: None,
            tree_width_percentage: 40,
        };
        app.rebuild_visible();
        app
    }

    // -------------------------------------------------------------------
    // Event loop
    // -------------------------------------------------------------------

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }

            let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
            let action = if self.searching {
                self.handle_search_key(key.code)
            } else {
                self.handle_normal_key(key.code, ctrl)
            };

            if matches!(action, Action::Quit) {
                return Ok(());
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let [main, status_bar] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(1)])
            .areas(frame.area());

        let [tree_area, detail_area] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(self.tree_width_percentage),
                Constraint::Percentage(100 - self.tree_width_percentage),
            ])
            .areas(main);

        self.draw_tree(frame, tree_area);
        self.draw_detail(frame, detail_area);
        self.draw_status(frame, status_bar);
        
        // Draw popup if open
        if self.dop_popup.is_some() {
            self.draw_dop_popup(frame);
        }
    }

    // -------------------------------------------------------------------
    // Visibility
    // -------------------------------------------------------------------

    fn rebuild_visible(&mut self) {
        self.visible.clear();
        let mut collapsed_below: Option<usize> = None;

        for (i, node) in self.all_nodes.iter().enumerate() {
            if let Some(cd) = collapsed_below {
                if node.depth > cd {
                    continue;
                }
                collapsed_below = None;
            }
            self.visible.push(i);
            if node.has_children && !node.expanded {
                collapsed_below = Some(node.depth);
            }
        }
    }

    // -------------------------------------------------------------------
    // Tree navigation
    // -------------------------------------------------------------------

    pub(crate) fn toggle_expand(&mut self) {
        let Some(&idx) = self.visible.get(self.cursor) else { return };
        if !self.all_nodes[idx].has_children {
            return;
        }
        self.all_nodes[idx].expanded = !self.all_nodes[idx].expanded;
        let old = self.cursor;
        self.rebuild_visible();
        self.cursor = old.min(self.visible.len().saturating_sub(1));
    }

    pub(crate) fn try_expand(&mut self) {
        if self.detail_focused {
            return;
        }
        let Some(&idx) = self.visible.get(self.cursor) else { return };
        if self.all_nodes[idx].has_children && !self.all_nodes[idx].expanded {
            self.toggle_expand();
        }
    }

    pub(crate) fn try_collapse_or_parent(&mut self) {
        if self.detail_focused {
            return;
        }
        let Some(&idx) = self.visible.get(self.cursor) else { return };
        let node = &self.all_nodes[idx];

        if node.has_children && node.expanded {
            self.toggle_expand();
            return;
        }
        // Navigate to parent
        let my_depth = node.depth;
        if my_depth == 0 {
            return;
        }
        for i in (0..self.cursor).rev() {
            if self.all_nodes[self.visible[i]].depth < my_depth {
                self.cursor = i;
                self.detail_scroll = 0;
                break;
            }
        }
    }

    pub(crate) fn expand_all(&mut self) {
        for n in &mut self.all_nodes {
            if n.has_children {
                n.expanded = true;
            }
        }
        self.rebuild_visible();
    }

    pub(crate) fn collapse_all(&mut self) {
        for (i, n) in self.all_nodes.iter_mut().enumerate() {
            if n.has_children {
                n.expanded = i == 0;
            }
        }
        self.rebuild_visible();
        self.cursor = 0;
        self.scroll_offset = 0;
    }

    // -------------------------------------------------------------------
    // Cursor movement
    // -------------------------------------------------------------------

    pub(crate) fn move_up(&mut self) {
        if self.detail_focused {
            self.detail_scroll = self.detail_scroll.saturating_sub(1);
        } else {
            self.cursor = self.cursor.saturating_sub(1);
            self.detail_scroll = 0;
        }
    }

    pub(crate) fn move_down(&mut self) {
        if self.detail_focused {
            self.detail_scroll += 1;
        } else if self.cursor + 1 < self.visible.len() {
            self.cursor += 1;
            self.detail_scroll = 0;
        }
    }

    pub(crate) fn page_up(&mut self, n: usize) {
        if self.detail_focused {
            self.detail_scroll = self.detail_scroll.saturating_sub(n);
        } else {
            self.cursor = self.cursor.saturating_sub(n);
            self.detail_scroll = 0;
        }
    }

    pub(crate) fn page_down(&mut self, n: usize) {
        if self.detail_focused {
            self.detail_scroll += n;
        } else {
            self.cursor = (self.cursor + n).min(self.visible.len().saturating_sub(1));
            self.detail_scroll = 0;
        }
    }

    pub(crate) fn home(&mut self) {
        if self.detail_focused {
            self.detail_scroll = 0;
        } else {
            self.cursor = 0;
            self.detail_scroll = 0;
        }
    }

    pub(crate) fn end(&mut self) {
        if self.detail_focused {
            self.detail_scroll = usize::MAX; // clamped during render
        } else {
            self.cursor = self.visible.len().saturating_sub(1);
            self.detail_scroll = 0;
        }
    }

    // -------------------------------------------------------------------
    // Scroll helpers
    // -------------------------------------------------------------------

    pub(crate) fn ensure_cursor_visible(&mut self, viewport_height: usize) {
        if viewport_height == 0 {
            return;
        }
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        } else if self.cursor >= self.scroll_offset + viewport_height {
            self.scroll_offset = self.cursor - viewport_height + 1;
        }
    }

    pub(crate) fn clamp_detail_scroll(&mut self, content_len: usize, viewport_height: usize) {
        if content_len > viewport_height {
            self.detail_scroll = self
                .detail_scroll
                .min(content_len.saturating_sub(viewport_height));
        } else {
            self.detail_scroll = 0;
        }
    }

    // -------------------------------------------------------------------
    // Search
    // -------------------------------------------------------------------

    pub(crate) fn update_search(&mut self) {
        self.search_matches.clear();
        if self.search.is_empty() {
            self.status.clear();
            return;
        }
        let query = self.search.to_lowercase();
        for (vi, &idx) in self.visible.iter().enumerate() {
            if self.all_nodes[idx].text.to_lowercase().contains(&query) {
                self.search_matches.push(vi);
            }
        }
        self.search_match_cursor = 0;
        if let Some(&first) = self.search_matches.first() {
            self.cursor = first;
            self.detail_scroll = 0;
        }
        let n = self.search_matches.len();
        self.status = format!("{n} match{}", if n == 1 { "" } else { "es" });
    }

    pub(crate) fn next_search_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        self.search_match_cursor = (self.search_match_cursor + 1) % self.search_matches.len();
        self.cursor = self.search_matches[self.search_match_cursor];
        self.detail_scroll = 0;
        self.status = format!(
            "Match {}/{}",
            self.search_match_cursor + 1,
            self.search_matches.len()
        );
    }

    pub(crate) fn prev_search_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        self.search_match_cursor = if self.search_match_cursor == 0 {
            self.search_matches.len() - 1
        } else {
            self.search_match_cursor - 1
        };
        self.cursor = self.search_matches[self.search_match_cursor];
        self.detail_scroll = 0;
        self.status = format!(
            "Match {}/{}",
            self.search_match_cursor + 1,
            self.search_matches.len()
        );
    }

    pub(crate) fn try_show_dop_popup(&mut self) {
        // Get current selected node details
        if self.cursor >= self.visible.len() {
            self.status = "Cursor out of bounds".to_string();
            return;
        }
        let node_idx = self.visible[self.cursor];
        let node = &self.all_nodes[node_idx];
        
        // Check if we're in a detail pane with sections
        if node.details.is_empty() {
            self.status = "No details available".to_string();
            return;
        }
        
        // Get the currently selected row in the focused section
        if self.focused_section >= self.section_cursors.len() {
            self.status = "Section cursor not initialized".to_string();
            return;
        }
        let row_cursor = self.section_cursors[self.focused_section];
        
        // Parse details into sections
        let sections = self.parse_detail_sections(&node.details);
        if self.focused_section >= sections.len() {
            self.status = format!("Section {} out of {} sections", self.focused_section, sections.len());
            return;
        }
        
        let section = &sections[self.focused_section];
        if row_cursor >= section.lines.len() {
            self.status = format!("Row {} out of {} lines", row_cursor, section.lines.len());
            return;
        }
        
        // Get the selected line and check if it has a DOP value
        let selected_line = &section.lines[row_cursor];
        
        // Parse the line to check if it's a pipe-separated row with a DOP column
        if selected_line.contains(" | ") {
            let cells: Vec<&str> = selected_line.split(" | ").map(|s| s.trim()).collect();
            
            self.status = format!("Selected line: {} cells: {:?}", cells.len(), cells);
            
            // Check if this is a parameter row (8 columns) and the DOP column (index 6) is not empty
            if cells.len() == 8 && !cells[6].is_empty() {
                let dop_name = cells[6].to_string();
                // Build DOP details (for now, show basic info - can be expanded)
                let dop_details = vec![
                    format!("DOP Name: {}", dop_name),
                    format!("Type: Data Object Property"),
                    format!("Used in parameter: {}", cells[0]),
                    format!("Semantic: {}", cells[7]),
                    String::new(),
                    "(Full DOP details would be loaded from database)".to_string(),
                ];
                
                self.status = format!("Opening DOP popup for: {}", dop_name);
                
                self.dop_popup = Some(DopPopupData {
                    dop_name,
                    dop_details,
                });
            } else if cells.len() == 8 {
                self.status = "Parameter row but no DOP value".to_string();
            }
        } else {
            self.status = format!("Not a pipe-separated row: {}", selected_line);
        }
    }
    
    fn parse_detail_sections(&self, details: &[String]) -> Vec<DetailSectionData> {
        let mut sections = Vec::new();
        let mut current_title = String::from("Details");
        let mut current_lines = Vec::new();

        for line in details {
            if line.starts_with("---") && line.ends_with("---") {
                // Save the current section if it has content
                if !current_lines.is_empty() || !sections.is_empty() {
                    sections.push(DetailSectionData {
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

        // Save the last section
        if !current_lines.is_empty() || !sections.is_empty() {
            sections.push(DetailSectionData {
                title: current_title,
                lines: current_lines,
            });
        } else if sections.is_empty() {
            // If no sections, create a default one
            sections.push(DetailSectionData {
                title: "Details".to_string(),
                lines: details.to_vec(),
            });
        }

        sections
    }
}

#[derive(Clone)]
struct DetailSectionData {
    #[allow(dead_code)]
    title: String,
    lines: Vec<String>,
}