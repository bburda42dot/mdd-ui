// Submodules that represent the tree hierarchy under variants
pub mod com_params;
pub mod functional_classes;
pub mod parent_refs;
pub mod placeholders;
pub mod requests;
pub mod responses;
pub mod services;
pub mod state_charts;

use cda_database::datatypes::{DiagLayer, EcuDb, Variant as VariantWrap};

use super::layers::LayerExt;
use crate::tree::{
    builder::TreeBuilder,
    types::{
        CellType, ColumnConstraint, DetailContent, DetailRow, DetailSectionData, DetailSectionType,
        NodeType, lines_to_single_section,
    },
};

/// Add all variants to the tree
pub fn add_variants(b: &mut TreeBuilder, ecu: &EcuDb<'_>) {
    // Add Variants section
    if let Some(variants) = ecu.variants() {
        // Collect all variants for cross-variant lookups (e.g., functional classes)
        let all_variants_vec: Vec<VariantWrap> = variants
            .iter()
            .map(|v| VariantWrap(v))
            .collect();
        
        b.push(
            0,
            "Variants".to_string(),
            true,
            true,
            NodeType::SectionHeader,
        );

        // Sort variants alphabetically by name
        let mut sorted_variants: Vec<_> = variants.iter().enumerate().collect();
        sorted_variants.sort_by_cached_key(|(_, v)| {
            let vw = VariantWrap(*v);
            vw.diag_layer()
                .and_then(|l| l.short_name())
                .unwrap_or("")
                .to_lowercase()
        });

        // Add each variant
        for (vi, variant) in sorted_variants.into_iter() {
            let vw = VariantWrap(variant);
            let mut name = vw
                .diag_layer()
                .and_then(|l| l.short_name().map(str::to_owned))
                .unwrap_or_else(|| format!("variant_{vi}"));
            let is_base = vw.is_base_variant();

            // Add [base] suffix for base variants
            if is_base {
                name.push_str(" [base]");
            }

            // Build detail sections
            let mut detail_sections = vec![];

            // Add summary section
            let summary_lines = get_variant_summary(&vw, &name);
            detail_sections.push(lines_to_single_section("Summary", summary_lines));

            // Add parent refs table if available
            if let Some(parent_refs_section) = build_parent_refs_section(&vw) {
                detail_sections.push(parent_refs_section);
            }

            b.push_details_structured(
                1,
                name.clone(),
                is_base,
                true,
                detail_sections,
                NodeType::Container,
            );

            // Add diag layer content directly under variant (no section header)
            if let Some(dl) = vw.diag_layer() {
                let layer = DiagLayer(dl);
                // Pass parent refs from variant for inherited service lookup
                let parent_refs_iter = vw
                    .parent_refs()
                    .map(|pr| pr.iter().map(cda_database::datatypes::ParentRef));
                // Pass all variants for cross-variant lookups
                let all_variants_iter = all_variants_vec.iter().cloned();
                b.add_diag_layer_structured(&layer, 2, &name, is_base, parent_refs_iter, Some(all_variants_iter));
            }
        }
    }
}

/// Add all functional groups to the tree
pub fn add_functional_groups(b: &mut TreeBuilder, ecu: &EcuDb<'_>) {
    // Add functional groups as separate section
    if let Some(groups) = ecu.functional_groups()
        && !groups.is_empty()
    {
        b.push(
            0,
            "Functional Groups".to_string(),
            false,
            true,
            NodeType::SectionHeader,
        );

        // Sort functional groups alphabetically by name
        let mut sorted_groups: Vec<_> = groups.iter().collect();
        sorted_groups.sort_by_cached_key(|fg| {
            fg.diag_layer()
                .and_then(|dl| DiagLayer(dl).short_name())
                .unwrap_or("")
                .to_lowercase()
        });

        for fg in sorted_groups.into_iter() {
            if let Some(dl) = fg.diag_layer() {
                let layer = DiagLayer(dl);
                let name = layer.short_name().unwrap_or("unnamed");

                b.push(1, name.to_string(), false, true, NodeType::Container);

                // Functional groups don't have parent refs like variants
                // add_diag_layer_structured will handle adding functional classes
                b.add_diag_layer_structured(
                    &layer,
                    2,
                    name,
                    false,
                    None::<std::iter::Empty<cda_database::datatypes::ParentRef>>,
                    None::<std::iter::Empty<cda_database::datatypes::Variant>>,
                );
            }
        }
    }
}

/// Add all ECU shared data to the tree
pub fn add_ecu_shared_data(b: &mut TreeBuilder, ecu: &EcuDb<'_>) {
    // ECU shared data is accessed through functional groups -> parent refs
    // Following the pattern from the provided find_ecu_shared_services function

    // Collect ECU shared data from functional groups
    let ecu_shared_data_refs: Vec<_> = ecu
        .functional_groups()
        .into_iter()
        .flatten()
        .filter_map(|fg| {
            fg.parent_refs().and_then(|parent_refs| {
                // Find EcuSharedData parent refs
                let esd_refs: Vec<_> = parent_refs
                    .iter()
                    .filter_map(|parent_ref| {
                        let parent_ref = cda_database::datatypes::ParentRef(parent_ref);
                        match parent_ref.ref_type().try_into() {
                            Ok(cda_database::datatypes::ParentRefType::EcuSharedData) => {
                                parent_ref.ref__as_ecu_shared_data()
                            }
                            _ => None,
                        }
                    })
                    .collect();

                if esd_refs.is_empty() {
                    None
                } else {
                    Some(esd_refs)
                }
            })
        })
        .flatten()
        .collect();

    // Deduplicate by layer short name (same ECU shared data may be referenced by multiple FGs)
    let mut seen_names = std::collections::HashSet::new();
    let unique_esd: Vec<_> = ecu_shared_data_refs
        .into_iter()
        .filter(|esd| {
            if let Some(dl) = esd.diag_layer() {
                let name = dl.short_name().unwrap_or("");
                if !name.is_empty() && seen_names.contains(name) {
                    return false;
                }
                seen_names.insert(name.to_owned());
                true
            } else {
                false
            }
        })
        .collect();

    if !unique_esd.is_empty() {
        b.push(
            0,
            "ECU Shared Data".to_string(),
            false,
            true,
            NodeType::SectionHeader,
        );

        // Sort ECU shared data alphabetically by name
        let mut sorted_esd = unique_esd.clone();
        sorted_esd.sort_by_cached_key(|esd| {
            esd.diag_layer()
                .and_then(|dl| dl.short_name())
                .unwrap_or("")
                .to_lowercase()
        });

        for esd in sorted_esd.iter() {
            if let Some(dl) = esd.diag_layer() {
                let layer = DiagLayer(dl);
                let name = layer.short_name().unwrap_or("unnamed");

                b.push(1, name.to_string(), false, true, NodeType::Container);

                // ECU shared data doesn't have parent refs like variants
                // add_diag_layer_structured will handle adding functional classes
                b.add_diag_layer_structured(
                    &layer,
                    2,
                    name,
                    false,
                    None::<std::iter::Empty<cda_database::datatypes::ParentRef>>,
                    None::<std::iter::Empty<cda_database::datatypes::Variant>>,
                );
            }
        }
    }
}

/// Add all protocols to the tree
pub fn add_protocols(b: &mut TreeBuilder, ecu: &EcuDb<'_>) {
    // Protocols are accessed through functional groups -> parent refs
    // Similar to ECU shared data

    // Collect protocols from functional groups
    let protocol_refs: Vec<_> = ecu
        .functional_groups()
        .into_iter()
        .flatten()
        .filter_map(|fg| {
            fg.parent_refs().and_then(|parent_refs| {
                // Find Protocol parent refs
                let proto_refs: Vec<_> = parent_refs
                    .iter()
                    .filter_map(|parent_ref| {
                        let parent_ref = cda_database::datatypes::ParentRef(parent_ref);
                        match parent_ref.ref_type().try_into() {
                            Ok(cda_database::datatypes::ParentRefType::Protocol) => {
                                parent_ref.ref__as_protocol()
                            }
                            _ => None,
                        }
                    })
                    .collect();

                if proto_refs.is_empty() {
                    None
                } else {
                    Some(proto_refs)
                }
            })
        })
        .flatten()
        .collect();

    // Deduplicate by layer short name
    let mut seen_names = std::collections::HashSet::new();
    let unique_protocols: Vec<_> = protocol_refs
        .into_iter()
        .filter(|protocol| {
            if let Some(dl) = protocol.diag_layer() {
                let name = dl.short_name().unwrap_or("");
                if !name.is_empty() && seen_names.contains(name) {
                    return false;
                }
                seen_names.insert(name.to_owned());
                true
            } else {
                false
            }
        })
        .collect();

    if !unique_protocols.is_empty() {
        b.push(
            0,
            "Protocols".to_string(),
            false,
            true,
            NodeType::SectionHeader,
        );

        // Sort protocols alphabetically by name
        let mut sorted_protocols = unique_protocols.clone();
        sorted_protocols.sort_by_cached_key(|protocol| {
            protocol.diag_layer()
                .and_then(|dl| dl.short_name())
                .unwrap_or("")
                .to_lowercase()
        });

        for protocol in sorted_protocols.iter() {
            if let Some(dl) = protocol.diag_layer() {
                let layer = DiagLayer(dl);
                let name = layer.short_name().unwrap_or("unnamed");

                b.push(1, name.to_string(), false, true, NodeType::Container);

                // Protocols don't have parent refs like variants
                b.add_diag_layer_structured(
                    &layer,
                    2,
                    name,
                    false,
                    None::<std::iter::Empty<cda_database::datatypes::ParentRef>>,
                    None::<std::iter::Empty<cda_database::datatypes::Variant>>,
                );
            }
        }
    }
}

/// Get variant summary lines
fn get_variant_summary(variant: &VariantWrap<'_>, name: &str) -> Vec<String> {
    let mut d = vec![
        format!("Variant: {name}"),
        format!("Base Variant: {}", variant.is_base_variant()),
    ];
    if let Some(dl) = variant.diag_layer() {
        let layer = DiagLayer(dl);
        if let Some(sn) = layer.short_name() {
            d.push(format!("Short Name: {sn}"));
        }
        if let Some(ln) = layer.long_name() {
            d.push(format!("Long Name: {:?}", ln));
        }
    }
    d
}

/// Build parent refs table section for a variant
fn build_parent_refs_section(variant: &VariantWrap<'_>) -> Option<DetailSectionData> {
    let parent_refs = variant.parent_refs()?;
    let parent_refs_list: Vec<_> = parent_refs
        .iter()
        .map(cda_database::datatypes::ParentRef)
        .collect();

    if parent_refs_list.is_empty() {
        return None;
    }

    // Build table header
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

    Some(
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
        .with_type(DetailSectionType::RelatedRefs),
    )
}
