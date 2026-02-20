use cda_database::datatypes::{DiagLayer, ParentRef};

use crate::tree::builder::TreeBuilder;

use super::variants::services::add_diag_comms;
use super::variants::requests::add_requests_section;
use super::variants::responses::{add_pos_responses_section, add_neg_responses_section};
use super::variants::state_charts::add_state_charts;
use super::variants::com_params::add_com_params;
use super::variants::placeholders::{
    add_functional_classes, add_diag_data_dictionary_spec,
    add_additional_audiences, add_sub_components, add_sdgs, add_parent_refs,
};

/// Extension trait for adding DiagLayer structures to the tree
pub trait LayerExt {
    /// Add a complete diag layer with structured hierarchy for containers
    /// variant_parent_refs: Optional iterator to parent refs from the variant for fetching inherited services
    fn add_diag_layer_structured<'a>(
        &mut self,
        layer: &DiagLayer<'a>,
        depth: usize,
        layer_name: &str,
        _expand: bool,
        variant_parent_refs: Option<impl Iterator<Item = ParentRef<'a>> + 'a>,
    );
}

impl LayerExt for TreeBuilder {
    fn add_diag_layer_structured<'a>(
        &mut self,
        layer: &DiagLayer<'a>,
        depth: usize,
        layer_name: &str,
        _expand: bool,
        variant_parent_refs: Option<impl Iterator<Item = ParentRef<'a>> + 'a>,
    ) {
        // Functional Classes
        add_functional_classes(self, layer, depth, layer_name);
        
        // Diag-Data-Dictionary-Spec
        add_diag_data_dictionary_spec(self, layer, depth, layer_name);
        
        // Diag-Comms
        add_diag_comms(self, layer, depth, layer_name, variant_parent_refs);
        
        // Requests (from diag-comms)
        add_requests_section(self, layer, depth, layer_name);
        
        // Pos-Responses (from diag-comms)
        add_pos_responses_section(self, layer, depth, layer_name);
        
        // Neg-Responses (from diag-comms)
        add_neg_responses_section(self, layer, depth, layer_name);
        
        // State-Charts
        add_state_charts(self, layer, depth, layer_name);
        
        // Additional Audiences
        add_additional_audiences(self, layer, depth, layer_name);
        
        // Sub-Components
        add_sub_components(self, layer, depth, layer_name);
        
        // SDGs
        add_sdgs(self, layer, depth, layer_name);
        
        // ComParam Refs
        add_com_params(self, layer, depth, layer_name);
        
        // Parent Refs
        add_parent_refs(self, layer, depth, layer_name);
    }
}
