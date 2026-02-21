use cda_database::datatypes::{DiagLayer, DiagService, Parameter, ParentRef};

use super::services::{extract_coded_value, extract_dop_name, get_parent_ref_services_recursive};
use crate::tree::{
    builder::TreeBuilder,
    types::{
        CellType, ColumnConstraint, DetailContent, DetailRow, DetailSectionData, DetailSectionType,
        NodeType,
    },
};

/// Build Pos-Response sections - this is the core rendering logic for Pos-Response data
/// DiagComm module should import and use this function to render the Pos-Response tabs
pub fn build_pos_responses_sections(ds: &DiagService<'_>) -> Vec<DetailSectionData> {
    let mut sections = Vec::new();

    if let Some(pos) = ds.pos_responses() {
        for (i, resp) in pos.iter().enumerate() {
            let params = resp.params().into_iter().flatten().map(Parameter);
            sections.push(build_response_param_section(
                &format!("Pos-Response {}", i + 1),
                params,
            ));
        }
    }

    sections
}

/// Build Neg-Response sections - this is the core rendering logic for Neg-Response data
/// DiagComm module should import and use this function to render the Neg-Response tabs
pub fn build_neg_responses_sections(ds: &DiagService<'_>) -> Vec<DetailSectionData> {
    let mut sections = Vec::new();

    if let Some(neg) = ds.neg_responses() {
        for (i, resp) in neg.iter().enumerate() {
            let params = resp.params().into_iter().flatten().map(Parameter);
            sections.push(build_response_param_section(
                &format!("Neg-Response {}", i + 1),
                params,
            ));
        }
    }

    sections
}

/// Add positive responses section to the tree
pub fn add_pos_responses_section<'a>(
    b: &mut TreeBuilder,
    layer: &DiagLayer<'a>,
    depth: usize,
    variant_parent_refs: Option<impl Iterator<Item = ParentRef<'a>> + 'a>,
) {
    // Collect own services that have positive responses
    let mut own_services: Vec<DiagService<'_>> = layer
        .diag_services()
        .map(|services| {
            services
                .iter()
                .map(DiagService)
                .filter(|ds| ds.pos_responses().is_some_and(|r| !r.is_empty()))
                .collect()
        })
        .unwrap_or_default();

    // Sort own services alphabetically by name
    own_services.sort_by_cached_key(|ds| {
        ds.diag_comm()
            .and_then(|dc| dc.short_name())
            .unwrap_or("")
            .to_lowercase()
    });

    // Collect services from parent refs with source layer names (that have positive responses)
    let mut parent_services: Vec<(DiagService<'_>, String)> =
        if let Some(parent_refs) = variant_parent_refs {
            get_parent_ref_services_recursive(parent_refs)
                .into_iter()
                .filter(|(ds, _)| ds.pos_responses().is_some_and(|r| !r.is_empty()))
                .collect()
        } else {
            Vec::new()
        };

    // Sort parent services alphabetically by name
    parent_services.sort_by_cached_key(|(ds, _)| {
        ds.diag_comm()
            .and_then(|dc| dc.short_name())
            .unwrap_or("")
            .to_lowercase()
    });

    let total_count = own_services.len() + parent_services.len();

    if total_count > 0 {
        // Build detail section for Pos-Responses header showing all services in a table
        let detail_section = build_pos_responses_table_section(&own_services, &parent_services);

        b.push_service_list_header(
            depth,
            format!("Pos-Responses ({})", total_count),
            false,
            true,
            vec![detail_section],
            crate::tree::ServiceListType::PosResponses,
        );

        // Add own services first
        for ds in own_services.iter() {
            if let Some(dc) = ds.diag_comm() {
                let name = dc.short_name().unwrap_or("?");

                let display_name = if let Some(sid) = ds.request_id() {
                    if let Some((sub_fn, bit_len)) = ds.request_sub_function_id() {
                        let sub_fn_str = if bit_len <= 8 {
                            format!("{sub_fn:02X}")
                        } else {
                            format!("{sub_fn:04X}")
                        };
                        let full_id = format!("{sid:02X}{sub_fn_str}");
                        format!("0x{:6} - {}", full_id, name)
                    } else {
                        format!("0x{:6} - {}", format!("{sid:02X}"), name)
                    }
                } else {
                    name.to_string()
                };

                let sections = build_pos_response_view_sections(ds, None);

                b.push_details_structured(
                    depth + 1,
                    display_name,
                    false,
                    false,
                    sections,
                    NodeType::PosResponse,
                );
            }
        }

        // Add parent ref services
        for (ds, source_layer_name) in parent_services.iter() {
            if let Some(dc) = ds.diag_comm() {
                let name = dc.short_name().unwrap_or("?");

                let display_name = if let Some(sid) = ds.request_id() {
                    if let Some((sub_fn, bit_len)) = ds.request_sub_function_id() {
                        let sub_fn_str = if bit_len <= 8 {
                            format!("{sub_fn:02X}")
                        } else {
                            format!("{sub_fn:04X}")
                        };
                        let full_id = format!("{sid:02X}{sub_fn_str}");
                        format!("0x{:6} - {}", full_id, name)
                    } else {
                        format!("0x{:6} - {}", format!("{sid:02X}"), name)
                    }
                } else {
                    name.to_string()
                };

                let sections =
                    build_pos_response_view_sections(ds, Some(source_layer_name.clone()));

                b.push_details_structured(
                    depth + 1,
                    display_name,
                    false,
                    false,
                    sections,
                    NodeType::PosResponse,
                );
            }
        }
    }
}

/// Add negative responses section to the tree
pub fn add_neg_responses_section<'a>(
    b: &mut TreeBuilder,
    layer: &DiagLayer<'a>,
    depth: usize,
    variant_parent_refs: Option<impl Iterator<Item = ParentRef<'a>> + 'a>,
) {
    // Collect own services that have negative responses
    let mut own_services: Vec<DiagService<'_>> = layer
        .diag_services()
        .map(|services| {
            services
                .iter()
                .map(DiagService)
                .filter(|ds| ds.neg_responses().is_some_and(|r| !r.is_empty()))
                .collect()
        })
        .unwrap_or_default();

    // Sort own services alphabetically by name
    own_services.sort_by_cached_key(|ds| {
        ds.diag_comm()
            .and_then(|dc| dc.short_name())
            .unwrap_or("")
            .to_lowercase()
    });

    // Collect services from parent refs with source layer names (that have negative responses)
    let mut parent_services: Vec<(DiagService<'_>, String)> =
        if let Some(parent_refs) = variant_parent_refs {
            get_parent_ref_services_recursive(parent_refs)
                .into_iter()
                .filter(|(ds, _)| ds.neg_responses().is_some_and(|r| !r.is_empty()))
                .collect()
        } else {
            Vec::new()
        };

    // Sort parent services alphabetically by name
    parent_services.sort_by_cached_key(|(ds, _)| {
        ds.diag_comm()
            .and_then(|dc| dc.short_name())
            .unwrap_or("")
            .to_lowercase()
    });

    let total_count = own_services.len() + parent_services.len();

    if total_count > 0 {
        // Build detail section for Neg-Responses header showing all services in a table
        let detail_section = build_neg_responses_table_section(&own_services, &parent_services);

        b.push_service_list_header(
            depth,
            format!("Neg-Responses ({})", total_count),
            false,
            true,
            vec![detail_section],
            crate::tree::ServiceListType::NegResponses,
        );

        // Add own services first
        for ds in own_services.iter() {
            if let Some(dc) = ds.diag_comm() {
                let name = dc.short_name().unwrap_or("?");

                let display_name = if let Some(sid) = ds.request_id() {
                    if let Some((sub_fn, bit_len)) = ds.request_sub_function_id() {
                        let sub_fn_str = if bit_len <= 8 {
                            format!("{sub_fn:02X}")
                        } else {
                            format!("{sub_fn:04X}")
                        };
                        let full_id = format!("{sid:02X}{sub_fn_str}");
                        format!("0x{:6} - {}", full_id, name)
                    } else {
                        format!("0x{:6} - {}", format!("{sid:02X}"), name)
                    }
                } else {
                    name.to_string()
                };

                let sections = build_neg_response_view_sections(ds, None);

                b.push_details_structured(
                    depth + 1,
                    display_name,
                    false,
                    false,
                    sections,
                    NodeType::NegResponse,
                );
            }
        }

        // Add parent ref services
        for (ds, source_layer_name) in parent_services.iter() {
            if let Some(dc) = ds.diag_comm() {
                let name = dc.short_name().unwrap_or("?");

                let display_name = if let Some(sid) = ds.request_id() {
                    if let Some((sub_fn, bit_len)) = ds.request_sub_function_id() {
                        let sub_fn_str = if bit_len <= 8 {
                            format!("{sub_fn:02X}")
                        } else {
                            format!("{sub_fn:04X}")
                        };
                        let full_id = format!("{sid:02X}{sub_fn_str}");
                        format!("0x{:6} - {}", full_id, name)
                    } else {
                        format!("0x{:6} - {}", format!("{sid:02X}"), name)
                    }
                } else {
                    name.to_string()
                };

                let sections =
                    build_neg_response_view_sections(ds, Some(source_layer_name.clone()));

                b.push_details_structured(
                    depth + 1,
                    display_name,
                    false,
                    false,
                    sections,
                    NodeType::NegResponse,
                );
            }
        }
    }
}

/// Build complete service view with Pos-Response tabs (used by Pos-Responses section)
fn build_pos_response_view_sections(
    ds: &DiagService<'_>,
    parent_layer_name: Option<String>,
) -> Vec<DetailSectionData> {
    let mut sections = Vec::new();

    // Add header section
    let service_name = ds.diag_comm().and_then(|dc| dc.short_name()).unwrap_or("?");
    let header_title = if let Some(sid) = ds.request_id() {
        format!("Pos Response - 0x{:02X} - {}", sid, service_name)
    } else {
        format!("Pos Response - {}", service_name)
    };

    sections.push(DetailSectionData {
        title: header_title,
        render_as_header: true,
        section_type: DetailSectionType::Header,
        content: DetailContent::PlainText(vec![]),
    });

    // Overview
    sections.push(build_overview_section(ds, parent_layer_name));

    // Pos-Response tabs - use rendering logic from this module
    sections.extend(build_pos_responses_sections(ds));

    sections
}

/// Build complete service view with Neg-Response tabs (used by Neg-Responses section)
fn build_neg_response_view_sections(
    ds: &DiagService<'_>,
    parent_layer_name: Option<String>,
) -> Vec<DetailSectionData> {
    let mut sections = Vec::new();

    // Add header section
    let service_name = ds.diag_comm().and_then(|dc| dc.short_name()).unwrap_or("?");
    let header_title = if let Some(sid) = ds.request_id() {
        format!("Neg Response - 0x{:02X} - {}", sid, service_name)
    } else {
        format!("Neg Response - {}", service_name)
    };

    sections.push(DetailSectionData {
        title: header_title,
        render_as_header: true,
        section_type: DetailSectionType::Header,
        content: DetailContent::PlainText(vec![]),
    });

    // Overview
    sections.push(build_overview_section(ds, parent_layer_name));

    // Neg-Response tabs - use rendering logic from this module
    sections.extend(build_neg_responses_sections(ds));

    sections
}

/// Build overview section (shared by both pos and neg response views)
fn build_overview_section(
    ds: &DiagService<'_>,
    parent_layer_name: Option<String>,
) -> DetailSectionData {
    let header = DetailRow {
        cells: vec!["Property".to_owned(), "Value".to_owned()],
        cell_types: vec![CellType::Text, CellType::Text],
        indent: 0,
        ..Default::default()
    };

    let mut rows = Vec::new();

    if let Some(dc) = ds.diag_comm() {
        if let Some(sn) = dc.short_name() {
            rows.push(DetailRow {
                cells: vec!["Service".to_owned(), sn.to_owned()],
                cell_types: vec![CellType::Text, CellType::Text],
                indent: 0,
                ..Default::default()
            });
        }
        if let Some(semantic) = dc.semantic() {
            rows.push(DetailRow {
                cells: vec!["Semantic".to_owned(), semantic.to_owned()],
                cell_types: vec![CellType::Text, CellType::Text],
                indent: 0,
                ..Default::default()
            });
        }
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
        rows.push(DetailRow {
            cells: vec![
                "Sub-Function".to_owned(),
                format!("0x{sub_fn:04X} ({bit_len} bits)"),
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

    if let Some(parent_name) = parent_layer_name {
        rows.push(DetailRow {
            cells: vec!["Inherited From".to_owned(), parent_name],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
            ..Default::default()
        });
    }

    DetailSectionData {
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
    }
}

/// Helper to build parameter table for responses
fn build_response_param_section<'a, I>(title: &str, params: I) -> DetailSectionData
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

    let mut rows: Vec<DetailRow> = Vec::new();

    for param in params {
        let name = param.short_name().unwrap_or("?").to_owned();
        let byte_pos = param.byte_position();
        let bit_pos = param.bit_position();
        let bit_len = "-".to_owned();
        let byte_len = "-".to_owned();
        let value = extract_coded_value(&param);
        let dop_name = extract_dop_name(&param);
        let semantic = param.semantic().unwrap_or_default().to_owned();
        let has_dop = !dop_name.is_empty();

        rows.push(DetailRow {
            cells: vec![
                name,
                byte_pos.to_string(),
                bit_pos.to_string(),
                bit_len,
                byte_len,
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
            indent: 0,
            ..Default::default()
        });
    }

    DetailSectionData {
        title: title.to_owned(),
        render_as_header: false,
        section_type: DetailSectionType::PosResponses,
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

/// Build a table section for the Pos-Responses header showing all services
fn build_pos_responses_table_section(
    own_services: &[DiagService<'_>],
    parent_services: &[(DiagService<'_>, String)],
) -> DetailSectionData {
    let header = DetailRow {
        cells: vec![
            "ID".to_owned(),
            "Short Name".to_owned(),
            "Inherited".to_owned(),
        ],
        cell_types: vec![CellType::Text, CellType::Text, CellType::Text],
        indent: 0,
        ..Default::default()
    };

    let mut rows = Vec::new();

    // Add own services first (inherited = false)
    for ds in own_services.iter() {
        if let Some(dc) = ds.diag_comm() {
            let name = dc.short_name().unwrap_or("?").to_owned();

            let id = if let Some(sid) = ds.request_id() {
                if let Some((sub_fn, bit_len)) = ds.request_sub_function_id() {
                    let sub_fn_str = if bit_len <= 8 {
                        format!("{sub_fn:02X}")
                    } else {
                        format!("{sub_fn:04X}")
                    };
                    let full_id = format!("{sid:02X}{sub_fn_str}");
                    format!("0x{}", full_id)
                } else {
                    format!("0x{:02X}", sid)
                }
            } else {
                "-".to_owned()
            };

            rows.push(DetailRow {
                cells: vec![id, name, "false".to_owned()],
                cell_types: vec![CellType::Text, CellType::Text, CellType::Text],
                indent: 0,
                ..Default::default()
            });
        }
    }

    // Add parent services (inherited = true)
    for (ds, _source_layer_name) in parent_services.iter() {
        if let Some(dc) = ds.diag_comm() {
            let name = dc.short_name().unwrap_or("?").to_owned();

            let id = if let Some(sid) = ds.request_id() {
                if let Some((sub_fn, bit_len)) = ds.request_sub_function_id() {
                    let sub_fn_str = if bit_len <= 8 {
                        format!("{sub_fn:02X}")
                    } else {
                        format!("{sub_fn:04X}")
                    };
                    let full_id = format!("{sid:02X}{sub_fn_str}");
                    format!("0x{}", full_id)
                } else {
                    format!("0x{:02X}", sid)
                }
            } else {
                "-".to_owned()
            };

            rows.push(DetailRow {
                cells: vec![id, name, "true".to_owned()],
                cell_types: vec![CellType::Text, CellType::Text, CellType::Text],
                indent: 0,
                ..Default::default()
            });
        }
    }

    let total_count = own_services.len() + parent_services.len();

    DetailSectionData {
        title: format!("Pos-Responses ({})", total_count),
        render_as_header: false,
        section_type: DetailSectionType::PosResponses,
        content: DetailContent::Table {
            header,
            rows,
            constraints: vec![
                ColumnConstraint::Percentage(20),
                ColumnConstraint::Percentage(60),
                ColumnConstraint::Percentage(20),
            ],
            use_row_selection: true,
        },
    }
}

/// Build a table section for the Neg-Responses header showing all services
fn build_neg_responses_table_section(
    own_services: &[DiagService<'_>],
    parent_services: &[(DiagService<'_>, String)],
) -> DetailSectionData {
    let header = DetailRow {
        cells: vec![
            "ID".to_owned(),
            "Short Name".to_owned(),
            "Inherited".to_owned(),
        ],
        cell_types: vec![CellType::Text, CellType::Text, CellType::Text],
        indent: 0,
        ..Default::default()
    };

    let mut rows = Vec::new();

    // Add own services first (inherited = false)
    for ds in own_services.iter() {
        if let Some(dc) = ds.diag_comm() {
            let name = dc.short_name().unwrap_or("?").to_owned();

            let id = if let Some(sid) = ds.request_id() {
                if let Some((sub_fn, bit_len)) = ds.request_sub_function_id() {
                    let sub_fn_str = if bit_len <= 8 {
                        format!("{sub_fn:02X}")
                    } else {
                        format!("{sub_fn:04X}")
                    };
                    let full_id = format!("{sid:02X}{sub_fn_str}");
                    format!("0x{}", full_id)
                } else {
                    format!("0x{:02X}", sid)
                }
            } else {
                "-".to_owned()
            };

            rows.push(DetailRow {
                cells: vec![id, name, "false".to_owned()],
                cell_types: vec![CellType::Text, CellType::Text, CellType::Text],
                indent: 0,
                ..Default::default()
            });
        }
    }

    // Add parent services (inherited = true)
    for (ds, _source_layer_name) in parent_services.iter() {
        if let Some(dc) = ds.diag_comm() {
            let name = dc.short_name().unwrap_or("?").to_owned();

            let id = if let Some(sid) = ds.request_id() {
                if let Some((sub_fn, bit_len)) = ds.request_sub_function_id() {
                    let sub_fn_str = if bit_len <= 8 {
                        format!("{sub_fn:02X}")
                    } else {
                        format!("{sub_fn:04X}")
                    };
                    let full_id = format!("{sid:02X}{sub_fn_str}");
                    format!("0x{}", full_id)
                } else {
                    format!("0x{:02X}", sid)
                }
            } else {
                "-".to_owned()
            };

            rows.push(DetailRow {
                cells: vec![id, name, "true".to_owned()],
                cell_types: vec![CellType::Text, CellType::Text, CellType::Text],
                indent: 0,
                ..Default::default()
            });
        }
    }

    let total_count = own_services.len() + parent_services.len();

    DetailSectionData {
        title: format!("Neg-Responses ({})", total_count),
        render_as_header: false,
        section_type: DetailSectionType::NegResponses,
        content: DetailContent::Table {
            header,
            rows,
            constraints: vec![
                ColumnConstraint::Percentage(20),
                ColumnConstraint::Percentage(60),
                ColumnConstraint::Percentage(20),
            ],
            use_row_selection: true,
        },
    }
}
