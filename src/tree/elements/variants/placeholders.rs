use cda_database::datatypes::DiagLayer;

use crate::tree::{builder::TreeBuilder, types::NodeType};

/// Add placeholder sections that are not fully implemented yet
/// These are kept for structure but may be expanded in the future
// Diag-Data-Dictionary-Spec is not supported and has been removed
// pub fn add_diag_data_dictionary_spec(b: &mut TreeBuilder, layer: &DiagLayer<'_>, depth: usize) { ... }

pub fn add_additional_audiences(b: &mut TreeBuilder, layer: &DiagLayer<'_>, depth: usize) {
    // Additional audiences would be part of admin data or metadata
    // This is typically not directly available in the DiagLayer API
    // Adding placeholder for structure
    if let Some(services) = layer.diag_services() {
        let has_audiences = services.iter().any(|s| {
            cda_database::datatypes::DiagService(s)
                .diag_comm()
                .and_then(|dc| dc.audience())
                .is_some()
        });

        if has_audiences {
            b.push_leaf(depth, "Additional Audiences".to_string(), NodeType::Default);
        }
    }
}

// Sub-Components is not supported and has been removed
// pub fn add_sub_components(b: &mut TreeBuilder, layer: &DiagLayer<'_>, depth: usize) { ... }

pub fn add_sdgs(b: &mut TreeBuilder, _layer: &DiagLayer<'_>, depth: usize) {
    // SDGs (Special Data Groups) from the layer
    // These would be accessed through specific methods if available
    // Placeholder for structure
    b.push_leaf(depth, "SDGs".to_string(), NodeType::Default);
}
