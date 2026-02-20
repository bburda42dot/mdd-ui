mod input;
mod render;

use std::io;

use crossterm::event::{self, Event, KeyEventKind, KeyModifiers, MouseEventKind, MouseButton, DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use ratatui::{
    DefaultTerminal,
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};
use std::time::{Instant, Duration};

use crate::tree::{TreeNode, DetailRow, CellType, NodeType};

use input::Action;

// -----------------------------------------------------------------------
// Application state
// -----------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SearchScope {
    All,              // Search everywhere
    Variants,         // Search only in variant names
    Services,         // Search only in service names
    DiagComms,        // Search only in Diag-Comms sections
}

pub struct App {
    all_nodes: Vec<TreeNode>,
    visible: Vec<usize>,
    cursor: usize,
    scroll_offset: usize,
    detail_scroll: usize,
    pub(crate) search: String,
    pub(crate) searching: bool,
    pub(crate) search_stack: Vec<(String, SearchScope)>,  // Stack of (search_term, scope) pairs
    pub(crate) search_scope: SearchScope,
    pub(crate) search_matches: Vec<usize>,
    search_match_cursor: usize,
    pub(crate) status: String,
    pub(crate) detail_focused: bool,
    pub(crate) selected_tab: usize, // Currently selected tab in detail pane
    pub(crate) focused_section: usize, // Which detail pane section is focused (0 = first)
    pub(crate) section_scrolls: Vec<usize>, // Scroll position for each section
    pub(crate) section_cursors: Vec<usize>, // Selected row in each section
    pub(crate) column_widths: Vec<Vec<u16>>, // Column widths for each section (percentages)
    pub(crate) focused_column: usize, // Currently focused column for resizing
    pub(crate) dop_popup: Option<DopPopupData>, // DOP popup state
    pub(crate) help_popup_visible: bool, // Help popup visibility
    pub(crate) tree_width_percentage: u16, // Tree pane width (0-100)
    pub(crate) diagcomm_sort_by_id: bool, // true = sort by ID (default), false = sort by name
    tree_area: Rect, // Cached tree area for mouse handling
    detail_area: Rect, // Cached detail area for mouse handling
    pub(crate) tab_area: Option<Rect>, // Cached tab area for mouse handling
    pub(crate) tab_titles: Vec<String>, // Cached tab titles for click detection
    pub(crate) table_content_area: Option<Rect>, // Cached table content area for row/cell clicking
    pub(crate) cached_ratatui_constraints: Vec<ratatui::layout::Constraint>, // Exact constraints used in Table
    last_click_time: Option<Instant>, // Time of last click for double-click detection
    last_click_pos: (u16, u16), // Position of last click (column, row)
    pub(crate) mouse_enabled: bool, // Whether mouse input is enabled
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
            search_stack: Vec::new(),
            search_scope: SearchScope::All,
            search_matches: Vec::new(),
            search_match_cursor: 0,
            status: String::new(),
            detail_focused: false,
            selected_tab: 0,
            focused_section: 0,
            section_scrolls: Vec::new(),
            section_cursors: Vec::new(),
            column_widths: Vec::new(),
            focused_column: 0,
            dop_popup: None,
            help_popup_visible: false,
            tree_width_percentage: 40,
            diagcomm_sort_by_id: true, // Default: sort by ID
            tree_area: Rect::default(),
            detail_area: Rect::default(),
            tab_area: None,
            tab_titles: Vec::new(),
            table_content_area: None,
            cached_ratatui_constraints: Vec::new(),
            last_click_time: None,
            last_click_pos: (0, 0),
            mouse_enabled: true, // Mouse enabled by default
        };
        // Apply initial sort order (default is by ID)
        app.sort_diagcomm_nodes_in_place();
        app.rebuild_visible();
        app
    }
    
    /// Get the actual section index accounting for semantic header offset
    fn get_section_index(&self) -> usize {
        // Check if current node has a semantic section (state chart)
        if let Some(&idx) = self.visible.get(self.cursor) {
            let sections = &self.all_nodes[idx].detail_sections;
            if sections.len() > 1 
                && sections[0].title == "Semantic" 
                && matches!(&sections[0].content, crate::tree::DetailContent::PlainText(_)) {
                // Has semantic section, so selected_tab needs offset of 1
                return self.selected_tab + 1;
            }
        }
        self.selected_tab
    }

    // -------------------------------------------------------------------
    // Event loop
    // -------------------------------------------------------------------

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            let event = event::read()?;
            
            let action = match event {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                    if self.searching {
                        self.handle_search_key(key.code)
                    } else {
                        self.handle_normal_key(key.code, ctrl)
                    }
                }
                Event::Mouse(mouse) => {
                    if self.mouse_enabled {
                        self.handle_mouse_event(mouse.kind, mouse.column, mouse.row)
                    } else {
                        Action::Continue
                    }
                }
                _ => Action::Continue,
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

        // Cache areas for mouse handling
        self.tree_area = tree_area;
        self.detail_area = detail_area;

        self.draw_tree(frame, tree_area);
        self.draw_detail(frame, detail_area);
        self.draw_status(frame, status_bar);
        
        // Draw popups if open (order matters - last drawn is on top)
        if self.dop_popup.is_some() {
            self.draw_dop_popup(frame);
        }
        if self.help_popup_visible {
            self.draw_help_popup(frame);
        }
    }

    // -------------------------------------------------------------------
    // Visibility
    // -------------------------------------------------------------------

    fn rebuild_visible(&mut self) {
        self.visible.clear();
        
        // Apply search stack (cumulative searches)
        if self.search_stack.is_empty() {
            // No search - show all nodes respecting collapse state
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
        } else {
            // Search mode: apply each search in the stack cumulatively
            let mut include = vec![true; self.all_nodes.len()];
            
            // Apply each search filter in sequence with its own scope
            for (query, scope) in &self.search_stack {
                let q = query.to_lowercase();
                let mut new_include = vec![false; self.all_nodes.len()];
                
                for i in 0..self.all_nodes.len() {
                    // Only consider nodes that passed previous filters
                    if !include[i] {
                        continue;
                    }
                    
                    // Apply this search's scope filter
                    let node_matches_scope = match scope {
                        SearchScope::All => true,
                        SearchScope::Variants => {
                            matches!(self.all_nodes[i].node_type, NodeType::Container)
                        },
                        SearchScope::Services => {
                            matches!(self.all_nodes[i].node_type, 
                                NodeType::Service | NodeType::ParentRefService |
                                NodeType::Request | NodeType::PosResponse | NodeType::NegResponse)
                        },
                        SearchScope::DiagComms => {
                            self.all_nodes[i].text.starts_with("Diag-Comms (") ||
                            matches!(self.all_nodes[i].node_type, 
                                NodeType::Service | NodeType::ParentRefService)
                        },
                    };
                    
                    // Check if this node matches both scope and search text
                    if node_matches_scope && self.all_nodes[i].text.to_lowercase().contains(&q) {
                        new_include[i] = true;
                        
                        // Include all children of matched nodes
                        let match_depth = self.all_nodes[i].depth;
                        for (offset, node) in self.all_nodes[(i + 1)..].iter().enumerate() {
                            if node.depth > match_depth {
                                new_include[i + 1 + offset] = true;
                            } else {
                                break;
                            }
                        }
                        
                        // Include all parents of matched nodes to maintain tree structure
                        let target_depth = self.all_nodes[i].depth;
                        if target_depth > 0 {
                            let mut parent_depth = target_depth - 1;
                            for j in (0..i).rev() {
                                if self.all_nodes[j].depth == parent_depth {
                                    new_include[j] = true;
                                    if parent_depth == 0 {
                                        break;
                                    }
                                    parent_depth -= 1;
                                }
                            }
                        }
                    }
                }
                
                include = new_include;
            }

            
            // Build visible list from included nodes, respecting collapse state
            let mut collapsed_below: Option<usize> = None;
            
            for (i, &should_include) in include.iter().enumerate() {
                if !should_include {
                    continue;
                }
                
                let node = &self.all_nodes[i];
                
                // Check if we're inside a collapsed section
                if let Some(cd) = collapsed_below {
                    if node.depth > cd {
                        continue; // Skip nodes under collapsed parent
                    }
                    collapsed_below = None;
                }
                
                self.visible.push(i);
                
                // If this node is collapsed, hide its children
                if node.has_children && !node.expanded {
                    collapsed_below = Some(node.depth);
                }
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
    
    pub(crate) fn toggle_diagcomm_sort(&mut self) {
        // Toggle between sorting by ID and by name
        self.diagcomm_sort_by_id = !self.diagcomm_sort_by_id;
        
        // Rebuild tree nodes with new sort order
        // We need to pass the database to rebuild, but we don't have access here
        // Instead, we'll need to sort the existing nodes in place
        self.sort_diagcomm_nodes_in_place();
        self.rebuild_visible();
        
        self.status = if self.diagcomm_sort_by_id {
            "DiagComm sort: by ID".to_owned()
        } else {
            "DiagComm sort: by Name".to_owned()
        };
    }
    
    fn sort_diagcomm_nodes_in_place(&mut self) {
        // Find all "Diag-Comms" section headers and sort their children
        let mut i = 0;
        while i < self.all_nodes.len() {
            let node = &self.all_nodes[i];
            
            // Skip non-Diag-Comms nodes early
            if !node.text.starts_with("Diag-Comms (") {
                i += 1;
                continue;
            }
            
            let section_depth = node.depth;
            let section_start = i + 1;
            
            // Find all children (services) of this section
            let mut section_end = section_start;
            while section_end < self.all_nodes.len() && self.all_nodes[section_end].depth > section_depth {
                section_end += 1;
            }
            
            // Skip if no children to sort
            if section_end <= section_start {
                i += 1;
                continue;
            }
            
            // Extract and sort the service nodes
            let mut services: Vec<TreeNode> = self.all_nodes.drain(section_start..section_end).collect();
            
            // Sort services based on current sort order
            match self.diagcomm_sort_by_id {
                true => services.sort_by(|a, b| {
                    let a_id = extract_service_id(&a.text);
                    let b_id = extract_service_id(&b.text);
                    a_id.cmp(&b_id)
                }),
                false => services.sort_by(|a, b| {
                    let a_name = extract_service_name(&a.text);
                    let b_name = extract_service_name(&b.text);
                    a_name.cmp(b_name)
                }),
            }
            
            // Deduplicate by name - keep only first occurrence of each service name
            let mut seen_names = std::collections::HashSet::new();
            services.retain(|service| {
                let name = extract_service_name(&service.text);
                seen_names.insert(name.to_owned())
            });
            
            // Update the count in the section header
            let new_count = services.len();
            if let Some(header_node) = self.all_nodes.get_mut(i) {
                // Update "Diag-Comms (X)" to reflect filtered count
                if header_node.text.find('(').is_some() {
                    header_node.text = format!("Diag-Comms ({})", new_count);
                }
            }
            
            // Re-insert sorted and deduplicated services
            self.all_nodes.splice(section_start..section_start, services);
            
            // Skip past the sorted section
            i = section_start + (section_end - section_start);
        }
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

    // -------------------------------------------------------------------
    // Search
    // -------------------------------------------------------------------

    pub(crate) fn update_search(&mut self) {
        if self.search.is_empty() {
            // If search is empty, don't add to stack
            self.status.clear();
        } else {
            // Add current search with its scope to stack
            self.search_stack.push((self.search.clone(), self.search_scope.clone()));
            self.search.clear();  // Clear for next search
            
            let depth = self.search_stack.len();
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
            self.status = format!("Search depth: {} [{}]", depth, stack_display.join(" → "));
        }
        
        // Rebuild visible list with the search stack
        self.rebuild_visible();
        self.cursor = 0;
        self.detail_scroll = 0;
        self.search_matches.clear();
        self.search_match_cursor = 0;
    }
    
    pub(crate) fn clear_search_stack(&mut self) {
        self.search_stack.clear();
        self.search.clear();
        self.status = "Search cleared".to_owned();
        self.rebuild_visible();
        self.cursor = 0;
    }
    
    pub(crate) fn cycle_search_scope(&mut self) {
        self.search_scope = match self.search_scope {
            SearchScope::All => SearchScope::Variants,
            SearchScope::Variants => SearchScope::Services,
            SearchScope::Services => SearchScope::DiagComms,
            SearchScope::DiagComms => SearchScope::All,
        };
        
        let scope_name = match self.search_scope {
            SearchScope::All => "All",
            SearchScope::Variants => "Variants",
            SearchScope::Services => "Services",
            SearchScope::DiagComms => "Diag-Comms",
        };
        self.status = format!("Search scope: {}", scope_name);
    }
    
    pub(crate) fn toggle_mouse_mode(&mut self) {
        self.mouse_enabled = !self.mouse_enabled;
        
        // Actually enable/disable mouse capture in the terminal
        let result = if self.mouse_enabled {
            execute!(std::io::stdout(), EnableMouseCapture)
        } else {
            execute!(std::io::stdout(), DisableMouseCapture)
        };
        
        if result.is_ok() {
            self.status = format!("Mouse: {}", if self.mouse_enabled { "enabled" } else { "disabled" });
        } else {
            self.status = "Failed to toggle mouse mode".to_owned();
        }
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
        // Validate cursor position
        if self.cursor >= self.visible.len() {
            self.status = "Cursor out of bounds".to_string();
            return;
        }
        
        let node_idx = self.visible[self.cursor];
        let node = &self.all_nodes[node_idx];
        
        // Validate node has detail sections
        if node.detail_sections.is_empty() {
            self.status = "No details available".to_string();
            return;
        }

        // Validate section cursor is initialized
        if self.selected_tab >= self.section_cursors.len() {
            self.status = "Section cursor not initialized".to_string();
            return;
        }
        let row_cursor = self.section_cursors[self.selected_tab];

        // Validate tab exists
        let sections = &node.detail_sections;
        if self.selected_tab >= sections.len() {
            self.status = format!("Tab {} out of {} tabs", self.selected_tab, sections.len());
            return;
        }

        let section = &sections[self.selected_tab];
        
        // Extract rows from table content
        use crate::tree::DetailContent;
        let rows = match &section.content {
            DetailContent::Table { rows, .. } => rows,
            _ => {
                self.status = "DOP selection only available in tables".to_owned();
                return;
            }
        };
        
        // Validate row exists
        if row_cursor >= rows.len() {
            self.status = format!("Row {} out of {} lines", row_cursor, rows.len());
            return;
        }

        // Get the selected row and validate DOP cell
        let selected_row: &DetailRow = &rows[row_cursor];
        let cells = &selected_row.cells;
        let cell_types = &selected_row.cell_types;
        
        // Find and validate DOP cell
        let dop_cell_index = match cell_types.iter().position(|ct| matches!(ct, CellType::DopReference)) {
            Some(idx) => idx,
            None => {
                self.status = "No DOP reference in this row".to_owned();
                return;
            }
        };
        
        // Validate DOP cell has content
        if dop_cell_index >= cells.len() || cells[dop_cell_index].is_empty() {
            self.status = "DOP cell is empty".to_owned();
            return;
        }

        // Extract DOP details
        let dop_name = cells[dop_cell_index].to_owned();
        
        let param_name = cell_types.iter()
            .position(|ct| matches!(ct, CellType::ParameterName))
            .and_then(|idx| cells.get(idx))
            .map(|s| s.as_str())
            .unwrap_or("unknown");
        
        let semantic = cells.last().map(|s| s.as_str()).unwrap_or("");
        
        let dop_details = vec![
            format!("DOP Name: {}", dop_name),
            "Type: Data Object Property".to_owned(),
            format!("Used in parameter: {}", param_name),
            format!("Semantic: {}", semantic),
            String::new(),
            "(Full DOP details would be loaded from database)".to_owned(),
        ];

        self.status = format!("Opening DOP popup for: {}", dop_name);
        self.dop_popup = Some(DopPopupData { dop_name, dop_details });
    }

    pub(crate) fn resize_column(&mut self, delta: i16) {
        // Ensure we have column_widths entries for all sections
        while self.column_widths.len() <= self.selected_tab {
            self.column_widths.push(Vec::new());
        }
        
        if self.cursor >= self.visible.len() {
            return;
        }
        let node_idx = self.visible[self.cursor];
        let node = &self.all_nodes[node_idx];
        if self.selected_tab >= node.detail_sections.len() {
            return;
        }
        let section = &node.detail_sections[self.selected_tab];
        use crate::tree::DetailContent;
        let constraints = match &section.content {
            DetailContent::Table { constraints, .. } => constraints,
            _ => {
                self.status = "Column resizing only available in tables".to_owned();
                return;
            }
        };
        
        // Initialize column widths from constraints if not already done
        if self.column_widths[self.selected_tab].is_empty() {
            // First pass: convert to initial widths
            let mut widths: Vec<u16> = constraints.iter().map(|c| match c {
                crate::tree::ColumnConstraint::Fixed(w) => {
                    // Convert fixed width to a reasonable percentage (roughly 1.5% per char)
                    (*w * 3 / 2).clamp(3, 15)
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
            
            self.column_widths[self.selected_tab] = widths;
        }
        
        let num_cols = self.column_widths[self.selected_tab].len();
        if num_cols == 0 || self.focused_column >= num_cols {
            return;
        }
        
        // Calculate new width for focused column
        let current_width = self.column_widths[self.selected_tab][self.focused_column] as i16;
        let new_current = (current_width + delta).clamp(3, 95) as u16; // Min 3%, Max 95%
        let actual_delta = new_current as i16 - current_width;
        
        if actual_delta == 0 {
            self.status = "Cannot resize: at min/max width".to_owned();
            return;
        }
        
        // Apply the change to the focused column
        self.column_widths[self.selected_tab][self.focused_column] = new_current;
        
        // Distribute the delta across all other columns proportionally
        let num_other_cols = num_cols - 1;
        if num_other_cols > 0 {
            let total_other: u16 = self.column_widths[self.selected_tab].iter().enumerate()
                .filter(|(i, _)| *i != self.focused_column)
                .map(|(_, w)| *w)
                .sum();
            
            if total_other > 0 {
                // Distribute the negative delta proportionally across other columns
                for i in 0..num_cols {
                    if i != self.focused_column {
                        let old_width = self.column_widths[self.selected_tab][i] as i16;
                        let proportion = old_width as f32 / total_other as f32;
                        let adjustment = (-actual_delta as f32 * proportion).round() as i16;
                        let new_width = (old_width + adjustment).max(3) as u16;
                        self.column_widths[self.selected_tab][i] = new_width;
                    }
                }
            }
        }
        
        // Normalize to ensure total is exactly 100%
        let total: u16 = self.column_widths[self.selected_tab].iter().sum();
        if total > 0 && total != 100 {
            // Scale all widths proportionally to sum to 100
            let normalized: Vec<u16> = self.column_widths[self.selected_tab].iter().map(|&w| {
                ((w as f32 / total as f32) * 100.0).round() as u16
            }).collect();
            
            self.column_widths[self.selected_tab] = normalized;
            
            // Handle rounding errors: adjust the focused column to make total exactly 100
            let new_total: u16 = self.column_widths[self.selected_tab].iter().sum();
            if new_total != 100 {
                let diff = 100i16 - new_total as i16;
                let focused_width = self.column_widths[self.selected_tab][self.focused_column] as i16;
                self.column_widths[self.selected_tab][self.focused_column] = (focused_width + diff).max(1) as u16;
            }
        }
        
        self.status = format!("Column {} width: {}% (total: {}%)", 
            self.focused_column, 
            self.column_widths[self.selected_tab][self.focused_column],
            self.column_widths[self.selected_tab].iter().sum::<u16>());
    }

    // -------------------------------------------------------------------
    // Mouse handling
    // -------------------------------------------------------------------

    pub(super) fn handle_mouse_event(&mut self, kind: MouseEventKind, column: u16, row: u16) -> Action {
        // If popup is open, only close on click
        if self.dop_popup.is_some() {
            if matches!(kind, MouseEventKind::Down(_)) {
                self.dop_popup = None;
            }
            return Action::Continue;
        }

        match kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Check for double-click (within 500ms and same position)
                let is_double_click = if let Some(last_time) = self.last_click_time {
                    let elapsed = last_time.elapsed();
                    elapsed < Duration::from_millis(500) && self.last_click_pos == (column, row)
                } else {
                    false
                };

                if is_double_click {
                    self.handle_double_click(column, row);
                    // Reset click tracking to avoid triple-click being detected as another double-click
                    self.last_click_time = None;
                } else {
                    self.handle_click(column, row);
                    // Track this click for double-click detection
                    self.last_click_time = Some(Instant::now());
                    self.last_click_pos = (column, row);
                }
            }
            MouseEventKind::ScrollDown => {
                if self.is_in_tree_area(column, row) {
                    self.move_down();
                } else if self.is_in_detail_area(column, row) {
                    self.detail_focused = true;
                    self.move_down();
                }
            }
            MouseEventKind::ScrollUp => {
                if self.is_in_tree_area(column, row) {
                    self.move_up();
                } else if self.is_in_detail_area(column, row) {
                    self.detail_focused = true;
                    self.move_up();
                }
            }
            _ => {}
        }
        Action::Continue
    }

    fn handle_click(&mut self, column: u16, row: u16) {
        // Check if click is in tree area
        if self.is_in_tree_area(column, row) {
            self.detail_focused = false;
            self.handle_tree_click(row);
        } else if self.is_in_detail_area(column, row) {
            self.detail_focused = true;
            // Check if click is on tab area
            if self.is_in_tab_area(column, row) {
                self.handle_tab_click(column);
            } else if self.is_in_table_content_area(column, row) {
                self.handle_table_click(column, row);
            }
        }
    }

    fn handle_double_click(&mut self, column: u16, row: u16) {
        // Double-click in table content area should trigger DOP popup (same as Enter key)
        if self.is_in_table_content_area(column, row) {
            self.detail_focused = true;
            self.try_show_dop_popup();
        }
    }

    fn is_in_tree_area(&self, column: u16, row: u16) -> bool {
        column >= self.tree_area.x && column < self.tree_area.x + self.tree_area.width
            && row >= self.tree_area.y && row < self.tree_area.y + self.tree_area.height
    }

    fn is_in_detail_area(&self, column: u16, row: u16) -> bool {
        column >= self.detail_area.x && column < self.detail_area.x + self.detail_area.width
            && row >= self.detail_area.y && row < self.detail_area.y + self.detail_area.height
    }

    fn is_in_tab_area(&self, column: u16, row: u16) -> bool {
        if let Some(tab_area) = self.tab_area {
            column >= tab_area.x && column < tab_area.x + tab_area.width
                && row >= tab_area.y && row < tab_area.y + tab_area.height
        } else {
            false
        }
    }

    fn is_in_table_content_area(&self, column: u16, row: u16) -> bool {
        if let Some(area) = self.table_content_area {
            column >= area.x && column < area.x + area.width
                && row >= area.y && row < area.y + area.height
        } else {
            false
        }
    }

    fn handle_tab_click(&mut self, column: u16) {
        // Early exits for invalid states
        if self.tab_titles.is_empty() {
            return;
        }

        let Some(tab_area) = self.tab_area else { return };
        
        // Account for border (1 column from left)
        let inner_x = tab_area.x + 1;
        if column < inner_x {
            return;
        }

        let relative_col = (column - inner_x) as usize;
        
        // Calculate which tab was clicked based on tab title positions
        // Tabs are rendered with spacing like: " Tab1 │ Tab2 │ Tab3 "
        let mut current_pos = 0;
        for (i, title) in self.tab_titles.iter().enumerate() {
            let tab_width = title.len() + 2; // +2 for spaces around title
            
            // Check if click falls within this tab
            if relative_col >= current_pos && relative_col < current_pos + tab_width {
                self.selected_tab = i;
                self.status = format!("Switched to tab: {}", title);
                return;
            }
            
            // Move past this tab and its separator
            current_pos += tab_width + 1; // +1 for separator "│"
        }
    }

    fn handle_table_click(&mut self, column: u16, row: u16) {
        let Some(area) = self.table_content_area else { return };
        
        // Validate cursor position
        if self.cursor >= self.visible.len() {
            return;
        }
        
        let node_idx = self.visible[self.cursor];
        let node = &self.all_nodes[node_idx];
        
        // Validate tab index
        if self.selected_tab >= node.detail_sections.len() {
            return;
        }
        
        // Extract table content
        use crate::tree::DetailContent;
        let (rows, constraints) = match &node.detail_sections[self.selected_tab].content {
            DetailContent::Table { rows, constraints, .. } => (rows, constraints),
            _ => return,
        };
        
        // Validate table has content
        if rows.is_empty() {
            return;
        }
        
        // Calculate clicked row (skip header which is 3 lines tall)
        let relative_row = (row - area.y) as usize;
        const HEADER_HEIGHT: usize = 3;
        
        if relative_row < HEADER_HEIGHT {
            return;  // Clicked on header
        }
        
        let clicked_row_idx = (relative_row - HEADER_HEIGHT) + self.section_scrolls[self.selected_tab];
        
        if clicked_row_idx >= rows.len() {
            return;
        }
        
        // Update the row cursor and column
        self.section_cursors[self.selected_tab] = clicked_row_idx;
        
        let relative_col = column - area.x;
        if let Some(col_idx) = self.calculate_clicked_column(relative_col, constraints) {
            self.focused_column = col_idx;
        }
    }

    fn calculate_clicked_column(&self, relative_col: u16, _constraints: &[crate::tree::ColumnConstraint]) -> Option<usize> {
        let area = self.table_content_area?;
        
        if self.cached_ratatui_constraints.is_empty() {
            return None;
        }
        
        // Use ratatui's Layout with the exact constraints used in rendering
        use ratatui::layout::{Layout, Direction};
        
        // Split the area using ratatui's layout - this matches what Table does internally
        let column_areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(self.cached_ratatui_constraints.clone())
            .spacing(3) // Match the column_spacing(3) from Table
            .split(area);
        
        // Find which column area contains the click
        for (idx, col_area) in column_areas.iter().enumerate() {
            let col_start = col_area.x - area.x;
            let col_end = col_start + col_area.width;
            
            if relative_col >= col_start && relative_col < col_end {
                return Some(idx);
            }
        }
        
        // If not in any column (in spacing), find closest
        column_areas.iter().enumerate()
            .map(|(idx, col_area)| {
                let col_center = (col_area.x - area.x) + col_area.width / 2;
                let distance = relative_col.abs_diff(col_center);
                (idx, distance)
            })
            .min_by_key(|(_, dist)| *dist)
            .map(|(idx, _)| idx)
    }

    fn handle_tree_click(&mut self, row: u16) {
        // Calculate which tree item was clicked
        // Account for border (1 line) and title
        let inner_y = self.tree_area.y + 1;
        if row < inner_y || row >= self.tree_area.y + self.tree_area.height - 1 {
            return; // Clicked on border or help text area
        }

        let clicked_line = (row - inner_y) as usize;
        let target_cursor = self.scroll_offset + clicked_line;

        if target_cursor >= self.visible.len() {
            return;
        }

        // If clicking on the same item, toggle expand/collapse
        if target_cursor == self.cursor {
            self.toggle_expand();
        } else {
            self.cursor = target_cursor;
            self.detail_scroll = 0;
        }
    }

}


// Helper functions for service sorting
fn extract_service_id(text: &str) -> u32 {
    // Extract ID from format like "0x10    - ServiceName" or "0x22F501 - ServiceName"
    if let Some(hex_part) = text.strip_prefix("0x")
        && let Some(dash_pos) = hex_part.find(" - ") {
        let id_str = hex_part[..dash_pos].trim();
        return u32::from_str_radix(id_str, 16).unwrap_or(0);
    }
    0
}

fn extract_service_name(text: &str) -> &str {
    // Extract name from format like "0x10    - ServiceName"
    if let Some(dash_pos) = text.find(" - ") {
        return text[dash_pos + 3..].trim();
    }
    text
}
