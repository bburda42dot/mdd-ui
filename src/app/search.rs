/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2026 Alexander Mohr
 */

use super::{App, SearchScope};

impl App {
    pub(crate) fn update_search(&mut self) {
        if self.search.query.is_empty() {
            // If search is empty, don't add to stack
            self.status.clear();
        } else {
            // Add current search with its scope to stack
            self.search
                .stack
                .push((self.search.query.clone(), self.search.scope.clone()));
            self.search.query.clear(); // Clear for next search

            let depth = self.search.stack.len();
            let stack_display: Vec<String> = self
                .search
                .stack
                .iter()
                .map(|(term, scope)| format!("{}{}", term, scope.abbrev()))
                .collect();
            self.status = format!("Search depth: {} [{}]", depth, stack_display.join(" → "));
        }

        // Rebuild visible list with the search stack
        self.rebuild_visible();
        self.tree.cursor = 0;
        self.reset_detail_state();
        self.populate_search_matches();
        self.search.match_cursor = 0;
    }

    /// Populate `search_matches` with visible indices of nodes that directly match
    /// the most recent search query, enabling n/N navigation between matches.
    fn populate_search_matches(&mut self) {
        self.search.matches.clear();

        let Some((query, scope)) = self.search.stack.last().cloned() else {
            return;
        };
        let q = query.to_lowercase();

        self.search.matches = self
            .tree
            .visible
            .iter()
            .enumerate()
            .filter(|&(_, &node_idx)| {
                self.tree.all_nodes.get(node_idx).is_some_and(|node| {
                    self.node_matches_scope_and_query(node, node_idx, &scope, &q)
                })
            })
            .map(|(visible_idx, _)| visible_idx)
            .collect();
    }

    pub(crate) fn clear_search_stack(&mut self) {
        self.search.stack.clear();
        self.search.query.clear();
        self.status = "Search cleared".into();
        self.rebuild_visible();
        self.tree.cursor = 0;
        self.reset_detail_state();
    }

    pub(crate) fn cycle_search_scope(&mut self) {
        self.search.scope = match self.search.scope {
            SearchScope::All => SearchScope::Variants,
            SearchScope::Variants => SearchScope::FunctionalGroups,
            SearchScope::FunctionalGroups => SearchScope::EcuSharedData,
            SearchScope::EcuSharedData => SearchScope::Services,
            SearchScope::Services => SearchScope::DiagComms,
            SearchScope::DiagComms => SearchScope::Requests,
            SearchScope::Requests => SearchScope::Responses,
            // Subtree is contextual, cycling from it resets to All
            SearchScope::Responses | SearchScope::Subtree { .. } => SearchScope::All,
        };

        self.status = format!("Search scope: {}", self.search.scope);
    }

    /// Set search scope to the subtree rooted at the current cursor position
    pub(crate) fn set_subtree_scope(&mut self) {
        let Some(&start_idx) = self.tree.visible.get(self.tree.cursor) else {
            return;
        };
        let Some(root_node) = self.tree.all_nodes.get(start_idx) else {
            return;
        };
        let root_name = root_node.text.clone();

        self.search.scope = SearchScope::Subtree {
            root_idx: start_idx,
            root_name: root_name.clone(),
        };
        self.status = format!("Search scope: subtree '{root_name}'");
    }

    pub(crate) fn next_search_match(&mut self) {
        if self.search.matches.is_empty() {
            return;
        }
        self.search.match_cursor = self
            .search
            .match_cursor
            .saturating_add(1)
            .rem_euclid(self.search.matches.len());
        if let Some(&match_idx) = self.search.matches.get(self.search.match_cursor) {
            self.tree.cursor = match_idx;
            self.reset_detail_state();
            self.status = format!(
                "Match {}/{}",
                self.search.match_cursor.saturating_add(1),
                self.search.matches.len()
            );
        }
    }

    pub(crate) fn prev_search_match(&mut self) {
        if self.search.matches.is_empty() {
            return;
        }
        self.search.match_cursor = if self.search.match_cursor == 0 {
            self.search.matches.len().saturating_sub(1)
        } else {
            self.search.match_cursor.saturating_sub(1)
        };
        if let Some(&match_idx) = self.search.matches.get(self.search.match_cursor) {
            self.tree.cursor = match_idx;
            self.reset_detail_state();
            self.status = format!(
                "Match {}/{}",
                self.search.match_cursor.saturating_add(1),
                self.search.matches.len()
            );
        }
    }
}
