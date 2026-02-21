use cda_database::datatypes::{DiagLayer, ParentRef};

use crate::tree::{
    builder::TreeBuilder,
    types::{
        CellType, ColumnConstraint, DetailContent, DetailRow, DetailSectionData, DetailSectionType,
        NodeType,
    },
};

/// Add a clickable Parent Refs node with detailed information about not-inherited elements
pub fn add_parent_refs_with_details<'a>(
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

    // Build main table section showing list of parent refs
    let main_section = build_parent_refs_list_section(&parent_refs_list);

    // Build not-inherited element sections as tabs
    let mut detail_sections = vec![main_section];

    // Add tabs for each not-inherited type
    detail_sections.push(build_not_inherited_diag_comms_section(&parent_refs_list));
    detail_sections.push(build_not_inherited_diag_variables_section(&parent_refs_list));
    detail_sections.push(build_not_inherited_dops_section(&parent_refs_list));
    detail_sections.push(build_not_inherited_tables_section(&parent_refs_list));

    b.push_details_structured(
        depth,
        "Parent Refs".to_string(),
        false,
        true,
        detail_sections,
        NodeType::ParentRefs,
    );
}

/// Build the main list of parent references
fn build_parent_refs_list_section(parent_refs_list: &[ParentRef<'_>]) -> DetailSectionData {
    let header = DetailRow::header(
        vec!["Short Name".to_owned(), "Type".to_owned()],
        vec![CellType::Text, CellType::Text],
    );

    let mut rows = Vec::new();

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

    DetailSectionData::new(
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
    .with_type(DetailSectionType::RelatedRefs)
}

/// Build a section showing not-inherited DiagComms
fn build_not_inherited_diag_comms_section(parent_refs_list: &[ParentRef<'_>]) -> DetailSectionData {
    let header = DetailRow::header(
        vec!["Short Name".to_owned(), "Parent".to_owned()],
        vec![CellType::Text, CellType::Text],
    );

    let mut rows = Vec::new();

    for parent_ref in parent_refs_list {
        // Get the parent layer name for display
        let parent_name = get_parent_ref_name(parent_ref);

        // Get not-inherited diag comm short names
        if let Some(not_inherited_names) = parent_ref.not_inherited_diag_comm_short_names() {
            for name in not_inherited_names.iter() {
                rows.push(DetailRow::normal(
                    vec![name.to_owned(), parent_name.clone()],
                    vec![CellType::Text, CellType::Text],
                    0,
                ));
            }
        }
    }

    DetailSectionData::new(
        "Not Inherited DiagComms".to_owned(),
        DetailContent::Table {
            header,
            rows,
            constraints: vec![
                ColumnConstraint::Percentage(60),
                ColumnConstraint::Percentage(40),
            ],
            use_row_selection: true,
        },
        false,
    )
    .with_type(DetailSectionType::Custom)
}

/// Build a section showing not-inherited DiagVariables
fn build_not_inherited_diag_variables_section(
    parent_refs_list: &[ParentRef<'_>],
) -> DetailSectionData {
    let header = DetailRow::header(
        vec!["Short Name".to_owned(), "Parent".to_owned()],
        vec![CellType::Text, CellType::Text],
    );

    let mut rows = Vec::new();

    for parent_ref in parent_refs_list {
        let parent_name = get_parent_ref_name(parent_ref);

        // Get not-inherited variables short names
        if let Some(not_inherited_names) = parent_ref.not_inherited_variables_short_names() {
            for name in not_inherited_names.iter() {
                rows.push(DetailRow::normal(
                    vec![name.to_owned(), parent_name.clone()],
                    vec![CellType::Text, CellType::Text],
                    0,
                ));
            }
        }
    }

    DetailSectionData::new(
        "Not Inherited DiagVariables".to_owned(),
        DetailContent::Table {
            header,
            rows,
            constraints: vec![
                ColumnConstraint::Percentage(60),
                ColumnConstraint::Percentage(40),
            ],
            use_row_selection: true,
        },
        false,
    )
    .with_type(DetailSectionType::Custom)
}

/// Build a section showing not-inherited DOPs
fn build_not_inherited_dops_section(parent_refs_list: &[ParentRef<'_>]) -> DetailSectionData {
    let header = DetailRow::header(
        vec!["Short Name".to_owned(), "Parent".to_owned()],
        vec![CellType::Text, CellType::Text],
    );

    let mut rows = Vec::new();

    for parent_ref in parent_refs_list {
        let parent_name = get_parent_ref_name(parent_ref);

        // Get not-inherited DOPs short names
        if let Some(not_inherited_names) = parent_ref.not_inherited_dops_short_names() {
            for name in not_inherited_names.iter() {
                rows.push(DetailRow::normal(
                    vec![name.to_owned(), parent_name.clone()],
                    vec![CellType::Text, CellType::Text],
                    0,
                ));
            }
        }
    }

    DetailSectionData::new(
        "Not Inherited Dops".to_owned(),
        DetailContent::Table {
            header,
            rows,
            constraints: vec![
                ColumnConstraint::Percentage(60),
                ColumnConstraint::Percentage(40),
            ],
            use_row_selection: true,
        },
        false,
    )
    .with_type(DetailSectionType::Custom)
}

/// Build a section showing not-inherited Tables
fn build_not_inherited_tables_section(parent_refs_list: &[ParentRef<'_>]) -> DetailSectionData {
    let header = DetailRow::header(
        vec!["Short Name".to_owned(), "Parent".to_owned()],
        vec![CellType::Text, CellType::Text],
    );

    let mut rows = Vec::new();

    for parent_ref in parent_refs_list {
        let parent_name = get_parent_ref_name(parent_ref);

        // Get not-inherited tables short names
        if let Some(not_inherited_names) = parent_ref.not_inherited_tables_short_names() {
            for name in not_inherited_names.iter() {
                rows.push(DetailRow::normal(
                    vec![name.to_owned(), parent_name.clone()],
                    vec![CellType::Text, CellType::Text],
                    0,
                ));
            }
        }
    }

    DetailSectionData::new(
        "Not Inherited Tables".to_owned(),
        DetailContent::Table {
            header,
            rows,
            constraints: vec![
                ColumnConstraint::Percentage(60),
                ColumnConstraint::Percentage(40),
            ],
            use_row_selection: true,
        },
        false,
    )
    .with_type(DetailSectionType::Custom)
}

/// Helper to extract the short name from a parent ref
fn get_parent_ref_name(parent_ref: &ParentRef<'_>) -> String {
    match parent_ref.ref_type().try_into() {
        Ok(cda_database::datatypes::ParentRefType::Variant) => parent_ref
            .ref__as_variant()
            .and_then(|v| v.diag_layer())
            .and_then(|dl| dl.short_name())
            .unwrap_or("?")
            .to_owned(),
        Ok(cda_database::datatypes::ParentRefType::EcuSharedData) => parent_ref
            .ref__as_ecu_shared_data()
            .and_then(|esd| esd.diag_layer())
            .and_then(|dl| dl.short_name())
            .unwrap_or("?")
            .to_owned(),
        Ok(cda_database::datatypes::ParentRefType::Protocol) => parent_ref
            .ref__as_protocol()
            .and_then(|p| p.diag_layer())
            .and_then(|dl| dl.short_name())
            .unwrap_or("?")
            .to_owned(),
        Ok(cda_database::datatypes::ParentRefType::FunctionalGroup) => parent_ref
            .ref__as_functional_group()
            .and_then(|fg| fg.diag_layer())
            .and_then(|dl| dl.short_name())
            .unwrap_or("?")
            .to_owned(),
        _ => "Unknown".to_owned(),
    }
}
