use cda_database::datatypes::{DiagLayer, ParentRef};

use crate::tree::{
    builder::TreeBuilder,
    types::{
        CellType, ColumnConstraint, DetailContent, DetailRow, DetailSectionData, DetailSectionType,
        NodeType,
    },
};

/// Add placeholder sections that are not fully implemented yet
/// These are kept for structure but may be expanded in the future
pub fn add_diag_data_dictionary_spec(b: &mut TreeBuilder, layer: &DiagLayer<'_>, depth: usize) {
    // Check if layer has diagnostic data dictionary specifications
    // This would typically be accessed through specific methods if available
    // For now, we'll add a placeholder if the layer has data operations or similar
    let has_spec = layer.diag_services().is_some_and(|s| !s.is_empty());

    if has_spec {
        b.push_leaf(
            depth,
            "Diag-Data-Dictionary-Spec".to_string(),
            NodeType::Default,
        );
    }
}

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

pub fn add_sub_components(b: &mut TreeBuilder, layer: &DiagLayer<'_>, depth: usize) {
    // Sub-components would be nested diagnostic layers or related structures
    // Placeholder for now
    if let Some(_jobs) = layer.single_ecu_jobs() {
        b.push_leaf(depth, "Sub-Components".to_string(), NodeType::Default);
    }
}

pub fn add_sdgs(b: &mut TreeBuilder, _layer: &DiagLayer<'_>, depth: usize) {
    // SDGs (Special Data Groups) from the layer
    // These would be accessed through specific methods if available
    // Placeholder for structure
    b.push_leaf(depth, "SDGs".to_string(), NodeType::Default);
}

pub fn add_parent_refs<'a>(
    b: &mut TreeBuilder,
    _layer: &DiagLayer<'_>,
    depth: usize,
    parent_refs: Option<impl Iterator<Item = ParentRef<'a>>>,
) {
    let Some(parent_refs_iter) = parent_refs else {
        return;
    };

    let parent_refs_list: Vec<_> = parent_refs_iter.collect();

    if parent_refs_list.is_empty() {
        return;
    }

    // Build table section for parent refs
    let header = DetailRow::header(
        vec!["Short Name".to_owned(), "Type".to_owned()],
        vec![CellType::Text, CellType::Text],
    );

    let mut rows = Vec::new();

    // Add each parent ref to the table
    for parent_ref in parent_refs_list {
        let (ref_type_str, short_name) = match parent_ref.ref_type().try_into() {
            Ok(cda_database::datatypes::ParentRefType::Variant) => {
                let short_name = parent_ref
                    .ref__as_variant()
                    .and_then(|v| v.diag_layer())
                    .and_then(|dl| dl.short_name())
                    .unwrap_or("?")
                    .to_owned();
                ("Variant", short_name)
            }
            Ok(cda_database::datatypes::ParentRefType::EcuSharedData) => {
                let short_name = parent_ref
                    .ref__as_ecu_shared_data()
                    .and_then(|esd| esd.diag_layer())
                    .and_then(|dl| dl.short_name())
                    .unwrap_or("?")
                    .to_owned();
                ("ECU Shared Data", short_name)
            }
            Ok(cda_database::datatypes::ParentRefType::Protocol) => {
                let short_name = parent_ref
                    .ref__as_protocol()
                    .and_then(|p| p.diag_layer())
                    .and_then(|dl| dl.short_name())
                    .unwrap_or("?")
                    .to_owned();
                ("Protocol", short_name)
            }
            Ok(cda_database::datatypes::ParentRefType::FunctionalGroup) => {
                let short_name = parent_ref
                    .ref__as_functional_group()
                    .and_then(|fg| fg.diag_layer())
                    .and_then(|dl| dl.short_name())
                    .unwrap_or("?")
                    .to_owned();
                ("Functional Group", short_name)
            }
            _ => ("Unknown", "?".to_owned()),
        };

        rows.push(DetailRow::normal(
            vec![short_name, ref_type_str.to_owned()],
            vec![CellType::Text, CellType::Text],
            0,
        ));
    }

    let detail_section = DetailSectionData::new(
        "Parent References".to_owned(),
        DetailContent::Table {
            header,
            rows,
            constraints: vec![
                ColumnConstraint::Percentage(70),
                ColumnConstraint::Percentage(30),
            ],
            use_row_selection: true,
        },
        false,
    )
    .with_type(DetailSectionType::RelatedRefs);

    b.push_details_structured(
        depth,
        "Parent Refs".to_string(),
        false,
        false,
        vec![detail_section],
        NodeType::Default,
    );
}
