mod types;
mod builder;
mod elements;

use cda_database::datatypes::DiagnosticDatabase;

// Re-export public types
pub use types::{
    TreeNode, NodeType, CellType, DetailRow, ColumnConstraint,
    DetailContent, DetailSectionData, lines_to_single_section,
};

use builder::TreeBuilder;
use elements::{add_variants, add_functional_groups};
use crate::database::{extract_data, get_ecu_summary};

/// Walk the entire database and produce a flat list of tree nodes ready for
/// the TUI to display.
pub fn build_tree(db: &DiagnosticDatabase) -> Vec<TreeNode> {
    let mut b = TreeBuilder::new();

    // Extract database data
    let data = extract_data(db);
    let ecu_name = &data.ecu_name;

    // Add General section with ECU info
    if let Some(ref ecu) = data.ecu {
        let ecu_details = get_ecu_summary(db, ecu_name);
        let ecu_section = lines_to_single_section("Summary", ecu_details.clone());
        b.push_details_structured(
            0,
            "General".to_string(),
            false,
            false,
            vec![ecu_section],
            NodeType::SectionHeader,
        );
        
        add_variants(&mut b, ecu);
        add_functional_groups(&mut b, ecu);
    }

    b.finish()
}
