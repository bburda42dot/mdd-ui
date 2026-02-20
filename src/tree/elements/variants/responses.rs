use cda_database::datatypes::{DiagLayer, DiagService};

use crate::tree::builder::TreeBuilder;
use crate::tree::types::NodeType;

/// Add positive responses section to the tree
pub fn add_pos_responses_section(b: &mut TreeBuilder, layer: &DiagLayer<'_>, depth: usize, _layer_name: &str) {
    if let Some(services) = layer.diag_services() {
        let response_count: usize = services.iter().filter(|&s| {
            DiagService(s).pos_responses().is_some_and(|r| !r.is_empty())
        }).count();
        
        if response_count > 0 {
            b.push(
                depth,
                format!("Pos-Responses ({})", response_count),
                false,
                true,
                NodeType::SectionHeader,
            );
            
            for svc in services.iter() {
                let ds = DiagService(svc);
                if ds.pos_responses().is_some_and(|r| !r.is_empty()) {
                    let name = ds.diag_comm().and_then(|dc| dc.short_name()).unwrap_or("?");
                    b.push_leaf(
                        depth + 1,
                        format!("PosResponse: {name}"),
                        NodeType::PosResponse,
                    );
                }
            }
        }
    }
}

/// Add negative responses section to the tree
pub fn add_neg_responses_section(b: &mut TreeBuilder, layer: &DiagLayer<'_>, depth: usize, _layer_name: &str) {
    if let Some(services) = layer.diag_services() {
        let response_count: usize = services.iter().filter(|&s| {
            DiagService(s).neg_responses().is_some_and(|r| !r.is_empty())
        }).count();
        
        if response_count > 0 {
            b.push(
                depth,
                format!("Neg-Responses ({})", response_count),
                false,
                true,
                NodeType::SectionHeader,
            );
            
            for svc in services.iter() {
                let ds = DiagService(svc);
                if ds.neg_responses().is_some_and(|r| !r.is_empty()) {
                    let name = ds.diag_comm().and_then(|dc| dc.short_name()).unwrap_or("?");
                    b.push_leaf(
                        depth + 1,
                        format!("NegResponse: {name}"),
                        NodeType::NegResponse,
                    );
                }
            }
        }
    }
}
