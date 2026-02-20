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
