// Submodules that represent the tree hierarchy under variants
pub mod services;
pub mod requests;
pub mod responses;
pub mod state_charts;
pub mod com_params;
pub mod placeholders;

use cda_database::datatypes::{DiagLayer, EcuDb, Variant as VariantWrap};

use crate::tree::builder::TreeBuilder;
use crate::tree::types::{NodeType, lines_to_single_section};

use super::layers::LayerExt;

/// Add all variants to the tree
pub fn add_variants(b: &mut TreeBuilder, ecu: &EcuDb<'_>) {
    // Add Variants section
    if let Some(variants) = ecu.variants() {
        b.push(
            0,
            "Variants".to_string(),
            true,
            true,
            NodeType::SectionHeader,
        );
        
        // Add each variant
        for (vi, variant) in variants.iter().enumerate() {
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

            let details = get_variant_summary(&vw, &name);
            let sec = lines_to_single_section("Summary", details.clone());
            b.push_details_structured(
                1,
                name.clone(),
                is_base,
                true,
                vec![sec],
                NodeType::Container,
            );

            // Add diag layer content directly under variant (no section header)
            if let Some(dl) = vw.diag_layer() {
                let layer = DiagLayer(dl);
                // Pass parent refs from variant for inherited service lookup
                let parent_refs_iter = vw.parent_refs().map(|pr| pr.iter().map(cda_database::datatypes::ParentRef));
                b.add_diag_layer_structured(&layer, 2, &name, is_base, parent_refs_iter);
            }
        }
    }
}

/// Add all functional groups to the tree
pub fn add_functional_groups(b: &mut TreeBuilder, ecu: &EcuDb<'_>) {
    // Add functional groups as separate section
    if let Some(groups) = ecu.functional_groups()
        && !groups.is_empty() {
        b.push(
            0,
            "Functional Groups".to_string(),
            false,
            true,
            NodeType::SectionHeader,
        );
        
        for fg in groups.iter() {
            if let Some(dl) = fg.diag_layer() {
                let layer = DiagLayer(dl);
                let name = layer.short_name().unwrap_or("unnamed");
                
                b.push(
                    1,
                    name.to_string(),
                    false,
                    true,
                    NodeType::Container,
                );
                
                // Functional groups don't have parent refs like variants
                b.add_diag_layer_structured(&layer, 2, name, false, None::<std::iter::Empty<cda_database::datatypes::ParentRef>>);
            }
        }
    }
}

/// Add all ECU shared data to the tree
pub fn add_ecu_shared_data(b: &mut TreeBuilder, ecu: &EcuDb<'_>) {
    // ECU shared data is accessed through functional groups -> parent refs
    // Following the pattern from the provided find_ecu_shared_services function
    
    // Collect ECU shared data from functional groups
    let ecu_shared_data_refs: Vec<_> = ecu.functional_groups()
        .into_iter()
        .flatten()
        .filter_map(|fg| {
            fg.parent_refs().and_then(|parent_refs| {
                // Find EcuSharedData parent refs
                let esd_refs: Vec<_> = parent_refs.iter()
                    .filter_map(|parent_ref| {
                        let parent_ref = cda_database::datatypes::ParentRef(parent_ref);
                        match parent_ref.ref_type().try_into() {
                            Ok(cda_database::datatypes::ParentRefType::EcuSharedData) => {
                                parent_ref.ref__as_ecu_shared_data()
                            }
                            _ => None
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
    let unique_esd: Vec<_> = ecu_shared_data_refs.into_iter()
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
        
        for esd in unique_esd.iter() {
            if let Some(dl) = esd.diag_layer() {
                let layer = DiagLayer(dl);
                let name = layer.short_name().unwrap_or("unnamed");
                
                b.push(
                    1,
                    name.to_string(),
                    false,
                    true,
                    NodeType::Container,
                );
                
                // ECU shared data doesn't have parent refs like variants
                b.add_diag_layer_structured(&layer, 2, name, false, None::<std::iter::Empty<cda_database::datatypes::ParentRef>>);
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
