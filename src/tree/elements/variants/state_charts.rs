use cda_database::datatypes::DiagLayer;

use crate::tree::builder::TreeBuilder;
use crate::tree::types::{NodeType, DetailSectionData, DetailRow, DetailContent, CellType, ColumnConstraint};

/// Add state charts section to the tree
pub fn add_state_charts(b: &mut TreeBuilder, layer: &DiagLayer<'_>, depth: usize, _layer_name: &str) {
    let Some(charts) = layer.state_charts() else { return };
    if charts.is_empty() {
        return;
    }

    b.push(
        depth,
        format!("State Charts ({})", charts.len()),
        false,
        true,
        NodeType::SectionHeader,
    );

    for chart in charts.iter() {
        let chart_name = chart.short_name().unwrap_or("unnamed");
        
        // Get semantic description if available
        let semantic = chart.semantic().unwrap_or("");
        
        // Build State Transitions table
        let transitions: Vec<_> = chart.state_transitions().into_iter().flatten()
            .map(|tr| {
                let name = tr.short_name().unwrap_or("?");
                let src = tr.source_short_name_ref().unwrap_or("?");
                let tgt = tr.target_short_name_ref().unwrap_or("?");
                DetailRow {
                    cells: vec![name.to_string(), src.to_string(), tgt.to_string()],
                    cell_types: vec![CellType::Text, CellType::Text, CellType::Text],
                    indent: 0,
                }
            })
            .collect();
        
        let transitions_section = DetailSectionData {
            title: "State Transitions".to_string(),
            content: if transitions.is_empty() {
                DetailContent::PlainText(vec!["No state transitions".to_string()])
            } else {
                DetailContent::Table {
                    header: DetailRow {
                        cells: vec!["Name".to_string(), "Source".to_string(), "Target".to_string()],
                        cell_types: vec![CellType::Text, CellType::Text, CellType::Text],
                        indent: 0,
                    },
                    rows: transitions,
                    constraints: vec![
                        ColumnConstraint::Percentage(34),
                        ColumnConstraint::Percentage(33),
                        ColumnConstraint::Percentage(33),
                    ],
                    is_diag_comms: false,
                }
            },
        };
        
        // Build States table
        let states: Vec<_> = chart.states().into_iter().flatten()
            .map(|state| {
                let sn = state.short_name().unwrap_or("?");
                DetailRow {
                    cells: vec![sn.to_string()],
                    cell_types: vec![CellType::Text],
                    indent: 0,
                }
            })
            .collect();
        
        let states_section = DetailSectionData {
            title: "States".to_string(),
            content: if states.is_empty() {
                DetailContent::PlainText(vec!["No states".to_string()])
            } else {
                DetailContent::Table {
                    header: DetailRow {
                        cells: vec!["Name".to_string()],
                        cell_types: vec![CellType::Text],
                        indent: 0,
                    },
                    rows: states,
                    constraints: vec![ColumnConstraint::Percentage(100)],
                    is_diag_comms: false,
                }
            },
        };
        
        // Create sections list with semantic header (always, even if empty)
        let mut sections = vec![];
        
        // Add semantic information as first section (not a tab)
        sections.push(DetailSectionData {
            title: "Semantic".to_string(),
            content: DetailContent::PlainText(vec![format!("Semantic: {}", semantic)]),
        });
        
        // Add the two tab sections
        sections.push(transitions_section);
        sections.push(states_section);
        
        // Push the state chart as a non-expandable node with detail sections
        b.push_details_structured(
            depth + 1,
            chart_name.to_owned(),
            false,
            false, // Not expandable - no tree children
            sections,
            NodeType::Default,
        );
    }
}
