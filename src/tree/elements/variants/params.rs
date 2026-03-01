/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2026 Alexander Mohr
 */

use cda_database::datatypes::{DiagService, Parameter};

use super::{
    format_service_id,
    services::{extract_coded_value, extract_dop_name},
};
use crate::tree::types::{
    BIT_POSITION_UNSET, CellJumpTarget, CellType, ColumnConstraint, DetailContent, DetailRow,
    DetailSectionData, DetailSectionType,
};

/// Build detail sections for a single parameter (Overview with key-value
/// properties).  Shared by both request and response parameter views.
pub fn build_param_detail_sections(param: &Parameter<'_>) -> Vec<DetailSectionData> {
    let mut sections = Vec::new();

    let param_name = param.short_name().unwrap_or("?");
    sections.push(DetailSectionData {
        title: format!("Parameter - {param_name}"),
        render_as_header: true,
        section_type: DetailSectionType::Header,
        content: DetailContent::PlainText(vec![]),
    });

    let mut overview_rows = Vec::new();

    if let Some(short_name) = param.short_name() {
        overview_rows.push(DetailRow::normal(
            vec!["Short Name".to_owned(), short_name.to_owned()],
            vec![CellType::Text, CellType::Text],
            0,
        ));
    }

    if let Ok(param_type) = param.param_type() {
        use cda_database::datatypes::ParamType;
        let param_type_str = match param_type {
            ParamType::CodedConst => "CodedConst",
            ParamType::Dynamic => "Dynamic",
            ParamType::LengthKey => "LengthKey",
            ParamType::MatchingRequestParam => "MatchingRequestParam",
            ParamType::NrcConst => "NrcConst",
            ParamType::PhysConst => "PhysConst",
            ParamType::Reserved => "Reserved",
            ParamType::System => "System",
            ParamType::TableEntry => "TableEntry",
            ParamType::TableKey => "TableKey",
            ParamType::TableStruct => "TableStruct",
            ParamType::Value => "Value",
        };
        overview_rows.push(DetailRow::normal(
            vec!["Type".to_owned(), param_type_str.to_owned()],
            vec![CellType::Text, CellType::Text],
            0,
        ));
    }

    if let Some(semantic) = param.semantic() {
        overview_rows.push(DetailRow::normal(
            vec!["Semantic".to_owned(), semantic.to_owned()],
            vec![CellType::Text, CellType::Text],
            0,
        ));
    }

    let byte_pos = param.byte_position();
    if byte_pos != 0 {
        overview_rows.push(DetailRow::normal(
            vec!["Byte Position".to_owned(), byte_pos.to_string()],
            vec![CellType::Text, CellType::NumericValue],
            0,
        ));
    }

    let bit_pos = param.bit_position();
    if bit_pos != BIT_POSITION_UNSET {
        overview_rows.push(DetailRow::normal(
            vec!["Bit Position".to_owned(), bit_pos.to_string()],
            vec![CellType::Text, CellType::NumericValue],
            0,
        ));
    }

    let coded_value = extract_coded_value(param);
    if !coded_value.is_empty() {
        overview_rows.push(DetailRow::normal(
            vec!["Coded Value".to_owned(), coded_value],
            vec![CellType::Text, CellType::NumericValue],
            0,
        ));
    }

    let dop_name = extract_dop_name(param);
    if !dop_name.is_empty() {
        overview_rows.push(DetailRow::normal(
            vec!["DOP".to_owned(), dop_name],
            vec![CellType::Text, CellType::DopReference],
            0,
        ));
    }

    if !overview_rows.is_empty() {
        let header = DetailRow::header(
            vec!["Property".to_owned(), "Value".to_owned()],
            vec![CellType::Text, CellType::Text],
        );

        sections.push(
            DetailSectionData::new(
                "Overview".to_owned(),
                DetailContent::Table {
                    header,
                    rows: overview_rows,
                    constraints: vec![
                        ColumnConstraint::Percentage(40),
                        ColumnConstraint::Percentage(60),
                    ],
                    use_row_selection: true,
                },
                false,
            )
            .with_type(DetailSectionType::Overview),
        );
    }

    sections
}

/// Build a parameter table section (the column-based param list used by
/// request / response detail views).  `section_type` distinguishes Requests
/// from `PosResponses` / `NegResponses`.
pub fn build_param_section<'a, I>(
    title: &str,
    params: I,
    section_type: DetailSectionType,
) -> DetailSectionData
where
    I: IntoIterator<Item = Parameter<'a>>,
{
    let header = DetailRow {
        cells: vec![
            "Short Name".to_owned(),
            "Byte".to_owned(),
            "Bit".to_owned(),
            "Bit\nLen".to_owned(),
            "Byte\nLen".to_owned(),
            "Value".to_owned(),
            "DOP".to_owned(),
            "Semantic".to_owned(),
        ],
        cell_types: vec![
            CellType::Text,
            CellType::NumericValue,
            CellType::NumericValue,
            CellType::NumericValue,
            CellType::NumericValue,
            CellType::Text,
            CellType::Text,
            CellType::Text,
        ],
        indent: 0,
        ..Default::default()
    };

    let rows: Vec<DetailRow> = params
        .into_iter()
        .map(|param| {
            let name = param.short_name().unwrap_or("?").to_owned();
            let byte_pos = param.byte_position();
            let bit_pos = param.bit_position();
            let value = extract_coded_value(&param);
            let dop_name = extract_dop_name(&param);
            let semantic = param.semantic().unwrap_or_default().to_owned();
            let has_dop = !dop_name.is_empty();
            let param_id = param.id();

            let dop_jump = if has_dop {
                Some(CellJumpTarget::Dop {
                    name: dop_name.clone(),
                })
            } else {
                None
            };

            DetailRow {
                cells: vec![
                    name,
                    byte_pos.to_string(),
                    bit_pos.to_string(),
                    "-".to_owned(),
                    "-".to_owned(),
                    value,
                    dop_name,
                    semantic,
                ],
                cell_types: vec![
                    CellType::ParameterName,
                    CellType::NumericValue,
                    CellType::NumericValue,
                    CellType::Text,
                    CellType::Text,
                    CellType::NumericValue,
                    if has_dop {
                        CellType::DopReference
                    } else {
                        CellType::Text
                    },
                    CellType::Text,
                ],
                cell_jump_targets: vec![
                    Some(CellJumpTarget::Parameter { param_id }),
                    None,
                    None,
                    None,
                    None,
                    None,
                    dop_jump,
                    None,
                ],
                indent: 0,
                row_type: crate::tree::DetailRowType::Normal,
                metadata: Some(crate::tree::RowMetadata::ParameterRow { param_id }),
            }
        })
        .collect();

    DetailSectionData {
        title: title.to_owned(),
        render_as_header: false,
        section_type,
        content: DetailContent::Table {
            header,
            rows,
            constraints: vec![
                ColumnConstraint::Percentage(45),
                ColumnConstraint::Fixed(4),
                ColumnConstraint::Fixed(3),
                ColumnConstraint::Fixed(4),
                ColumnConstraint::Fixed(5),
                ColumnConstraint::Percentage(15),
                ColumnConstraint::Percentage(15),
                ColumnConstraint::Percentage(25),
            ],
            use_row_selection: false,
        },
    }
}

/// Build a service-list table section (the header table showing all services
/// with Short Name / ID / Inherited columns).  Used by both the Requests and
/// Responses list headers.
pub fn build_service_list_table_section(
    own_services: &[DiagService<'_>],
    parent_services: &[(DiagService<'_>, String)],
    label: &str,
    section_type: DetailSectionType,
) -> DetailSectionData {
    let header = DetailRow {
        cells: vec![
            "Short Name".to_owned(),
            "ID".to_owned(),
            "Inherited".to_owned(),
        ],
        cell_types: vec![CellType::Text, CellType::Text, CellType::Text],
        indent: 0,
        ..Default::default()
    };

    let build_row = |ds: &DiagService<'_>, inherited: &str| -> Option<DetailRow> {
        let name = ds.diag_comm()?.short_name().unwrap_or("?").to_owned();
        let id_str = format_service_id(ds);
        let id = if id_str.is_empty() {
            "-".to_owned()
        } else {
            id_str
        };
        Some(DetailRow {
            cells: vec![name, id, inherited.to_owned()],
            cell_types: vec![CellType::ParameterName, CellType::Text, CellType::Text],
            cell_jump_targets: vec![Some(CellJumpTarget::TreeNodeByName), None, None],
            indent: 0,
            ..Default::default()
        })
    };

    let mut rows = Vec::new();
    rows.extend(own_services.iter().filter_map(|ds| build_row(ds, "false")));
    rows.extend(
        parent_services
            .iter()
            .filter_map(|(ds, _)| build_row(ds, "true")),
    );

    let total_count = own_services.len().saturating_add(parent_services.len());

    DetailSectionData {
        title: format!("{label} ({total_count})"),
        render_as_header: false,
        section_type,
        content: DetailContent::Table {
            header,
            rows,
            constraints: vec![
                ColumnConstraint::Percentage(60),
                ColumnConstraint::Percentage(20),
                ColumnConstraint::Percentage(20),
            ],
            use_row_selection: true,
        },
    }
}
