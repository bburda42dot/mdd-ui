use super::types::{TreeNode, NodeType, DetailSectionData};

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

    /// Push a collapsible/expandable node.
    pub(crate) fn push(
        &mut self,
        depth: usize,
        text: String,
        expanded: bool,
        has_children: bool,
        node_type: NodeType,
    ) {
        self.nodes.push(TreeNode {
            depth,
            text,
            expanded,
            has_children,
            detail_sections: Vec::new(),
            node_type,
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
        self.nodes.push(TreeNode {
            depth,
            text,
            expanded,
            has_children,
            detail_sections: sections,
            node_type,
        });
    }

    /// Convenience: push a leaf node (no children, no details).
    pub(crate) fn push_leaf(&mut self, depth: usize, text: String, node_type: NodeType) {
        self.push(depth, text, false, false, node_type);
    }

    pub(crate) fn finish(self) -> Vec<TreeNode> {
        self.nodes
    }
}
