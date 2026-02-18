use crossterm::event::KeyCode;

use super::App;

/// Result of processing a key press.
pub enum Action {
    Continue,
    Quit,
}

impl App {
    /// Handle a key press while the search prompt is active.
    pub(super) fn handle_search_key(&mut self, code: KeyCode) -> Action {
        match code {
            KeyCode::Esc => {
                self.searching = false;
                self.search.clear();
                self.search_matches.clear();
                self.status.clear();
            }
            KeyCode::Enter => {
                self.searching = false;
                self.update_search();
            }
            KeyCode::Backspace => {
                self.search.pop();
                self.update_search();
            }
            KeyCode::Char(c) => {
                self.search.push(c);
                self.update_search();
            }
            _ => {}
        }
        Action::Continue
    }

    /// Handle a key press in normal (non-search) mode.
    pub(super) fn handle_normal_key(&mut self, code: KeyCode, ctrl: bool) -> Action {
        // Check if popup is open
        if self.dop_popup.is_some() {
            // Popup is open - only Escape closes it
            if matches!(code, KeyCode::Esc) {
                self.dop_popup = None;
            }
            return Action::Continue;
        }
        
        match code {
            KeyCode::Char('q') | KeyCode::Esc => return Action::Quit,
            KeyCode::Char('c') if ctrl => return Action::Quit,

            KeyCode::Tab => {
                self.detail_focused = !self.detail_focused;
                if !self.detail_focused {
                    self.focused_section = 0; // Reset when returning to tree
                }
            }

            // Pane resizing
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.tree_width_percentage = (self.tree_width_percentage + 5).min(80);
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                self.tree_width_percentage = (self.tree_width_percentage.saturating_sub(5)).max(20);
            }

            KeyCode::Up | KeyCode::Char('k') => {
                if self.detail_focused {
                    // Move cursor up in the focused section
                    if self.focused_section < self.section_cursors.len() {
                        self.section_cursors[self.focused_section] = 
                            self.section_cursors[self.focused_section].saturating_sub(1);
                    }
                } else {
                    self.move_up();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.detail_focused {
                    // Move cursor down in the focused section (will be clamped during render)
                    if self.focused_section < self.section_cursors.len() {
                        self.section_cursors[self.focused_section] = 
                            self.section_cursors[self.focused_section].saturating_add(1);
                    }
                } else {
                    self.move_down();
                }
            }
            KeyCode::PageUp => {
                if self.detail_focused {
                    // Move cursor up by page in focused section
                    if self.focused_section < self.section_cursors.len() {
                        self.section_cursors[self.focused_section] = 
                            self.section_cursors[self.focused_section].saturating_sub(20);
                    }
                } else {
                    self.page_up(20);
                }
            }
            KeyCode::PageDown => {
                if self.detail_focused {
                    // Move cursor down by page in focused section
                    if self.focused_section < self.section_cursors.len() {
                        self.section_cursors[self.focused_section] = 
                            self.section_cursors[self.focused_section].saturating_add(20);
                    }
                } else {
                    self.page_down(20);
                }
            }
            KeyCode::Home => {
                if self.detail_focused {
                    // Move cursor to top of focused section
                    if self.focused_section < self.section_cursors.len() {
                        self.section_cursors[self.focused_section] = 0;
                    }
                } else {
                    self.home();
                }
            }
            KeyCode::End => {
                if self.detail_focused {
                    // Move cursor to bottom of focused section (will be clamped during render)
                    if self.focused_section < self.section_cursors.len() {
                        self.section_cursors[self.focused_section] = usize::MAX;
                    }
                } else {
                    self.end();
                }
            }

            KeyCode::Left | KeyCode::Char('h') => {
                if self.detail_focused {
                    // Navigate to previous detail pane section
                    let old_section = self.focused_section;
                    self.focused_section = self.focused_section.saturating_sub(1);
                    // Reset cursor and scroll when changing sections
                    if old_section != self.focused_section {
                        if self.focused_section < self.section_scrolls.len() {
                            self.section_scrolls[self.focused_section] = 0;
                        }
                        if self.focused_section < self.section_cursors.len() {
                            self.section_cursors[self.focused_section] = 0;
                        }
                    }
                } else {
                    self.try_collapse_or_parent();
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if self.detail_focused {
                    // Navigate to next detail pane section (will be clamped during render)
                    let old_section = self.focused_section;
                    self.focused_section = self.focused_section.saturating_add(1);
                    // Reset cursor and scroll when changing sections
                    if old_section != self.focused_section {
                        if self.focused_section < self.section_scrolls.len() {
                            self.section_scrolls[self.focused_section] = 0;
                        }
                        if self.focused_section < self.section_cursors.len() {
                            self.section_cursors[self.focused_section] = 0;
                        }
                    }
                } else {
                    self.try_expand();
                }
            }
            KeyCode::Enter => {
                if self.detail_focused {
                    // Check if current row has DOP and show popup
                    self.try_show_dop_popup();
                } else {
                    self.try_expand();
                }
            }
            KeyCode::Char(' ') if !self.detail_focused => {
                self.toggle_expand();
            }

            KeyCode::Char('e') => self.expand_all(),
            KeyCode::Char('c') => self.collapse_all(),

            KeyCode::Char('/') => {
                self.searching = true;
                self.search.clear();
                self.status = "Search: ".into();
            }
            KeyCode::Char('n') => self.next_search_match(),
            KeyCode::Char('N') => self.prev_search_match(),

            _ => {}
        }
        Action::Continue
    }
}
