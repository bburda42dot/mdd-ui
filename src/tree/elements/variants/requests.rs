/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2026 Alexander Mohr
 */

use cda_database::datatypes::{DiagLayer, DiagService, Parameter, ParentRef};

use super::{
    params::{build_param_detail_sections, build_param_section, build_service_list_table_section},
    services::get_parent_ref_services_recursive,
};
use crate::tree::{
    builder::TreeBuilder,
    types::{
        CellType, ColumnConstraint, DetailContent, DetailRow, DetailSectionData, DetailSectionType,
        NodeType,
    },
};

/// Add requests section to the tree
/// This uses EXACTLY the same logic and display as `DiagComm` - just filtered
/// to show only services with requests
pub fn add_requests_section<'a>(
    b: &mut TreeBuilder,
    layer: &DiagLayer<'a>,
    depth: usize,
    variant_parent_refs: Option<impl Iterator<Item = ParentRef<'a>> + 'a>,
) {
    // Collect own services that have requests
    let own_services: Vec<DiagService<'_>> = layer
        .diag_services()
        .map(|services| {
            services
                .iter()
                .map(DiagService)
                .filter(|ds| ds.request().is_some())
                .collect()
        })
        .unwrap_or_default();

    // Collect services from parent refs with source layer names (that have requests)
    let parent_services: Vec<(DiagService<'_>, String)> =
        if let Some(parent_refs) = variant_parent_refs {
            get_parent_ref_services_recursive(parent_refs)
                .into_iter()
                .filter(|(ds, _)| ds.request().is_some())
                .collect()
        } else {
            Vec::new()
        };

    let total_count = own_services.len().saturating_add(parent_services.len());

    if total_count > 0 {
        let detail_section = build_service_list_table_section(
            &own_services,
            &parent_services,
            "Requests",
            DetailSectionType::Requests,
        );

        b.push_service_list_header(
            depth,
            format!("Requests ({total_count})"),
            false,
            true,
            vec![detail_section],
            crate::tree::ServiceListType::Requests,
        );

        // Add own services first - using EXACTLY the same display as DiagComm
        for ds in &own_services {
            let Some(display_name) = super::format_service_display_name(ds) else {
                continue;
            };

            // Build full service details, but with Request tab rendered by this module
            let sections = build_request_view_sections(ds, None);

            // Check if there are params to show as children
            let has_params = ds
                .request()
                .and_then(|req| req.params())
                .is_some_and(|p| !p.is_empty());

            b.push_details_structured(
                depth.saturating_add(1),
                display_name.clone(),
                false,
                has_params,
                sections,
                NodeType::Request, // Use Request node type for navigation
            );

            // Add params as child nodes
            ds.request()
                .and_then(|req| req.params())
                .into_iter()
                .flat_map(|params| params.iter().map(Parameter))
                .for_each(|param| {
                    let param_name = param.short_name().unwrap_or("?").to_owned();
                    let param_detail = build_param_detail_sections(&param);
                    let param_id = param.id();

                    b.push_param(
                        depth.saturating_add(2),
                        param_name,
                        param_detail,
                        NodeType::Default,
                        param_id,
                    );
                });
        }

        // Add parent ref services with different node type (same as DiagComm)
        for (ds, source_layer_name) in &parent_services {
            let Some(display_name) = super::format_service_display_name(ds) else {
                continue;
            };

            // Build full service details, but with Request tab rendered by this module
            let sections = build_request_view_sections(ds, Some(source_layer_name.clone()));

            // Check if there are params to show as children
            let has_params = ds
                .request()
                .and_then(|req| req.params())
                .is_some_and(|p| !p.is_empty());

            b.push_details_structured(
                depth.saturating_add(1),
                display_name.clone(),
                false,
                has_params,
                sections,
                NodeType::Request, // Use Request node type for navigation (inherited)
            );

            // Add params as child nodes
            ds.request()
                .and_then(|req| req.params())
                .into_iter()
                .flat_map(|params| params.iter().map(Parameter))
                .for_each(|param| {
                    let param_name = param.short_name().unwrap_or("?").to_owned();
                    let param_detail = build_param_detail_sections(&param);
                    let param_id = param.id();

                    b.push_param(
                        depth.saturating_add(2),
                        param_name,
                        param_detail,
                        NodeType::Default,
                        param_id,
                    );
                });
        }
    }
}

/// Build the Request tab section - this is the core rendering logic for Request data
/// `DiagComm` module should import and use this function to render the Request tab
/// Always returns a section (empty table if no request data)
pub fn build_request_section(ds: &DiagService<'_>) -> DetailSectionData {
    let params: Vec<Parameter<'_>> = ds
        .request()
        .and_then(|req| req.params())
        .into_iter()
        .flatten()
        .map(Parameter)
        .collect();

    build_param_section("Request", params, DetailSectionType::Requests)
}

/// Build complete service view with Request tab (used by Requests section)
fn build_request_view_sections(
    ds: &DiagService<'_>,
    parent_layer_name: Option<String>,
) -> Vec<DetailSectionData> {
    let mut sections = Vec::new();

    // Add header section with service ID and name
    let service_name = ds.diag_comm().and_then(|dc| dc.short_name()).unwrap_or("?");
    let id_str = super::format_service_id(ds);
    let header_title = if id_str.is_empty() {
        format!("Request - {service_name}")
    } else {
        format!("Request - {id_str} - {service_name}")
    };

    sections.push(DetailSectionData {
        title: header_title,
        render_as_header: true,
        section_type: DetailSectionType::Header,
        content: DetailContent::PlainText(vec![]),
    });

    // Overview - table with key-value pairs
    let header = DetailRow {
        cells: vec!["Property".to_owned(), "Value".to_owned()],
        cell_types: vec![CellType::Text, CellType::Text],
        indent: 0,
        ..Default::default()
    };

    let mut rows = Vec::new();

    if let Some(dc) = ds.diag_comm() {
        rows.extend(dc.short_name().map(|sn| DetailRow {
            cells: vec!["Service".to_owned(), sn.to_owned()],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
            ..Default::default()
        }));
        rows.extend(dc.semantic().map(|semantic| DetailRow {
            cells: vec!["Semantic".to_owned(), semantic.to_owned()],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
            ..Default::default()
        }));
    }
    if let Some(sid) = ds.request_id() {
        rows.push(DetailRow {
            cells: vec!["SID".to_owned(), format!("0x{sid:02X}")],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
            ..Default::default()
        });
    }
    if let Some((sub_fn, bit_len)) = ds.request_sub_function_id() {
        let sub_fn_str = if bit_len <= 8 {
            format!("0x{sub_fn:02X}")
        } else {
            format!("0x{sub_fn:04X}")
        };
        rows.push(DetailRow {
            cells: vec![
                "Sub-Function".to_owned(),
                format!("{sub_fn_str} ({bit_len} bits)"),
            ],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
            ..Default::default()
        });
    }
    rows.push(DetailRow {
        cells: vec!["Addressing".to_owned(), format!("{:?}", ds.addressing())],
        cell_types: vec![CellType::Text, CellType::Text],
        indent: 0,
        ..Default::default()
    });
    rows.push(DetailRow {
        cells: vec![
            "Transmission".to_owned(),
            format!("{:?}", ds.transmission_mode()),
        ],
        cell_types: vec![CellType::Text, CellType::Text],
        indent: 0,
        ..Default::default()
    });

    // Add inheritance information only if inherited
    if let Some(parent_name) = parent_layer_name {
        rows.push(DetailRow::inherited_from(parent_name));
    }

    sections.push(DetailSectionData {
        title: "Overview".to_owned(),
        render_as_header: false,
        section_type: DetailSectionType::Overview,
        content: DetailContent::Table {
            header,
            rows,
            constraints: vec![
                ColumnConstraint::Percentage(30),
                ColumnConstraint::Percentage(70),
            ],
            use_row_selection: true,
        },
    });

    // Request params - use the rendering logic from this module
    sections.push(build_request_section(ds));

    sections
}
