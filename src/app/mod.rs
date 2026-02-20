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
    FunctionalGroups, // Search only in functional group names
    EcuSharedData,    // Search only in ECU shared data names
    Services,         // Search only in service names
    DiagComms,        // Search only in Diag-Comms sections
    Requests,         // Search only in Requests
    Responses,        // Search only in Responses (Pos and Neg)
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub(crate) enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub(crate) struct TableSortState {
    pub column: usize,
    pub direction: SortDirection,
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
    pub(crate) detail_popup: Option<PopupData>, // Generic popup state
    pub(crate) help_popup_visible: bool, // Help popup visibility
    pub(crate) tree_width_percentage: u16, // Tree pane width (0-100)
    pub(crate) diagcomm_sort_by_id: bool, // true = sort by ID (default), false = sort by name
    pub(crate) row_selection_mode: bool, // true = select by row, false = select by cell
    pub(crate) table_sort_state: Vec<Option<TableSortState>>, // Sort state for each table section (None = default order)
    tree_area: Rect, // Cached tree area for mouse handling
    detail_area: Rect, // Cached detail area for mouse handling
    pub(crate) tab_area: Option<Rect>, // Cached tab area for mouse handling
    pub(crate) tab_titles: Vec<String>, // Cached tab titles for click detection
    pub(crate) table_content_area: Option<Rect>, // Cached table content area for row/cell clicking
    pub(crate) cached_ratatui_constraints: Vec<ratatui::layout::Constraint>, // Exact constraints used in Table
    last_click_time: Option<Instant>, // Time of last click for double-click detection
    last_click_pos: (u16, u16), // Position of last click (column, row)
    pub(crate) mouse_enabled: bool, // Whether mouse input is enabled
    navigation_history: Vec<usize>, // History of cursor positions (node indices in visible)
    history_position: usize, // Current position in history (for potential forward navigation)
}

#[derive(Clone)]
pub struct PopupData {
    pub title: String,
    pub content: Vec<String>,
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
            detail_popup: None,
            help_popup_visible: false,
            tree_width_percentage: 40,
            diagcomm_sort_by_id: true, // Default: sort by ID
            row_selection_mode: true, // Default: row selection mode
            table_sort_state: Vec::new(), // No sorting by default
            tree_area: Rect::default(),
            detail_area: Rect::default(),
            tab_area: None,
            tab_titles: Vec::new(),
            table_content_area: None,
            cached_ratatui_constraints: Vec::new(),
            last_click_time: None,
            last_click_pos: (0, 0),
            mouse_enabled: true, // Mouse enabled by default
            navigation_history: Vec::new(),
            history_position: 0,
        };
        // Apply initial sort order (default is by ID)
        app.sort_diagcomm_nodes_in_place();
        app.rebuild_visible();
        app
    }
    
    /// Get the actual section index accounting for header section offset
    fn get_section_index(&self) -> usize {
        // Check if current node has a header section (rendered above tabs)
        if let Some(&idx) = self.visible.get(self.cursor) {
            let sections = &self.all_nodes[idx].detail_sections;
            if sections.len() > 1 
                && sections[0].render_as_header
                && matches!(&sections[0].content, crate::tree::DetailContent::PlainText(_)) {
                // Has header section, so selected_tab needs offset of 1
                return self.selected_tab + 1;
            }
        }
        self.selected_tab
    }

    /// Get the section offset for rendering (0 or 1 if there's a header section)
    fn get_section_offset(&self) -> usize {
        if let Some(&idx) = self.visible.get(self.cursor) {
            let sections = &self.all_nodes[idx].detail_sections;
            if sections.len() > 1 
                && sections[0].render_as_header
                && matches!(&sections[0].content, crate::tree::DetailContent::PlainText(_)) {
                return 1;
            }
        }
        0
    }

    /// Get the actual table section index for storing/retrieving sort state
    fn get_table_section_idx(&self) -> usize {
        self.selected_tab + self.get_section_offset()
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
        let [main, breadcrumb_bar, status_bar] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(1), Constraint::Length(1)])
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
        self.draw_breadcrumb(frame, breadcrumb_bar);
        self.draw_status(frame, status_bar);
        
        // Draw popups if open (order matters - last drawn is on top)
        if self.detail_popup.is_some() {
            self.draw_detail_popup(frame);
        }
        if self.help_popup_visible {
            self.draw_help_popup(frame);
        }
    }

    // -------------------------------------------------------------------
    // Visibility
    // -------------------------------------------------------------------

    /// Check if a node at index i is under a specific section header
    fn is_under_section(&self, node_idx: usize, section_name: &str) -> bool {
        if node_idx == 0 {
            return false;
        }
        
        let node_depth = self.all_nodes[node_idx].depth;
        
        // Search backwards from node_idx to find parent section
        for i in (0..node_idx).rev() {
            let parent = &self.all_nodes[i];
            
            // Stop if we reach a node at the same or lower depth (not a parent)
            if parent.depth >= node_depth {
                continue;
            }
            
            // If this is a section header at depth 0, check if it matches
            if parent.depth == 0 && parent.text == section_name {
                return true;
            }
        }
        
        false
    }

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
                            // Match variant containers and the "Variants" header
                            self.all_nodes[i].text == "Variants" ||
                            (matches!(self.all_nodes[i].node_type, NodeType::Container) &&
                             i > 0 && self.is_under_section(i, "Variants"))
                        },
                        SearchScope::FunctionalGroups => {
                            // Match functional group containers and the "Functional Groups" header
                            self.all_nodes[i].text == "Functional Groups" ||
                            (matches!(self.all_nodes[i].node_type, NodeType::Container) &&
                             i > 0 && self.is_under_section(i, "Functional Groups"))
                        },
                        SearchScope::EcuSharedData => {
                            // Match ECU shared data containers and the "ECU Shared Data" header
                            self.all_nodes[i].text == "ECU Shared Data" ||
                            (matches!(self.all_nodes[i].node_type, NodeType::Container) &&
                             i > 0 && self.is_under_section(i, "ECU Shared Data"))
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
                        SearchScope::Requests => {
                            self.all_nodes[i].text.starts_with("Requests (") ||
                            matches!(self.all_nodes[i].node_type, NodeType::Request)
                        },
                        SearchScope::Responses => {
                            self.all_nodes[i].text.starts_with("Pos-Responses (") ||
                            self.all_nodes[i].text.starts_with("Neg-Responses (") ||
                            matches!(self.all_nodes[i].node_type, 
                                NodeType::PosResponse | NodeType::NegResponse)
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

    pub(crate) fn toggle_table_column_sort(&mut self) {
        // Only works when detail pane is focused
        if !self.detail_focused {
            return;
        }
        
        let section_idx = self.get_table_section_idx();
        
        // Ensure we have enough entries in table_sort_state
        while self.table_sort_state.len() <= section_idx {
            self.table_sort_state.push(None);
        }
        
        let column = self.focused_column;
        
        // Toggle sort state: if already sorting by this column, toggle direction, otherwise sort ascending by this column
        self.table_sort_state[section_idx] = match self.table_sort_state[section_idx] {
            Some(state) if state.column == column => {
                // Same column clicked: toggle direction
                let new_direction = match state.direction {
                    SortDirection::Ascending => SortDirection::Descending,
                    SortDirection::Descending => SortDirection::Ascending,
                };
                Some(TableSortState {
                    column,
                    direction: new_direction,
                })
            }
            _ => {
                // Different column or no sort: sort ascending by this column
                Some(TableSortState {
                    column,
                    direction: SortDirection::Ascending,
                })
            }
        };
        
        // Update status message
        if let Some(state) = self.table_sort_state[section_idx] {
            let direction_str = match state.direction {
                SortDirection::Ascending => "▲",
                SortDirection::Descending => "▼",
            };
            self.status = format!("Sort by column {} {}", state.column, direction_str);
        }
    }

    // -------------------------------------------------------------------
    // Navigation history
    // -------------------------------------------------------------------
    
    /// Add current cursor position to navigation history
    fn push_to_history(&mut self) {
        // Only store if cursor is in valid range
        if self.cursor >= self.visible.len() {
            return;
        }
        
        // Don't store duplicate consecutive positions
        if let Some(&last) = self.navigation_history.last() {
            if last == self.cursor {
                return;
            }
        }
        
        // If we're not at the end of history, truncate forward history
        if self.history_position < self.navigation_history.len() {
            self.navigation_history.truncate(self.history_position);
        }
        
        // Add current position
        self.navigation_history.push(self.cursor);
        self.history_position = self.navigation_history.len();
        
        // Limit history size to prevent unbounded growth
        const MAX_HISTORY: usize = 100;
        if self.navigation_history.len() > MAX_HISTORY {
            self.navigation_history.remove(0);
            self.history_position = self.navigation_history.len();
        }
    }
    
    /// Navigate back in history
    pub(crate) fn navigate_back(&mut self) {
        // Need at least 2 entries to go back (current + previous)
        if self.navigation_history.len() < 2 || self.history_position <= 1 {
            self.status = "No previous location in history".to_owned();
            return;
        }
        
        // Move back in history
        self.history_position = self.history_position.saturating_sub(1);
        let target_cursor = self.navigation_history[self.history_position - 1];
        
        // Navigate to the stored position
        if target_cursor < self.visible.len() {
            self.cursor = target_cursor;
            self.detail_scroll = 0;
            self.scroll_offset = self.cursor.saturating_sub(5); // Center the view
            self.status = format!("Navigated back (history: {}/{})", 
                self.history_position, 
                self.navigation_history.len());
        } else {
            // Position is no longer valid (tree structure changed)
            self.status = "Previous location no longer valid".to_owned();
        }
    }

    // -------------------------------------------------------------------
    // Cursor movement
    // -------------------------------------------------------------------

    pub(crate) fn move_up(&mut self) {
        if self.detail_focused {
            // Move cursor up in the detail pane (typically a table row)
            let section_idx = self.get_section_index();
            if section_idx < self.section_cursors.len() {
                self.section_cursors[section_idx] = 
                    self.section_cursors[section_idx].saturating_sub(1);
            }
        } else {
            let old_cursor = self.cursor;
            self.cursor = self.cursor.saturating_sub(1);
            self.detail_scroll = 0;
            
            // Only push to history if we moved to a different node
            if old_cursor != self.cursor {
                self.push_to_history();
            }
        }
    }

    pub(crate) fn move_down(&mut self) {
        if self.detail_focused {
            // Move cursor down in the detail pane (typically a table row)
            let section_idx = self.get_section_index();
            if section_idx < self.section_cursors.len() {
                self.section_cursors[section_idx] = 
                    self.section_cursors[section_idx].saturating_add(1);
            }
        } else if self.cursor + 1 < self.visible.len() {
            let old_cursor = self.cursor;
            self.cursor += 1;
            self.detail_scroll = 0;
            
            // Only push to history if we moved to a different node
            if old_cursor != self.cursor {
                self.push_to_history();
            }
        }
    }

    pub(crate) fn page_up(&mut self, n: usize) {
        if self.detail_focused {
            let section_idx = self.get_section_index();
            if section_idx < self.section_cursors.len() {
                self.section_cursors[section_idx] = 
                    self.section_cursors[section_idx].saturating_sub(n);
            }
        } else {
            let old_cursor = self.cursor;
            self.cursor = self.cursor.saturating_sub(n);
            self.detail_scroll = 0;
            if old_cursor != self.cursor {
                self.push_to_history();
            }
        }
    }

    pub(crate) fn page_down(&mut self, n: usize) {
        if self.detail_focused {
            let section_idx = self.get_section_index();
            if section_idx < self.section_cursors.len() {
                self.section_cursors[section_idx] = 
                    self.section_cursors[section_idx].saturating_add(n);
            }
        } else {
            let old_cursor = self.cursor;
            self.cursor = (self.cursor + n).min(self.visible.len().saturating_sub(1));
            self.detail_scroll = 0;
            if old_cursor != self.cursor {
                self.push_to_history();
            }
        }
    }

    pub(crate) fn home(&mut self) {
        if self.detail_focused {
            let section_idx = self.get_section_index();
            if section_idx < self.section_cursors.len() {
                self.section_cursors[section_idx] = 0;
            }
        } else {
            let old_cursor = self.cursor;
            self.cursor = 0;
            self.detail_scroll = 0;
            if old_cursor != self.cursor {
                self.push_to_history();
            }
        }
    }

    pub(crate) fn end(&mut self) {
        if self.detail_focused {
            let section_idx = self.get_section_index();
            if section_idx < self.section_cursors.len() {
                self.section_cursors[section_idx] = usize::MAX; // clamped during render
            }
        } else {
            let old_cursor = self.cursor;
            self.cursor = self.visible.len().saturating_sub(1);
            self.detail_scroll = 0;
            if old_cursor != self.cursor {
                self.push_to_history();
            }
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
                        SearchScope::FunctionalGroups => "[FG]",
                        SearchScope::EcuSharedData => "[ESD]",
                        SearchScope::Services => "[S]",
                        SearchScope::DiagComms => "[D]",
                        SearchScope::Requests => "[Rq]",
                        SearchScope::Responses => "[Rs]",
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
            SearchScope::Variants => SearchScope::FunctionalGroups,
            SearchScope::FunctionalGroups => SearchScope::EcuSharedData,
            SearchScope::EcuSharedData => SearchScope::Services,
            SearchScope::Services => SearchScope::DiagComms,
            SearchScope::DiagComms => SearchScope::Requests,
            SearchScope::Requests => SearchScope::Responses,
            SearchScope::Responses => SearchScope::All,
        };
        
        let scope_name = match self.search_scope {
            SearchScope::All => "All",
            SearchScope::Variants => "Variants",
            SearchScope::FunctionalGroups => "Functional Groups",
            SearchScope::EcuSharedData => "ECU Shared Data",
            SearchScope::Services => "Services",
            SearchScope::DiagComms => "Diag-Comms",
            SearchScope::Requests => "Requests",
            SearchScope::Responses => "Responses",
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

    /// Try to show a popup based on the current row selection
    pub(crate) fn try_show_detail_popup(&mut self) {
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
        let section_index = self.get_section_index();
        if section_index >= self.section_cursors.len() {
            self.status = "Section cursor not initialized".to_string();
            return;
        }
        let row_cursor = self.section_cursors[section_index];

        // Validate tab exists
        let sections = &node.detail_sections;
        if section_index >= sections.len() {
            self.status = format!("Tab {} out of {} tabs", section_index, sections.len());
            return;
        }

        let section = &sections[section_index];
        
        // Extract rows from table content
        use crate::tree::DetailContent;
        let rows = match &section.content {
            DetailContent::Table { rows, .. } => rows,
            _ => {
                self.status = "Popup only available for table rows".to_owned();
                return;
            }
        };
        
        // Validate row exists
        if row_cursor >= rows.len() {
            self.status = format!("Row {} out of {} lines", row_cursor, rows.len());
            return;
        }

        // Get the selected row
        let selected_row: &DetailRow = &rows[row_cursor];
        let cells = &selected_row.cells;
        let cell_types = &selected_row.cell_types;
        
        // Try to find a reference cell (like DOP reference) to show details for
        let reference_cell_index = cell_types.iter().position(|ct| matches!(ct, CellType::DopReference));
        
        if let Some(ref_idx) = reference_cell_index {
            // Found a reference cell - show popup for it
            if ref_idx < cells.len() && !cells[ref_idx].is_empty() {
                let reference_name = cells[ref_idx].to_owned();
                
                // Extract additional context from the row
                let param_name = cell_types.iter()
                    .position(|ct| matches!(ct, CellType::ParameterName))
                    .and_then(|idx| cells.get(idx))
                    .map(|s| s.as_str())
                    .unwrap_or("unknown");
                
                // Build generic popup content
                let mut content = vec![
                    format!("Reference: {}", reference_name),
                    format!("Context: {}", param_name),
                    String::new(),
                ];
                
                // Add all cell data for debugging/inspection
                for (i, (cell, cell_type)) in cells.iter().zip(cell_types.iter()).enumerate() {
                    if i != ref_idx && !cell.is_empty() {
                        content.push(format!("{:?}: {}", cell_type, cell));
                    }
                }
                
                content.push(String::new());
                content.push("(Details would be loaded from data source)".to_owned());

                self.status = format!("Opening details for: {}", reference_name);
                self.detail_popup = Some(PopupData { 
                    title: reference_name, 
                    content,
                });
            } else {
                self.status = "Reference cell is empty".to_owned();
            }
        } else {
            self.status = "No reference in this row".to_owned();
        }
    }

    /// Navigate to a service in the tree from a service list table (Diag-Comms, Requests, Responses)
    pub(crate) fn try_navigate_to_service(&mut self) {
        // Validate cursor position and that we're on a service list header
        if self.cursor >= self.visible.len() {
            self.status = "Cursor out of bounds".to_string();
            return;
        }
        
        let node_idx = self.visible[self.cursor];
        let node = &self.all_nodes[node_idx];
        
        // Check if this is a service list section header
        if !node.text.starts_with("Diag-Comms (") 
            && !node.text.starts_with("Requests (") 
            && !node.text.starts_with("Pos-Responses (") 
            && !node.text.starts_with("Neg-Responses (") {
            self.status = "Not a service list section".to_owned();
            return;
        }
        
        // Validate we have the details
        if node.detail_sections.is_empty() {
            self.status = "No details available".to_string();
            return;
        }

        let section = &node.detail_sections[0]; // The service list table is always first
        
        // Extract the short name from the table
        use crate::tree::DetailContent;
        let rows = match &section.content {
            DetailContent::Table { rows, .. } => rows,
            _ => {
                self.status = "Diag-Comms details should be a table".to_owned();
                return;
            }
        };
        
        // Get the selected row
        let section_index = self.get_section_index();
        if section_index >= self.section_cursors.len() {
            self.status = "Section cursor not initialized".to_string();
            return;
        }
        let row_cursor = self.section_cursors[section_index];
        
        if row_cursor >= rows.len() {
            self.status = format!("Row {} out of {} lines", row_cursor, rows.len());
            return;
        }

        let selected_row = &rows[row_cursor];
        if selected_row.cells.len() < 2 {
            self.status = "Invalid row structure".to_owned();
            return;
        }
        
        // The short name is in the second column
        let service_name = selected_row.cells[1].clone();
        
        // Expand the service list section if it's collapsed
        if !self.all_nodes[node_idx].expanded {
            self.all_nodes[node_idx].expanded = true;
            self.rebuild_visible();
            // After expanding, the cursor position in visible might have changed
            // Re-find the service list node in visible
            if let Some(new_cursor) = self.visible.iter().position(|&idx| idx == node_idx) {
                self.cursor = new_cursor;
            }
        }
        
        // Find the service node in the tree that matches this name
        // We need to search for a child of the current service list node with a matching service name
        let service_list_depth = self.all_nodes[node_idx].depth;
        
        // Look for service nodes immediately after this Diag-Comms node
        // Service nodes have depth = diag_comms_depth + 1
        let mut found_idx: Option<usize> = None;
        
        // Find all visible indices for children of this service list node
        for &vis_idx in &self.visible[self.cursor + 1..] {
            let child_node = &self.all_nodes[vis_idx];
            
            // Stop if we've reached a node at the same or lower depth (left the service list section)
            if child_node.depth <= service_list_depth {
                break;
            }
            
            // Skip nodes not directly under the service list (must be immediate children)
            if child_node.depth != service_list_depth + 1 {
                continue;
            }
            
            // Check if this is a service-related node (generic check for all service types)
            let is_service_node = matches!(child_node.node_type,
                NodeType::Service | NodeType::ParentRefService | 
                NodeType::Request | NodeType::PosResponse | NodeType::NegResponse);
            
            if !is_service_node {
                continue;
            }
            
            // Check if this service node contains the service name in its text
            // The text format is "0xXXXX - ServiceName"
            if child_node.text.contains(&service_name) {
                // Find the position of this node in visible
                if let Some(pos) = self.visible.iter().position(|&idx| idx == vis_idx) {
                    found_idx = Some(pos);
                    break;
                }
            }
        }
        
        if let Some(target_cursor) = found_idx {
            // Navigate tree focus to this service
            self.push_to_history(); // Store old position before jumping
            self.detail_focused = false;
            self.cursor = target_cursor;
            self.scroll_offset = self.cursor.saturating_sub(5); // Center the view
        } else {
            self.status = format!("Service '{}' not found in tree", service_name);
        }
    }
    
    /// Navigate to an inherited parent layer in the tree
    pub(crate) fn try_navigate_to_inherited_parent(&mut self) {
        // Validate cursor position
        if self.cursor >= self.visible.len() {
            return;
        }
        
        let node_idx = self.visible[self.cursor];
        let node = &self.all_nodes[node_idx];
        
        // We should be on any service-related node (generic check)
        if !matches!(node.node_type, 
            NodeType::Service | NodeType::ParentRefService | 
            NodeType::Request | NodeType::PosResponse | NodeType::NegResponse) {
            self.status = "Not a service node".to_owned();
            return;
        }
        
        // Get the current service name from the node text (format: "0xXXXX - ServiceName")
        let current_service_name = if let Some(dash_idx) = node.text.find(" - ") {
            node.text[dash_idx + 3..].to_string()
        } else {
            node.text.clone()
        };
        
        // Find the Overview section
        // If there's a header section (render_as_header = true), Overview is at index 1, otherwise 0
        if node.detail_sections.is_empty() {
            return;
        }
        
        let overview_idx = if node.detail_sections.len() > 1 
            && node.detail_sections[0].render_as_header {
            1  // Header exists, Overview is second
        } else {
            0  // No header, Overview is first
        };
        
        if overview_idx >= node.detail_sections.len() {
            return;
        }
        
        let overview_section = &node.detail_sections[overview_idx];
        
        // Extract the parent layer name from the Overview table
        use crate::tree::DetailContent;
        let rows = match &overview_section.content {
            DetailContent::Table { rows, .. } => rows,
            _ => return,
        };
        
        // Get the selected row in the Overview section
        // Use overview_idx directly as it's the actual section index in section_cursors
        let row_cursor = if overview_idx < self.section_cursors.len() {
            self.section_cursors[overview_idx]
        } else {
            return;
        };
        
        if row_cursor >= rows.len() {
            return;
        }
        
        let selected_row = &rows[row_cursor];
        
        // Check if this is the "Inherited From" row
        if selected_row.cells.len() >= 2 && selected_row.cells[0] == "Inherited From" {
            let parent_layer_name = selected_row.cells[1].clone();
            
            // Search for a node in the tree that matches this parent layer name
            // We need to find variants, protocols, functional groups, or EcuSharedData nodes with this name
            let mut found_container_idx: Option<usize> = None;
            
            // Search through ALL nodes, not just visible ones
            for (ni, n) in self.all_nodes.iter().enumerate() {
                // Check if this is a Container (variant/functional group) with matching name
                if matches!(n.node_type, NodeType::Container) {
                    // Extract name from text (may include [base] suffix)
                    let name = if let Some(idx) = n.text.find(" [") {
                        &n.text[..idx]
                    } else {
                        &n.text
                    };
                    
                    if name == parent_layer_name {
                        found_container_idx = Some(ni);
                        break;
                    }
                }
            }
            
            if let Some(container_node_idx) = found_container_idx {
                // Expand all parents of the target node to make it visible
                let container_depth = self.all_nodes[container_node_idx].depth;
                
                // Expand parent nodes
                if container_depth > 0 {
                    for i in 0..container_node_idx {
                        let node = &mut self.all_nodes[i];
                        if node.depth < container_depth && node.has_children {
                            node.expanded = true;
                        }
                    }
                }
                
                // Expand the container node itself if it has children
                if self.all_nodes[container_node_idx].has_children {
                    self.all_nodes[container_node_idx].expanded = true;
                }
                
                // Now find and expand the Diag-Comms section within the container
                let mut diagcomm_node_idx: Option<usize> = None;
                for i in (container_node_idx + 1)..self.all_nodes.len() {
                    let child_node = &self.all_nodes[i];
                    
                    // Stop if we've left the container
                    if child_node.depth <= container_depth {
                        break;
                    }
                    
                    // Look for the Diag-Comms section
                    if child_node.depth == container_depth + 1 && child_node.text.starts_with("Diag-Comms (") {
                        diagcomm_node_idx = Some(i);
                        break;
                    }
                }
                
                // If we found the Diag-Comms section, expand it and find the specific service
                if let Some(dc_idx) = diagcomm_node_idx {
                    // Expand the Diag-Comms section
                    self.all_nodes[dc_idx].expanded = true;
                    
                    // Rebuild visible list
                    self.rebuild_visible();
                    
                    // Now find the specific service node within the Diag-Comms section
                    let diagcomm_depth = self.all_nodes[dc_idx].depth;
                    let mut found_service_idx: Option<usize> = None;
                    
                    for i in (dc_idx + 1)..self.all_nodes.len() {
                        let service_node = &self.all_nodes[i];
                        
                        // Stop if we've left the Diag-Comms section
                        if service_node.depth <= diagcomm_depth {
                            break;
                        }
                        
                        // Look for service nodes at depth diagcomm_depth + 1
                        if service_node.depth == diagcomm_depth + 1 {
                            // Check if this service matches our service name
                            if service_node.text.contains(&current_service_name) {
                                found_service_idx = Some(i);
                                break;
                            }
                        }
                    }
                    
                    // Navigate to the service if found, otherwise to the container
                    let target_idx = found_service_idx.unwrap_or(container_node_idx);
                    
                    // Find the new position of the target node in visible
                    if let Some(new_cursor) = self.visible.iter().position(|&idx| idx == target_idx) {
                        self.push_to_history(); // Store old position before jumping
                        self.detail_focused = false;
                        self.cursor = new_cursor;
                        self.scroll_offset = self.cursor.saturating_sub(5); // Center the view
                    }
                } else {
                    // No Diag-Comms section found, just navigate to the container
                    self.rebuild_visible();
                    
                    if let Some(new_cursor) = self.visible.iter().position(|&idx| idx == container_node_idx) {
                        self.push_to_history();
                        self.detail_focused = false;
                        self.cursor = new_cursor;
                        self.scroll_offset = self.cursor.saturating_sub(5);
                    }
                }
            } else {
                self.status = format!("Parent layer '{}' not found in tree", parent_layer_name);
            }
        }
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
        if self.detail_popup.is_some() {
            if matches!(kind, MouseEventKind::Down(_)) {
                self.detail_popup = None;
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
                    // First handle the click to update cursor position
                    self.handle_click(column, row);
                    // Then handle the double-click action
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
                self.handle_tab_click(column, row);
            } else if self.is_in_table_content_area(column, row) {
                self.handle_table_click(column, row);
            }
        }
    }

    fn handle_double_click(&mut self, column: u16, row: u16) {
        // Double-click in table content area should trigger navigation or DOP popup
        if self.is_in_table_content_area(column, row) {
            self.detail_focused = true;
            
            // Check what type of node we're on
            if self.cursor < self.visible.len() {
                let node_idx = self.visible[self.cursor];
                let node = &self.all_nodes[node_idx];
                
                // Check if this is a service list header (generic check)
                let is_service_list = node.text.starts_with("Diag-Comms (") 
                    || node.text.starts_with("Requests (") 
                    || node.text.starts_with("Pos-Responses (") 
                    || node.text.starts_with("Neg-Responses (");
                
                // Check if this is any service-related node type (generic check)
                let is_service_node = matches!(node.node_type, 
                    NodeType::Service | NodeType::ParentRefService | 
                    NodeType::Request | NodeType::PosResponse | NodeType::NegResponse);
                
                if is_service_list {
                    // Navigate to selected service from service list table
                    self.try_navigate_to_service();
                } else if is_service_node {
                    // Check if we're on the "Inherited From" row in Overview
                    let mut should_navigate_to_parent = false;
                    
                    // Get the actual section index accounting for header section offset
                    let section_idx = self.get_section_index();
                    
                    if section_idx < node.detail_sections.len() {
                        let section = &node.detail_sections[section_idx];
                        if section.title == "Overview" {
                            if let crate::tree::DetailContent::Table { rows, .. } = &section.content {
                                let row_cursor = if section_idx < self.section_cursors.len() {
                                    self.section_cursors[section_idx]
                                } else {
                                    0
                                };
                                
                                if row_cursor < rows.len() {
                                    let selected_row = &rows[row_cursor];
                                    if selected_row.cells.len() >= 2 && selected_row.cells[0] == "Inherited From" {
                                        should_navigate_to_parent = true;
                                    }
                                }
                            }
                        }
                    }
                    
                    if should_navigate_to_parent {
                        self.try_navigate_to_inherited_parent();
                    } else {
                        // Default: try to show DOP popup for other rows
                        self.try_show_detail_popup();
                    }
                } else {
                    // Try to show DOP popup
                    self.try_show_detail_popup();
                }
            }
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

    fn handle_tab_click(&mut self, column: u16, row: u16) {
        // Early exits for invalid states
        if self.tab_titles.is_empty() {
            return;
        }

        let Some(tab_area) = self.tab_area else { return };
        
        // Account for border (1 column from left, 1 row from top)
        let inner_x = tab_area.x + 1;
        let inner_y = tab_area.y + 1;
        
        if column < inner_x || row < inner_y {
            return;
        }

        let relative_col = (column - inner_x) as usize;
        let relative_row = (row - inner_y) as usize;
        
        // Calculate available width for tabs
        let available_width = (tab_area.width.saturating_sub(2)) as usize; // -2 for borders
        
        // Build tab strings with decorators to match rendering logic
        let tab_strings: Vec<String> = self.tab_titles.iter()
            .map(|title| format!(" {} ", title))
            .collect();
        
        // Simulate tab wrapping to determine which line each tab is on
        let mut lines: Vec<Vec<usize>> = Vec::new();
        let mut current_line: Vec<usize> = Vec::new();
        let mut current_width = 0;
        
        for (idx, tab_str) in tab_strings.iter().enumerate() {
            let tab_width = tab_str.len() + 1; // +1 for separator
            
            if current_width + tab_width > available_width && !current_line.is_empty() {
                // Start a new line
                lines.push(current_line);
                current_line = Vec::new();
                current_width = 0;
            }
            
            current_line.push(idx);
            current_width += tab_width;
        }
        
        if !current_line.is_empty() {
            lines.push(current_line);
        }
        
        // Determine which line was clicked
        if relative_row >= lines.len() {
            return;
        }
        
        let clicked_line_tabs = &lines[relative_row];
        
        // Calculate tab positions on the clicked line
        let mut current_pos = 0;
        for (i, &tab_idx) in clicked_line_tabs.iter().enumerate() {
            let tab_str = &tab_strings[tab_idx];
            let separator_width = if i == 0 { 0 } else { 1 }; // "│" separator before tab
            
            // Check if click falls within this tab
            if relative_col >= current_pos + separator_width && relative_col < current_pos + separator_width + tab_str.len() {
                self.selected_tab = tab_idx;
                return;
            }
            
            // Move past this tab and its separator
            current_pos += separator_width + tab_str.len();
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
        
        // Get the correct section index (accounting for header section offset)
        let section_idx = self.get_section_index();
        
        // Validate section index
        if section_idx >= node.detail_sections.len() {
            return;
        }
        
        // Extract table content
        use crate::tree::DetailContent;
        let (rows, use_row_selection) = match &node.detail_sections[section_idx].content {
            DetailContent::Table { rows, use_row_selection, .. } => (rows, *use_row_selection),
            _ => return,
        };
        
        // Validate table has content
        if rows.is_empty() {
            return;
        }
        
        // Ensure section cursors and scrolls are properly sized
        while self.section_scrolls.len() <= section_idx {
            self.section_scrolls.push(0);
        }
        while self.section_cursors.len() <= section_idx {
            self.section_cursors.push(0);
        }
        
        // Calculate clicked row (skip header which is 3 lines tall)
        let relative_row = (row - area.y) as usize;
        const HEADER_HEIGHT: usize = 3;
        
        if relative_row < HEADER_HEIGHT {
            return;  // Clicked on header
        }
        
        let clicked_row_idx = (relative_row - HEADER_HEIGHT) + self.section_scrolls[section_idx];
        
        if clicked_row_idx >= rows.len() {
            return;
        }
        
        // Update the row cursor
        self.section_cursors[section_idx] = clicked_row_idx;
        
        // For tables with row selection mode, only select by row
        // For cell selection mode, also update the focused column
        if !use_row_selection {
            let relative_col = column - area.x;
            if let Some(col_idx) = self.calculate_clicked_column(relative_col) {
                self.focused_column = col_idx;
            }
        }
    }

    fn calculate_clicked_column(&self, relative_col: u16) -> Option<usize> {
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
            self.push_to_history(); // Store old position before jumping
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
