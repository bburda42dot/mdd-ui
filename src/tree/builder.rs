/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2026 Alexander Mohr
 */

use std::rc::Rc;

use super::types::{DetailSectionData, NodeType, SectionType, ServiceListType, TreeNode};

/// Configuration for a single tree node, used to avoid repeating the full
/// `TreeNode` struct literal in every `push_*` method.
#[derive(Default)]
struct NodeConfig {
    depth: usize,
    text: String,
    expanded: bool,
    has_children: bool,
    sections: Vec<DetailSectionData>,
    node_type: NodeType,
    section_type: Option<SectionType>,
    service_list_type: Option<ServiceListType>,
    param_id: Option<u32>,
}

/// Accumulates `TreeNode`s while walking the database model.
///
/// Methods are spread across submodules (`services`, `layers`) via
/// `impl TreeBuilder` blocks so each concern lives in its own file.
pub struct TreeBuilder {
    nodes: Vec<TreeNode>,
}

impl TreeBuilder {
    pub(crate) fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    fn push_node(&mut self, cfg: NodeConfig) {
        self.nodes.push(TreeNode {
            depth: cfg.depth,
            text: cfg.text,
            expanded: cfg.expanded,
            has_children: cfg.has_children,
            detail_sections: Rc::from(cfg.sections),
            node_type: cfg.node_type,
            section_type: cfg.section_type,
            service_list_type: cfg.service_list_type,
            param_id: cfg.param_id,
        });
    }

    /// Push a node that carries structured detail sections (preferred).
    pub(crate) fn push_details_structured(
        &mut self,
        depth: usize,
        text: String,
        expanded: bool,
        has_children: bool,
        sections: Vec<DetailSectionData>,
        node_type: NodeType,
    ) {
        self.push_node(NodeConfig {
            depth,
            text,
            expanded,
            has_children,
            sections,
            node_type,
            ..NodeConfig::default()
        });
    }

    /// Push a parameter node with its ID for lookup
    pub(crate) fn push_param(
        &mut self,
        depth: usize,
        text: String,
        sections: Vec<DetailSectionData>,
        node_type: NodeType,
        param_id: u32,
    ) {
        self.push_node(NodeConfig {
            depth,
            text,
            sections,
            node_type,
            param_id: Some(param_id),
            ..NodeConfig::default()
        });
    }

    /// Push a service list section header with type information
    pub(crate) fn push_service_list_header(
        &mut self,
        depth: usize,
        text: String,
        expanded: bool,
        has_children: bool,
        sections: Vec<DetailSectionData>,
        service_list_type: ServiceListType,
    ) {
        self.push_node(NodeConfig {
            depth,
            text,
            expanded,
            has_children,
            sections,
            node_type: NodeType::SectionHeader,
            service_list_type: Some(service_list_type),
            ..NodeConfig::default()
        });
    }

    /// Push a top-level section header with type information
    pub(crate) fn push_section_header(
        &mut self,
        text: String,
        expanded: bool,
        has_children: bool,
        sections: Vec<DetailSectionData>,
        section_type: SectionType,
    ) {
        self.push_node(NodeConfig {
            text,
            expanded,
            has_children,
            sections,
            node_type: NodeType::SectionHeader,
            section_type: Some(section_type),
            ..NodeConfig::default()
        });
    }

    pub(crate) fn finish(self) -> Vec<TreeNode> {
        self.nodes
    }
}
