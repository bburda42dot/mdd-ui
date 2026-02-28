/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2026 Alexander Mohr
 */

mod column_widths;
pub(crate) mod config;
mod cursor;
mod history;
mod input;
mod mouse;
mod navigation;
mod render;
mod search;
mod sort;
mod visibility;

use std::{collections::HashMap, io, time::Instant};

use config::ResolvedTheme;
use crossterm::event::{self, Event, KeyEventKind, KeyModifiers};
use input::Action;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout, Rect},
};

use crate::tree::{DetailRow, DetailSectionType, NodeType, TreeNode};

// -----------------------------------------------------------------------
// Layout & interaction constants
// -----------------------------------------------------------------------

pub(crate) const COLUMN_SPACING: u16 = 3;
pub(crate) const PAGE_SIZE: usize = 20;
pub(crate) const SCROLL_CONTEXT_LINES: usize = 5;
pub(crate) const TREE_WIDTH_STEP: u16 = 5;
pub(crate) const COMPOSITE_SCROLL_STEP: usize = 5;
pub(crate) const DOUBLE_CLICK_MS: u64 = 500;
pub(crate) const DIVIDER_MIN_PCT: u16 = 20;
pub(crate) const DIVIDER_MAX_PCT: u16 = 80;
pub(crate) const DEFAULT_TREE_WIDTH_PCT: u16 = 35;

// -----------------------------------------------------------------------
// Application state
// -----------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) enum SearchScope {
    #[default]
    All, // Search everywhere
    Variants,         // Search only in variant names
    FunctionalGroups, // Search only in functional group names
    EcuSharedData,    // Search only in ECU shared data names
    Services,         // Search only in service names
    DiagComms,        // Search only in Diag-Comms sections
    Requests,         // Search only in Requests
    Responses,        // Search only in Responses (Pos and Neg)
    Subtree {
        start_idx: usize,
        end_idx: usize,
        root_name: String,
    },
}

impl std::fmt::Display for SearchScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchScope::All => write!(f, "All"),
            SearchScope::Variants => write!(f, "Variants"),
            SearchScope::FunctionalGroups => write!(f, "functional groups"),
            SearchScope::EcuSharedData => write!(f, "ECU shared data"),
            SearchScope::Services => write!(f, "Services"),
            SearchScope::DiagComms => write!(f, "Diag-Comms"),
            SearchScope::Requests => write!(f, "Requests"),
            SearchScope::Responses => write!(f, "Responses"),
            SearchScope::Subtree { root_name, .. } => write!(f, "in {root_name}"),
        }
    }
}

impl SearchScope {
    /// Returns the scope indicator for search mode (e.g., " [variants]")
    pub(crate) fn search_indicator(&self) -> &str {
        match self {
            SearchScope::All => "",
            SearchScope::Variants => " [variants]",
            SearchScope::FunctionalGroups => " [functional groups]",
            SearchScope::EcuSharedData => " [ECU shared data]",
            SearchScope::Services => " [services]",
            SearchScope::DiagComms => " [diag-comms]",
            SearchScope::Requests => " [requests]",
            SearchScope::Responses => " [responses]",
            SearchScope::Subtree { .. } => " [subtree]",
        }
    }

    /// Returns the scope indicator for status line (e.g., " | scope: variants")
    pub(crate) fn status_indicator(&self) -> &str {
        match self {
            SearchScope::All => "",
            SearchScope::Variants => " | scope: variants",
            SearchScope::FunctionalGroups => " | scope: functional groups",
            SearchScope::EcuSharedData => " | scope: ECU shared data",
            SearchScope::Services => " | scope: services",
            SearchScope::DiagComms => " | scope: diag-comms",
            SearchScope::Requests => " | scope: requests",
            SearchScope::Responses => " | scope: responses",
            SearchScope::Subtree { .. } => " | scope: subtree",
        }
    }

    /// Returns the abbreviated scope indicator (e.g., "[V]" for Variants)
    pub(crate) fn abbrev(&self) -> &str {
        match self {
            SearchScope::All => "",
            SearchScope::Variants => "[V]",
            SearchScope::FunctionalGroups => "[FG]",
            SearchScope::EcuSharedData => "[ESD]",
            SearchScope::Services => "[S]",
            SearchScope::DiagComms => "[D]",
            SearchScope::Requests => "[Rq]",
            SearchScope::Responses => "[Rs]",
            SearchScope::Subtree { .. } => "[ST]",
        }
    }
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
    /// Optional secondary column for tie-breaking (e.g., Bit position after Byte)
    pub secondary_column: Option<usize>,
}

#[derive(Clone, Debug, Copy, PartialEq, Eq, Default)]
pub(crate) enum DragState {
    #[default]
    None,
    Divider,
    TreeScrollbar,
    DetailScrollbar,
    DetailHScrollbar,
    ColumnBorder(usize),
}

/// Key for persisting column widths across element switches
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ColumnWidthCacheKey {
    section_type: DetailSectionType,
    title: String,
    column_count: usize,
}

/// Which pane currently has keyboard focus.
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum FocusState {
    /// The tree navigation pane.
    Tree,
    /// The detail/table pane.
    Detail,
    /// The help popup overlay.
    HelpPopup,
}

// -----------------------------------------------------------------------
// State sub-structs
// -----------------------------------------------------------------------

/// Tree navigation and node state
#[derive(Default)]
pub(crate) struct TreeState {
    pub all_nodes: Vec<TreeNode>,
    pub visible: Vec<usize>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub diagcomm_sort_by_id: bool, // true = sort by ID (default), false = sort by name
}

/// Search-related state
#[derive(Default)]
pub(crate) struct SearchState {
    pub query: String,
    pub active: bool,
    pub stack: Vec<(String, SearchScope)>, // Stack of (search_term, scope) pairs
    pub scope: SearchScope,
    pub matches: Vec<usize>,
    pub match_cursor: usize,
}

/// Detail pane state
#[derive(Default)]
pub(crate) struct DetailState {
    pub selected_tab: usize,
    pub focused_section: usize,
    pub section_scrolls: Vec<usize>,
    pub section_cursors: Vec<usize>,
    pub popup: Option<PopupData>,
    pub composite_scroll: Vec<usize>,
    pub composite_max_scroll: usize,
    pub last_diagcomm_tab: usize,
    pub last_section_tabs: HashMap<DetailSectionType, usize>,
    pub last_selected_section_type: Option<DetailSectionType>,
    pub last_selected_section_title: Option<String>,
}

/// Table column and scrolling state
#[derive(Default)]
pub(crate) struct TableState {
    pub column_widths: Vec<Vec<u16>>,
    pub column_widths_absolute: Vec<bool>,
    pub horizontal_scroll: Vec<u16>,
    pub persisted_column_widths: HashMap<ColumnWidthCacheKey, Vec<u16>>,
    pub focused_column: usize,
    pub sort_state: Vec<Option<TableSortState>>,
    pub cached_ratatui_constraints: Vec<ratatui::layout::Constraint>,
    pub cached_total_table_width: u16,
    pub jump_buffer: String,
    pub jump_buffer_time: Option<Instant>,
}

/// Mouse interaction state
#[derive(Default)]
pub(crate) struct MouseState {
    pub drag_state: DragState,
    pub last_click_time: Option<Instant>,
    pub last_click_pos: (u16, u16),
    pub enabled: bool,
}

/// Navigation history state
#[derive(Default)]
pub(crate) struct HistoryState {
    pub entries: Vec<HistoryEntry>,
    pub position: usize,
}

/// Cached layout areas for mouse handling
#[derive(Default)]
pub(crate) struct LayoutCache {
    pub tree_area: Rect,
    pub detail_area: Rect,
    pub tab_area: Option<Rect>,
    pub tab_titles: Vec<String>,
    pub table_content_area: Option<Rect>,
    pub breadcrumb_area: Rect,
    pub breadcrumb_segments: Vec<(String, usize, u16, u16)>,
    pub tree_scrollbar_area: Option<Rect>,
    pub detail_scrollbar_area: Option<Rect>,
    pub detail_hscrollbar_area: Option<Rect>,
    pub tree_width_percentage: u16,
}

/// Main application state, owning all sub-states and driving the TUI event loop.
pub struct App {
    pub(crate) tree: TreeState,
    pub(crate) search: SearchState,
    pub(crate) detail: DetailState,
    pub(crate) table: TableState,
    pub(crate) mouse: MouseState,
    pub(crate) history: HistoryState,
    pub(crate) layout: LayoutCache,
    pub(crate) status: String,
    pub(crate) focus_state: FocusState,
    pub(crate) theme: ResolvedTheme,
}

/// Data for a popup overlay (e.g. DOP reference details).
#[derive(Clone)]
pub struct PopupData {
    /// Title displayed in the popup border.
    pub title: String,
    /// Lines of content displayed inside the popup.
    pub content: Vec<String>,
}

/// A single entry in the navigation history, storing the node index and the
/// full path from root so that the target can be found even after
/// expand/collapse changes.
#[derive(Clone)]
pub(crate) struct HistoryEntry {
    node_idx: usize,
    /// Path from root to target: `(depth, text)` pairs.
    node_path: Vec<(usize, String)>,
}

impl App {
    pub fn new(nodes: Vec<TreeNode>, theme: ResolvedTheme) -> Self {
        let mut app = Self {
            tree: TreeState {
                all_nodes: nodes,
                visible: Vec::new(),
                cursor: 0,
                scroll_offset: 0,
                diagcomm_sort_by_id: true,
            },
            search: SearchState {
                query: String::new(),
                active: false,
                stack: Vec::new(),
                scope: SearchScope::All,
                matches: Vec::new(),
                match_cursor: 0,
            },
            detail: DetailState {
                selected_tab: 0,
                focused_section: 0,
                section_scrolls: Vec::new(),
                section_cursors: Vec::new(),
                popup: None,
                composite_scroll: Vec::new(),
                composite_max_scroll: 0,
                last_diagcomm_tab: 0,
                last_section_tabs: HashMap::new(),
                last_selected_section_type: None,
                last_selected_section_title: None,
            },
            table: TableState {
                column_widths: Vec::new(),
                column_widths_absolute: Vec::new(),
                horizontal_scroll: Vec::new(),
                persisted_column_widths: HashMap::new(),
                focused_column: 0,
                sort_state: Vec::new(),
                cached_ratatui_constraints: Vec::new(),
                cached_total_table_width: 0,
                jump_buffer: String::new(),
                jump_buffer_time: None,
            },
            mouse: MouseState {
                drag_state: DragState::None,
                last_click_time: None,
                last_click_pos: (0, 0),
                enabled: true,
            },
            history: HistoryState {
                entries: Vec::new(),
                position: 0,
            },
            layout: LayoutCache {
                tree_area: Rect::default(),
                detail_area: Rect::default(),
                tab_area: None,
                tab_titles: Vec::new(),
                table_content_area: None,
                breadcrumb_area: Rect::default(),
                breadcrumb_segments: Vec::new(),
                tree_scrollbar_area: None,
                detail_scrollbar_area: None,
                detail_hscrollbar_area: None,
                tree_width_percentage: DEFAULT_TREE_WIDTH_PCT,
            },
            status: String::new(),
            focus_state: FocusState::Tree,
            theme,
        };
        // Apply initial sort order (default is by ID)
        app.sort_diagcomm_nodes_in_place();
        app.rebuild_visible();
        app
    }

    /// Helper: Check if a node is a service list section header
    fn is_service_list_section(node: &TreeNode) -> bool {
        node.service_list_type.is_some()
    }

    /// Helper: Check if a node is a specific service list type
    fn is_service_list_type(node: &TreeNode, list_type: crate::tree::ServiceListType) -> bool {
        matches!(&node.service_list_type, Some(t) if *t == list_type)
    }

    /// Check if a node is a DOP category node (child of DIAG-DATA-DICTIONARY-SPEC)
    fn is_dop_category_node(&self, node_idx: usize) -> bool {
        let Some(node) = self.tree.all_nodes.get(node_idx) else {
            return false;
        };
        if !node.has_children || node.depth == 0 {
            return false;
        }
        // Walk backwards to find the parent (first node with depth - 1)
        for i in (0..node_idx).rev() {
            let Some(candidate) = self.tree.all_nodes.get(i) else {
                continue;
            };
            if candidate.depth < node.depth {
                return matches!(candidate.node_type, NodeType::DOP);
            }
        }
        false
    }

    /// Check if a node is an individual DOP with children (e.g. a DTC-DOP under DTC-DOPS).
    /// These nodes should navigate to their children instead of showing a popup.
    fn is_individual_dop_node(&self, node_idx: usize) -> bool {
        let Some(node) = self.tree.all_nodes.get(node_idx) else {
            return false;
        };
        if !node.has_children || node.depth < 2 {
            return false;
        }
        // Walk backwards to find the parent
        for i in (0..node_idx).rev() {
            let Some(candidate) = self.tree.all_nodes.get(i) else {
                continue;
            };
            if candidate.depth < node.depth {
                return self.is_dop_category_node(i);
            }
        }
        false
    }

    /// Get the actual section index accounting for header section offset
    fn get_section_index(&self) -> usize {
        // Check if current node has a header section (rendered above tabs)
        if let Some(&idx) = self.tree.visible.get(self.tree.cursor) {
            let Some(node) = self.tree.all_nodes.get(idx) else {
                return self.detail.selected_tab;
            };
            let sections = &node.detail_sections;
            if sections.len() > 1
                && let Some(first_section) = sections.first()
                && first_section.render_as_header
                && matches!(
                    &first_section.content,
                    crate::tree::DetailContent::PlainText(_)
                )
            {
                // Has header section, so selected_tab needs offset of 1
                return self.detail.selected_tab.saturating_add(1);
            }
        }
        self.detail.selected_tab
    }

    /// Get the section offset for rendering (0 or 1 if there's a header section)
    fn get_section_offset(&self) -> usize {
        if let Some(&idx) = self.tree.visible.get(self.tree.cursor) {
            let Some(node) = self.tree.all_nodes.get(idx) else {
                return 0;
            };
            let sections = &node.detail_sections;
            if sections.len() > 1
                && let Some(first_section) = sections.first()
                && first_section.render_as_header
                && matches!(
                    &first_section.content,
                    crate::tree::DetailContent::PlainText(_)
                )
            {
                return 1;
            }
        }
        0
    }

    /// Get the actual table section index for storing/retrieving sort state
    fn get_table_section_idx(&self) -> usize {
        self.detail
            .selected_tab
            .saturating_add(self.get_section_offset())
    }

    /// Returns true if the currently selected detail section is a Composite
    fn is_current_section_composite(&self) -> bool {
        let Some(&node_idx) = self.tree.visible.get(self.tree.cursor) else {
            return false;
        };
        let Some(node) = self.tree.all_nodes.get(node_idx) else {
            return false;
        };
        let section_idx = self.get_section_index();
        node.detail_sections
            .get(section_idx)
            .is_some_and(|s| matches!(s.content, crate::tree::DetailContent::Composite(_)))
    }

    /// Update the selected tab and persist it generically for the current section type
    fn set_selected_tab(&mut self, new_tab: usize) {
        self.detail.selected_tab = new_tab;
        self.table.jump_buffer.clear();
        self.table.jump_buffer_time = None;

        // Save tab selection for the current section type
        if self.tree.cursor < self.tree.visible.len()
            && let Some(&node_idx) = self.tree.visible.get(self.tree.cursor)
            && let Some(node) = self.tree.all_nodes.get(node_idx)
        {
            // For backward compatibility, still save diagcomm tab
            if matches!(
                node.node_type,
                NodeType::Service | NodeType::ParentRefService | NodeType::Job
            ) {
                self.detail.last_diagcomm_tab = new_tab;
            }

            // Save tab for any node with detail sections that have a section type
            if !node.detail_sections.is_empty() {
                let section_offset = self.get_section_offset();
                let section_idx = new_tab.saturating_add(section_offset);
                if let Some(section) = node.detail_sections.get(section_idx) {
                    self.detail
                        .last_section_tabs
                        .insert(section.section_type, new_tab);
                    self.detail.last_selected_section_type = Some(section.section_type);
                    self.detail.last_selected_section_title = Some(section.title.clone());
                }
            }
        }
    }

    /// Jump to the first table row whose first cell starts with the `jump_buffer` text
    fn jump_to_matching_row(&mut self) {
        if self.table.jump_buffer.is_empty() || self.tree.cursor >= self.tree.visible.len() {
            return;
        }

        let Some(&node_idx) = self.tree.visible.get(self.tree.cursor) else {
            return;
        };
        let Some(node) = self.tree.all_nodes.get(node_idx) else {
            return;
        };
        let section_idx = self.get_section_index();

        let Some(section) = node.detail_sections.get(section_idx) else {
            return;
        };

        // Get table rows (apply sorting if active)
        let Some(rows) = section.content.table_rows() else {
            return;
        };
        let rows = self.apply_table_sort(rows, section_idx);

        let buffer_lower = self.table.jump_buffer.to_lowercase();

        // Find first row where the focused column starts with the buffer (case-insensitive)
        for (i, row) in rows.iter().enumerate() {
            if let Some(cell) = row.cells.get(self.table.focused_column)
                && cell.to_lowercase().starts_with(&buffer_lower)
            {
                if let Some(cursor) = self.detail.section_cursors.get_mut(section_idx) {
                    *cursor = i;
                }
                self.status = format!("Jump: \"{}\"", self.table.jump_buffer);
                return;
            }
        }

        self.status = format!("Jump: \"{}\" (no match)", self.table.jump_buffer);
    }

    /// Jump to the first visible tree node whose text starts with the `jump_buffer`
    fn jump_to_matching_tree_node(&mut self) {
        if self.table.jump_buffer.is_empty() {
            return;
        }

        let buffer_lower = self.table.jump_buffer.to_lowercase();

        // Search from current cursor position forward, then wrap around
        let len = self.tree.visible.len();
        let start = self.tree.cursor.saturating_add(1).min(len);

        let found = (start..len).chain(0..start).find(|&vis_idx| {
            self.tree
                .visible
                .get(vis_idx)
                .and_then(|&node_idx| self.tree.all_nodes.get(node_idx))
                .is_some_and(|node| node.text.to_lowercase().contains(&buffer_lower))
        });

        if let Some(target) = found {
            self.tree.cursor = target;
            self.reset_detail_state();
            self.tree.scroll_offset = self.tree.cursor.saturating_sub(SCROLL_CONTEXT_LINES);
            self.status = format!("Jump: \"{}\"", self.table.jump_buffer);
        } else {
            self.status = format!("Jump: \"{}\" (no match)", self.table.jump_buffer);
        }
    }

    /// Apply sorting to rows if a sort state exists for the given section
    fn apply_table_sort(&self, rows: &[DetailRow], section_idx: usize) -> Vec<DetailRow> {
        let Some(sort_state) = self
            .table
            .sort_state
            .get(section_idx)
            .and_then(|s| s.as_ref())
        else {
            return rows.to_vec();
        };

        let mut sorted = rows.to_vec();
        let col = sort_state.column;
        let dir = sort_state.direction;
        let secondary = sort_state.secondary_column;
        sorted.sort_by(|a, b| {
            let cmp = Self::compare_cells(a, b, col);
            let cmp = match cmp {
                std::cmp::Ordering::Equal => secondary.map_or(std::cmp::Ordering::Equal, |sec| {
                    Self::compare_cells(a, b, sec)
                }),
                other => other,
            };

            match dir {
                SortDirection::Ascending => cmp,
                SortDirection::Descending => cmp.reverse(),
            }
        });
        sorted
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
                    if self.search.active {
                        self.handle_search_key(key.code)
                    } else {
                        self.handle_normal_key(key.code, ctrl)
                    }
                }
                Event::Mouse(mouse) => {
                    if self.mouse.enabled {
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
            .constraints([
                Constraint::Min(3),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .areas(frame.area());

        let [tree_area, detail_area] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(self.layout.tree_width_percentage),
                Constraint::Percentage(100u16.saturating_sub(self.layout.tree_width_percentage)),
            ])
            .areas(main);

        // Cache areas for mouse handling
        self.layout.tree_area = tree_area;
        self.layout.detail_area = detail_area;
        self.layout.breadcrumb_area = breadcrumb_bar;

        self.draw_tree(frame, tree_area);
        self.draw_detail(frame, detail_area);
        self.draw_breadcrumb(frame, breadcrumb_bar);
        self.draw_status(frame, status_bar);

        // Draw popups if open (order matters - last drawn is on top)
        if self.detail.popup.is_some() {
            self.draw_detail_popup(frame);
        }
        if self.focus_state == FocusState::HelpPopup {
            self.draw_help_popup(frame);
        }
    }
}
