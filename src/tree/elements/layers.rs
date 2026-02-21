use cda_database::datatypes::{DiagLayer, ParentRef};

use super::variants::{
    com_params::add_com_params,
    functional_classes::add_functional_classes,
    placeholders::{
        add_additional_audiences, add_diag_data_dictionary_spec,
        add_parent_refs, add_sdgs, add_sub_components,
    },
    requests::add_requests_section,
    responses::{add_neg_responses_section, add_pos_responses_section},
    services::add_diag_comms,
    state_charts::add_state_charts,
};
use crate::tree::builder::TreeBuilder;

/// Extension trait for adding DiagLayer structures to the tree
pub trait LayerExt {
    /// Add a complete diag layer with structured hierarchy for containers
    /// variant_parent_refs: Optional iterator to parent refs from the variant
    /// for fetching inherited services
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
        // Collect parent refs into a vector so we can reuse them for multiple sections
        let parent_refs_vec: Option<Vec<ParentRef<'a>>> =
            variant_parent_refs.map(|iter| iter.collect());

        // Functional Classes
        add_functional_classes(self, layer, depth);

        // Diag-Data-Dictionary-Spec
        add_diag_data_dictionary_spec(self, layer, depth);

        // Diag-Comms
        add_diag_comms(
            self,
            layer,
            depth,
            layer_name,
            parent_refs_vec.as_ref().map(|v| v.iter().cloned()),
        );

        // Requests (from diag-comms) - use EXACTLY the same logic as DiagComm
        add_requests_section(
            self,
            layer,
            depth,
            parent_refs_vec.as_ref().map(|v| v.iter().cloned()),
        );

        // Pos-Responses (from diag-comms) - use EXACTLY the same logic as DiagComm
        add_pos_responses_section(
            self,
            layer,
            depth,
            parent_refs_vec.as_ref().map(|v| v.iter().cloned()),
        );

        // Neg-Responses (from diag-comms) - use EXACTLY the same logic as DiagComm
        add_neg_responses_section(
            self,
            layer,
            depth,
            parent_refs_vec.as_ref().map(|v| v.iter().cloned()),
        );

        // State-Charts
        add_state_charts(self, layer, depth);

        // Additional Audiences
        add_additional_audiences(self, layer, depth);

        // Sub-Components
        add_sub_components(self, layer, depth);

        // SDGs
        add_sdgs(self, layer, depth);

        // ComParam Refs
        add_com_params(self, layer, depth);

        // Parent Refs
        add_parent_refs(self, layer, depth);
    }
}
