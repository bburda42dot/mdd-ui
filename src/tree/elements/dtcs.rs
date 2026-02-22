use cda_database::datatypes::EcuDb;

use crate::tree::{
    builder::TreeBuilder,
    types::{
        CellType, ColumnConstraint, DetailContent, DetailRow, DetailSectionData, DetailSectionType,
        NodeType, SectionType,
    },
};

/// Add all DTCs to the tree
pub fn add_dtcs(b: &mut TreeBuilder, ecu: &EcuDb<'_>) {
    if let Some(dtcs) = ecu.dtcs() {
        if dtcs.is_empty() {
            return;
        }

        // Deduplicate DTCs by trouble_code
        use std::collections::HashMap;
        let mut unique_dtcs = HashMap::new();
        
        for dtc in dtcs.iter() {
            let trouble_code = dtc.trouble_code();
            // Keep the first occurrence of each unique trouble code
            unique_dtcs.entry(trouble_code).or_insert(dtc);
        }
        
        // Convert back to a vector and sort by trouble code
        let mut unique_dtcs_vec: Vec<_> = unique_dtcs.into_values().collect();
        unique_dtcs_vec.sort_by_key(|dtc| dtc.trouble_code());

        // Build overview table for DTCs section header
        let mut overview_rows = Vec::new();
        
        for dtc in unique_dtcs_vec.iter() {
            let short_name = dtc.short_name().unwrap_or("?").to_owned();
            let display_code = dtc.display_trouble_code().unwrap_or("");
            let code_str = if !display_code.is_empty() {
                display_code.to_owned()
            } else {
                format!("0x{:06X}", dtc.trouble_code())
            };

            let description = dtc
                .text()
                .and_then(|t| t.value())
                .unwrap_or("")
                .to_owned();

            overview_rows.push(DetailRow::normal(
                vec![short_name, code_str, description],
                vec![CellType::Text, CellType::Text, CellType::Text],
                0,
            ));
        }

        let header = DetailRow::header(
            vec![
                "Short Name".to_owned(),
                "Code".to_owned(),
                "Description".to_owned(),
            ],
            vec![CellType::Text, CellType::Text, CellType::Text],
        );

        let overview_section = DetailSectionData::new(
            "DTCs Overview".to_owned(),
            DetailContent::Table {
                header,
                rows: overview_rows,
                constraints: vec![
                    ColumnConstraint::Percentage(30),
                    ColumnConstraint::Percentage(20),
                    ColumnConstraint::Percentage(50),
                ],
                use_row_selection: true,
            },
            false,
        )
        .with_type(DetailSectionType::Overview);

        b.push_section_header(
            format!("DTCs ({})", unique_dtcs_vec.len()),
            false,
            true,
            vec![overview_section],
            SectionType::DTCs,
        );

        // Add each unique DTC
        for dtc in unique_dtcs_vec.iter() {
            let short_name = dtc.short_name().unwrap_or("?");
            let trouble_code = dtc.trouble_code();
            let display_trouble_code = dtc.display_trouble_code().unwrap_or("");

            let display_name = if !display_trouble_code.is_empty() {
                format!("{} - {}", display_trouble_code, short_name)
            } else {
                format!("0x{:06X} - {}", trouble_code, short_name)
            };

            // Build detail sections inline
            let mut sections = Vec::new();

            // Header section
            let header_title = if !display_trouble_code.is_empty() {
                format!("DTC - {} - {}", display_trouble_code, short_name)
            } else {
                format!("DTC - 0x{:06X} - {}", trouble_code, short_name)
            };

            sections.push(DetailSectionData {
                title: header_title,
                render_as_header: true,
                content: DetailContent::PlainText(vec![]),
                section_type: DetailSectionType::Header,
            });

            // Overview section
            let mut detail_rows = Vec::new();

            detail_rows.push(DetailRow::normal(
                vec!["Short Name".to_owned(), short_name.to_owned()],
                vec![CellType::Text, CellType::Text],
                0,
            ));

            detail_rows.push(DetailRow::normal(
                vec![
                    "Trouble Code".to_owned(),
                    format!("0x{:06X}", trouble_code),
                ],
                vec![CellType::Text, CellType::Text],
                0,
            ));

            if !display_trouble_code.is_empty() {
                detail_rows.push(DetailRow::normal(
                    vec!["Display Code".to_owned(), display_trouble_code.to_owned()],
                    vec![CellType::Text, CellType::Text],
                    0,
                ));
            }

            if let Some(text) = dtc.text() {
                if let Some(value) = text.value() {
                    detail_rows.push(DetailRow::normal(
                        vec!["Description".to_owned(), value.to_owned()],
                        vec![CellType::Text, CellType::Text],
                        0,
                    ));
                }
            }

            if let Some(level) = dtc.level() {
                detail_rows.push(DetailRow::normal(
                    vec!["Level".to_owned(), level.to_string()],
                    vec![CellType::Text, CellType::Text],
                    0,
                ));
            }

            detail_rows.push(DetailRow::normal(
                vec![
                    "Temporary".to_owned(),
                    if dtc.is_temporary() {
                        "Yes".to_owned()
                    } else {
                        "No".to_owned()
                    },
                ],
                vec![CellType::Text, CellType::Text],
                0,
            ));

            let detail_header = DetailRow::header(
                vec!["Property".to_owned(), "Value".to_owned()],
                vec![CellType::Text, CellType::Text],
            );

            sections.push(
                DetailSectionData::new(
                    "Overview".to_owned(),
                    DetailContent::Table {
                        header: detail_header,
                        rows: detail_rows,
                        constraints: vec![
                            ColumnConstraint::Percentage(30),
                            ColumnConstraint::Percentage(70),
                        ],
                        use_row_selection: true,
                    },
                    false,
                )
                .with_type(DetailSectionType::Overview),
            );

            // SDGs section - extract actual SDG data
            let mut sdg_rows = Vec::new();
            if let Some(sdgs) = dtc.sdgs() {
                if let Some(sdg_list) = sdgs.sdgs() {
                    for sdg in sdg_list.iter() {
                        if let Some(caption) = sdg.caption_sn() {
                            let si = sdg.si().unwrap_or("-");
                            sdg_rows.push(DetailRow::normal(
                                vec![caption.to_owned(), si.to_owned()],
                                vec![CellType::Text, CellType::Text],
                                0,
                            ));
                        }
                    }
                }
            }

            if sdg_rows.is_empty() {
                sdg_rows.push(DetailRow::normal(
                    vec!["(No SDGs available)".to_owned()],
                    vec![CellType::Text],
                    0,
                ));
            }

            let sdg_header = DetailRow::header(
                vec!["Caption".to_owned(), "SI".to_owned()],
                vec![CellType::Text, CellType::Text],
            );
            sections.push(
                DetailSectionData::new(
                    "SDGs".to_owned(),
                    DetailContent::Table {
                        header: sdg_header,
                        rows: sdg_rows,
                        constraints: vec![
                            ColumnConstraint::Percentage(70),
                            ColumnConstraint::Percentage(30),
                        ],
                        use_row_selection: true,
                    },
                    false,
                )
                .with_type(DetailSectionType::Custom),
            );

            b.push_details_structured(
                1,
                display_name,
                false,
                false,
                sections,
                NodeType::Default,
            );
        }
    }
}
