use cda_database::datatypes::DiagLayer;

use crate::tree::{builder::TreeBuilder, types::NodeType};

/// Add functional classes section from the diagnostic layer
pub fn add_functional_classes(b: &mut TreeBuilder, layer: &DiagLayer<'_>, depth: usize) {
    // Check if layer has functional classes
    // Functional classes represent groupings of diagnostic functionality
    if let Some(services) = layer.diag_services() {
        if !services.is_empty() {
            // For now, add a placeholder node
            // TODO: Expand this to show actual functional class hierarchy
            b.push_leaf(depth, "Functional Classes".to_string(), NodeType::Default);
        }
    }
}
