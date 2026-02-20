use cda_database::datatypes::DiagLayer;

use crate::tree::builder::TreeBuilder;
use crate::tree::types::NodeType;

/// Add ComParam refs section to the tree
pub fn add_com_params(b: &mut TreeBuilder, layer: &DiagLayer<'_>, depth: usize, _layer_name: &str) {
    let Some(cp_refs) = layer.com_param_refs() else { return };
    if cp_refs.is_empty() {
        return;
    }

    b.push(
        depth,
        format!("ComParam Refs ({})", cp_refs.len()),
        false,
        true,
        NodeType::SectionHeader,
    );

    for cpr in cp_refs.iter() {
        let Some(cp) = cpr.com_param() else { continue };
        let cp_name = cp.short_name().unwrap_or("?");
        let cp_type = format!("{:?}", cp.com_param_type());
        b.push_leaf(
            depth + 1,
            format!("{cp_name} ({cp_type})"),
            NodeType::Default,
        );
    }
}
