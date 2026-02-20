use cda_database::datatypes::{DiagLayer, DiagService};

use crate::tree::builder::TreeBuilder;
use crate::tree::types::NodeType;

/// Add requests section to the tree
pub fn add_requests_section(b: &mut TreeBuilder, layer: &DiagLayer<'_>, depth: usize, _layer_name: &str) {
    if let Some(services) = layer.diag_services() {
        let request_count: usize = services.iter().filter(|&s| {
            DiagService(s).request().is_some()
        }).count();
        
        if request_count > 0 {
            b.push(
                depth,
                format!("Requests ({})", request_count),
                false,
                true,
                NodeType::SectionHeader,
            );
            
            for svc in services.iter() {
                let ds = DiagService(svc);
                if ds.request().is_some() {
                    let name = ds.diag_comm().and_then(|dc| dc.short_name()).unwrap_or("?");
                    b.push_leaf(
                        depth + 1,
                        format!("Request: {name}"),
                        NodeType::Request,
                    );
                }
            }
        }
    }
}
