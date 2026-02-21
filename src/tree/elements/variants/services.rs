use cda_database::datatypes::{DiagLayer, DiagService, ParamType, Parameter, ParentRef};

// Import rendering functions from specialized modules
use super::requests::build_request_section;
use super::responses::{build_neg_responses_sections, build_pos_responses_sections};
use crate::tree::{
    builder::TreeBuilder,
    types::{
        CellType, ColumnConstraint, DetailContent, DetailRow, DetailRowType, DetailSectionData,
        DetailSectionType, NodeType,
    },
};

// Helper functions to extract parameter data
pub fn extract_coded_value(param: &Parameter<'_>) -> String {
    let Ok(pt) = param.param_type() else {
        return String::new();
    };

    if !matches!(pt, ParamType::CodedConst) {
        return String::new();
    }

    param
        .specific_data_as_coded_const()
        .and_then(|cc| cc.coded_value())
        .map(|v| {
            if let Ok(num) = v.parse::<u64>() {
                // Format with minimum 2 hex digits (0x01, 0x10, 0x100, etc.)
                if num <= 0xFF {
                    format!("0x{num:02X}")
                } else if num <= 0xFFFF {
                    format!("0x{num:04X}")
                } else if num <= 0xFFFFFF {
                    format!("0x{num:06X}")
                } else if num <= 0xFFFFFFFF {
                    format!("0x{num:08X}")
                } else {
                    format!("0x{num:016X}")
                }
            } else {
                v.to_owned()
            }
        })
        .unwrap_or_default()
}

pub fn extract_dop_name(param: &Parameter<'_>) -> String {
    let Ok(pt) = param.param_type() else {
        return String::new();
    };

    if !matches!(pt, ParamType::Value) {
        return String::new();
    }

    param
        .specific_data_as_value()
        .and_then(|vd| vd.dop())
        .and_then(|dop| dop.short_name())
        .map(|s| s.to_owned())
        .unwrap_or_default()
}

/// Add Diag-Comms section (services and jobs) to the tree
pub fn add_diag_comms<'a>(
    b: &mut TreeBuilder,
    layer: &DiagLayer<'a>,
    depth: usize,
    _layer_name: &str,
    variant_parent_refs: Option<impl Iterator<Item = ParentRef<'a>> + 'a>,
) {
    // Collect own services
    let own_services: Vec<DiagService<'_>> = layer
        .diag_services()
        .map(|services| services.iter().map(DiagService).collect())
        .unwrap_or_default();

    // Collect services from parent refs with source layer names
    let parent_services: Vec<(DiagService<'_>, String)> =
        if let Some(parent_refs) = variant_parent_refs {
            get_parent_ref_services_recursive(parent_refs)
        } else {
            Vec::new()
        };

    // Count single ECU jobs
    let job_count = layer.single_ecu_jobs().map(|jobs| jobs.len()).unwrap_or(0);

    let service_count = own_services.len() + parent_services.len();
    let total_count = service_count + job_count;

    if total_count > 0 {
        // Build detail section for Diag-Comms header showing all services and jobs
        // Collect job names from diag_com
        let job_names: Vec<String> = layer
            .single_ecu_jobs()
            .map(|jobs| {
                jobs.iter()
                    .map(|job| {
                        job.diag_comm()
                            .and_then(|dc| dc.short_name())
                            .unwrap_or("Unnamed")
                            .to_owned()
                    })
                    .collect()
            })
            .unwrap_or_default();
        let detail_section =
            build_diag_comms_table_section(&own_services, &parent_services, &job_names);

        b.push_service_list_header(
            depth,
            format!(
                "Diag-Comms ({} services, {} jobs)",
                service_count, job_count
            ),
            false,
            true,
            vec![detail_section],
            crate::tree::ServiceListType::DiagComms,
        );

        // Add own services first
        for ds in own_services.iter() {
            if let Some(dc) = ds.diag_comm() {
                let name = dc.short_name().unwrap_or("?");

                // Format with service ID with proper padding for alignment
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

                let sections = build_diag_comm_details_with_parent(ds, None);

                b.push_details_structured(
                    depth + 1,
                    format!("[Service] {}", display_name),
                    false,
                    false,
                    sections,
                    NodeType::Service,
                );
            }
        }

        // Add parent ref services with different node type
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
                    build_diag_comm_details_with_parent(ds, Some(source_layer_name.clone()));

                b.push_details_structured(
                    depth + 1,
                    format!("[Service] {}", display_name),
                    false,
                    false,
                    sections,
                    NodeType::ParentRefService, // Mark as inherited
                );
            }
        }

        // Add single ECU jobs after services
        if let Some(jobs) = layer.single_ecu_jobs() {
            for job in jobs.iter() {
                let job_name = job
                    .diag_comm()
                    .and_then(|dc| dc.short_name())
                    .unwrap_or("Unnamed");
                let sections = build_simple_job_details_with_name(job_name);

                b.push_details_structured(
                    depth + 1,
                    format!("[Job] {}", job_name),
                    false,
                    false,
                    sections,
                    NodeType::Default,
                );
            }
        }
    }
}

/// Get services from parent references recursively with proper filtering
/// Returns a tuple of (DiagService, source_layer_name)
pub fn get_parent_ref_services_recursive<'a>(
    parent_refs: impl Iterator<Item = ParentRef<'a>>,
) -> Vec<(DiagService<'a>, String)> {
    fn filter_not_inherited_services<'a>(
        diag_services: impl Iterator<Item = impl Into<DiagService<'a>>>,
        not_inherited_names: &[&str],
        source_layer_name: String,
    ) -> Vec<(DiagService<'a>, String)> {
        diag_services
            .into_iter()
            .map(|s| s.into())
            .filter(|service| {
                service
                    .diag_comm()
                    .and_then(|dc| dc.short_name())
                    .is_none_or(|name| !not_inherited_names.contains(&name))
            })
            .map(|service| (service, source_layer_name.clone()))
            .collect()
    }

    fn find_services_recursive<'a>(
        parent_refs: impl Iterator<Item = ParentRef<'a>>,
    ) -> Vec<(DiagService<'a>, String)> {
        parent_refs
            .into_iter()
            .filter_map(|parent_ref| {
                // Get the list of short names that should not be inherited
                let not_inherited_names: Vec<&str> = parent_ref
                    .not_inherited_diag_comm_short_names()
                    .map(|names| names.iter().collect())
                    .unwrap_or_default();

                match parent_ref.ref_type().try_into() {
                    Ok(cda_database::datatypes::ParentRefType::EcuSharedData) => {
                        let ecu_shared_data = parent_ref.ref__as_ecu_shared_data()?;
                        let layer = ecu_shared_data.diag_layer()?;
                        let layer_name = layer.short_name().unwrap_or("EcuSharedData").to_owned();
                        let services = layer.diag_services()?.iter().map(DiagService);
                        Some(filter_not_inherited_services(
                            services,
                            &not_inherited_names,
                            layer_name,
                        ))
                    }
                    Ok(cda_database::datatypes::ParentRefType::FunctionalGroup) => parent_ref
                        .ref__as_functional_group()
                        .and_then(|fg| fg.parent_refs())
                        .map(|nested_refs| {
                            find_services_recursive(nested_refs.iter().map(ParentRef))
                        }),
                    Ok(cda_database::datatypes::ParentRefType::Protocol) => {
                        let protocol = parent_ref.ref__as_protocol()?;
                        let layer = protocol.diag_layer()?;
                        let layer_name = layer.short_name().unwrap_or("Protocol").to_owned();
                        let services = layer.diag_services()?.iter().map(DiagService);
                        Some(filter_not_inherited_services(
                            services,
                            &not_inherited_names,
                            layer_name,
                        ))
                    }
                    Ok(cda_database::datatypes::ParentRefType::Variant) => {
                        let variant = parent_ref.ref__as_variant()?;
                        let layer = variant.diag_layer()?;
                        let layer_name = layer.short_name().unwrap_or("Variant").to_owned();
                        let services = layer.diag_services()?.iter().map(DiagService);
                        Some(filter_not_inherited_services(
                            services,
                            &not_inherited_names,
                            layer_name,
                        ))
                    }
                    _ => {
                        // Unsupported parent ref type
                        None
                    }
                }
            })
            .flatten()
            .collect()
    }

    find_services_recursive(parent_refs)
}

/// Build detailed sections for a diagnostic service with optional parent layer info
pub fn build_diag_comm_details_with_parent(
    ds: &DiagService<'_>,
    parent_layer_name: Option<String>,
) -> Vec<DetailSectionData> {
    let mut sections: Vec<DetailSectionData> = Vec::new();

    // Add header section with service ID and name (matching tree display format)
    let service_name = ds.diag_comm().and_then(|dc| dc.short_name()).unwrap_or("?");
    let header_title = if let Some(sid) = ds.request_id() {
        if let Some((sub_fn, bit_len)) = ds.request_sub_function_id() {
            let sub_fn_str = if bit_len <= 8 {
                format!("{sub_fn:02X}")
            } else {
                format!("{sub_fn:04X}")
            };
            let full_id = format!("{sid:02X}{sub_fn_str}");
            format!("Service - 0x{} - {}", full_id, service_name)
        } else {
            format!("Service - 0x{:02X} - {}", sid, service_name)
        }
    } else {
        format!("Service - {}", service_name)
    };

    sections.push(DetailSectionData {
        title: header_title,
        render_as_header: true,
        content: DetailContent::PlainText(vec![]),
        section_type: DetailSectionType::Header,
    });

    // Overview - table with key-value pairs
    let header = DetailRow::header(
        vec!["Property".to_owned(), "Value".to_owned()],
        vec![CellType::Text, CellType::Text],
    );

    let mut rows = Vec::new();

    if let Some(dc) = ds.diag_comm() {
        if let Some(sn) = dc.short_name() {
            rows.push(DetailRow::normal(
                vec!["Service".to_owned(), sn.to_owned()],
                vec![CellType::Text, CellType::Text],
                0,
            ));
        }
        if let Some(semantic) = dc.semantic() {
            rows.push(DetailRow::normal(
                vec!["Semantic".to_owned(), semantic.to_owned()],
                vec![CellType::Text, CellType::Text],
                0,
            ));
        }
    }
    if let Some(sid) = ds.request_id() {
        rows.push(DetailRow::normal(
            vec!["SID".to_owned(), format!("0x{sid:02X}")],
            vec![CellType::Text, CellType::Text],
            0,
        ));
    }
    if let Some((sub_fn, bit_len)) = ds.request_sub_function_id() {
        rows.push(DetailRow::normal(
            vec![
                "Sub-Function".to_owned(),
                format!("0x{sub_fn:04X} ({bit_len} bits)"),
            ],
            vec![CellType::Text, CellType::Text],
            0,
        ));
    }
    rows.push(DetailRow::normal(
        vec!["Addressing".to_owned(), format!("{:?}", ds.addressing())],
        vec![CellType::Text, CellType::Text],
        0,
    ));
    rows.push(DetailRow::normal(
        vec![
            "Transmission".to_owned(),
            format!("{:?}", ds.transmission_mode()),
        ],
        vec![CellType::Text, CellType::Text],
        0,
    ));

    // Add inheritance information only if inherited
    if let Some(parent_name) = parent_layer_name {
        rows.push(DetailRow::inherited_from(parent_name));
    }

    sections.push(
        DetailSectionData::new(
            "Overview".to_owned(),
            DetailContent::Table {
                header,
                rows,
                constraints: vec![
                    ColumnConstraint::Percentage(30),
                    ColumnConstraint::Percentage(70),
                ],
                use_row_selection: true, // Use row selection for Overview
            },
            false,
        )
        .with_type(DetailSectionType::Overview),
    );

    // Request params - use rendering logic from requests module
    if let Some(request_section) = build_request_section(ds) {
        sections.push(request_section);
    }

    // Pos responses - use rendering logic from responses module
    sections.extend(build_pos_responses_sections(ds));

    // Neg responses - use rendering logic from responses module
    sections.extend(build_neg_responses_sections(ds));

    // ComParam refs
    let comparam_header = DetailRow {
        row_type: DetailRowType::Normal,
        metadata: None,
        cells: vec![
            "ComParam".to_owned(),
            "Value".to_owned(),
            "Complex Value".to_owned(),
            "Protocol".to_owned(),
            "Prot-Stack".to_owned(),
        ],
        cell_types: vec![
            CellType::Text,
            CellType::Text,
            CellType::Text,
            CellType::Text,
            CellType::Text,
        ],
        indent: 0,
    };
    sections.push(DetailSectionData {
        title: "ComParam-Refs".to_owned(),
        render_as_header: false,
        section_type: DetailSectionType::ComParams,
        content: DetailContent::Table {
            header: comparam_header,
            rows: vec![DetailRow {
                row_type: DetailRowType::Normal,
                metadata: None,
                cells: vec!["(No ComParam refs at comm level)".to_owned()],
                cell_types: vec![CellType::Text],
                indent: 0,
            }],
            constraints: vec![
                ColumnConstraint::Percentage(20),
                ColumnConstraint::Percentage(20),
                ColumnConstraint::Percentage(20),
                ColumnConstraint::Percentage(20),
                ColumnConstraint::Percentage(20),
            ],
            use_row_selection: false,
        },
    });

    // Audience section - Combined into single tab with composite content
    if let Some(dc) = ds.diag_comm() {
        if let Some(audience) = dc.audience() {
            let mut subsections = Vec::new();

            // Flags subsection
            let mut flag_lines = Vec::new();
            flag_lines.push(format!(
                "IS_MANUFACTURER: {}",
                if audience.is_manufacturing() {
                    "true"
                } else {
                    "false"
                }
            ));
            flag_lines.push(format!(
                "IS_DEVELOPMENT: {}",
                if audience.is_development() {
                    "true"
                } else {
                    "false"
                }
            ));
            flag_lines.push(format!(
                "IS_AFTERSALES: {}",
                if audience.is_after_sales() {
                    "true"
                } else {
                    "false"
                }
            ));
            flag_lines.push(format!(
                "IS_AFTERMARKET: {}",
                if audience.is_after_market() {
                    "true"
                } else {
                    "false"
                }
            ));

            subsections.push(DetailSectionData {
                title: "Audience Flags".to_owned(),
                render_as_header: false,
                section_type: DetailSectionType::Custom,
                content: DetailContent::PlainText(flag_lines),
            });

            // Additional audiences subsection
            if let Some(audiences) = audience.enabled_audiences() {
                let audiences_list: Vec<_> = audiences
                    .iter()
                    .filter_map(|aa| aa.short_name().map(|s| s.to_owned()))
                    .collect();

                if !audiences_list.is_empty() {
                    subsections.push(DetailSectionData {
                        title: "Additional Audiences".to_owned(),
                        render_as_header: false,
                        section_type: DetailSectionType::Custom,
                        content: DetailContent::PlainText(audiences_list),
                    });
                }
            }

            sections.push(DetailSectionData {
                title: "Audience".to_owned(),
                render_as_header: false,
                section_type: DetailSectionType::Custom,
                content: DetailContent::Composite(subsections),
            });
        } else {
            sections.push(DetailSectionData {
                title: "Audience".to_owned(),
                render_as_header: false,
                section_type: DetailSectionType::Custom,
                content: DetailContent::PlainText(vec!["(No audience info)".to_owned()]),
            });
        }
    }

    // SDGs
    let sdg_header = DetailRow {
        row_type: DetailRowType::Normal,
        metadata: None,
        cells: vec![
            "Short Name".to_owned(),
            "SD".to_owned(),
            "SI".to_owned(),
            "TI".to_owned(),
        ],
        cell_types: vec![
            CellType::Text,
            CellType::Text,
            CellType::Text,
            CellType::Text,
        ],
        indent: 0,
    };
    sections.push(DetailSectionData {
        title: "SDGs".to_owned(),
        render_as_header: false,
        section_type: DetailSectionType::Custom,
        content: DetailContent::Table {
            header: sdg_header,
            rows: vec![DetailRow {
                row_type: DetailRowType::Normal,
                metadata: None,
                cells: vec!["(SDGs not available at comm level)".to_owned()],
                cell_types: vec![CellType::Text],
                indent: 0,
            }],
            constraints: vec![
                ColumnConstraint::Percentage(40),
                ColumnConstraint::Percentage(20),
                ColumnConstraint::Percentage(20),
                ColumnConstraint::Percentage(20),
            ],
            use_row_selection: false,
        },
    });

    // States
    if let Some(dc) = ds.diag_comm() {
        let header = DetailRow {
            row_type: DetailRowType::Normal,
            metadata: None,
            cells: vec!["Short Name".to_owned()],
            cell_types: vec![CellType::Text],
            indent: 0,
        };

        let mut rows = Vec::new();
        for pc in dc.pre_condition_state_refs().into_iter().flatten() {
            if let Some(val) = pc.value() {
                rows.push(DetailRow {
                    row_type: DetailRowType::Normal,
                    metadata: None,
                    cells: vec![val.to_owned()],
                    cell_types: vec![CellType::Text],
                    indent: 0,
                });
            }
        }
        for st in dc.state_transition_refs().into_iter().flatten() {
            if let Some(val) = st.value() {
                rows.push(DetailRow {
                    row_type: DetailRowType::Normal,
                    metadata: None,
                    cells: vec![val.to_owned()],
                    cell_types: vec![CellType::Text],
                    indent: 0,
                });
            }
        }
        sections.push(DetailSectionData {
            title: "States".to_owned(),
            render_as_header: false,
            section_type: DetailSectionType::States,
            content: DetailContent::Table {
                header,
                rows,
                constraints: vec![ColumnConstraint::Percentage(100)],
                use_row_selection: false,
            },
        });
    }

    // Related diag comm refs
    let related_header = DetailRow {
        row_type: DetailRowType::Normal,
        metadata: None,
        cells: vec!["Short Name".to_owned()],
        cell_types: vec![CellType::Text],
        indent: 0,
    };
    sections.push(DetailSectionData {
        title: "Related-Diag-Comm-Refs".to_owned(),
        render_as_header: false,
        section_type: DetailSectionType::RelatedRefs,
        content: DetailContent::Table {
            header: related_header,
            rows: vec![DetailRow {
                row_type: DetailRowType::Normal,
                metadata: None,
                cells: vec!["(Related comms not available)".to_owned()],
                cell_types: vec![CellType::Text],
                indent: 0,
            }],
            constraints: vec![ColumnConstraint::Percentage(100)],
            use_row_selection: false,
        },
    });

    sections
}
/// Build a table section for the Diag-Comms header showing all services and jobs
fn build_diag_comms_table_section(
    own_services: &[DiagService<'_>],
    parent_services: &[(DiagService<'_>, String)],
    job_names: &[String],
) -> DetailSectionData {
    let header = DetailRow {
        row_type: DetailRowType::Normal,
        metadata: None,
        cells: vec![
            "ID".to_owned(),
            "Short Name".to_owned(),
            "Type".to_owned(),
            "Inherited".to_owned(),
        ],
        cell_types: vec![
            CellType::Text,
            CellType::Text,
            CellType::Text,
            CellType::Text,
        ],
        indent: 0,
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
                row_type: DetailRowType::Normal,
                metadata: None,
                cells: vec![id, name, "Service".to_owned(), "false".to_owned()],
                cell_types: vec![
                    CellType::Text,
                    CellType::Text,
                    CellType::Text,
                    CellType::Text,
                ],
                indent: 0,
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
                row_type: DetailRowType::Normal,
                metadata: None,
                cells: vec![id, name, "Service".to_owned(), "true".to_owned()],
                cell_types: vec![
                    CellType::Text,
                    CellType::Text,
                    CellType::Text,
                    CellType::Text,
                ],
                indent: 0,
            });
        }
    }

    // Add job entries to the table
    for job_name in job_names.iter() {
        rows.push(DetailRow {
            row_type: DetailRowType::Normal,
            metadata: None,
            cells: vec![
                "-".to_owned(),
                job_name.clone(),
                "Job".to_owned(),
                "false".to_owned(),
            ],
            cell_types: vec![
                CellType::Text,
                CellType::Text,
                CellType::Text,
                CellType::Text,
            ],
            indent: 0,
        });
    }

    DetailSectionData {
        title: format!(
            "Diag-Comms ({} services, {} jobs)",
            own_services.len() + parent_services.len(),
            job_names.len()
        ),
        render_as_header: false,
        section_type: DetailSectionType::Services,
        content: DetailContent::Table {
            header,
            rows,
            constraints: vec![
                ColumnConstraint::Percentage(15),
                ColumnConstraint::Percentage(45),
                ColumnConstraint::Percentage(15),
                ColumnConstraint::Percentage(25),
            ],
            use_row_selection: true,
        },
    }
}

/// Build detailed sections for a single ECU job (simplified placeholder)
pub fn build_simple_job_details_with_name(job_name: &str) -> Vec<DetailSectionData> {
    let mut sections = Vec::new();

    // Add header section with job name
    sections.push(DetailSectionData {
        title: format!("Job - {}", job_name),
        render_as_header: true,
        content: DetailContent::PlainText(vec![]),
        section_type: DetailSectionType::Header,
    });

    // Overview section
    let header = DetailRow {
        row_type: DetailRowType::Normal,
        metadata: None,
        cells: vec!["Property".to_owned(), "Value".to_owned()],
        cell_types: vec![CellType::Text, CellType::Text],
        indent: 0,
    };

    let rows = vec![
        DetailRow {
            row_type: DetailRowType::Normal,
            metadata: None,
            cells: vec!["Job Name".to_owned(), job_name.to_owned()],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
        },
        DetailRow {
            row_type: DetailRowType::Normal,
            metadata: None,
            cells: vec!["Status".to_owned(), "Available in database".to_owned()],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
        },
    ];

    sections.push(DetailSectionData {
        title: "Job Information".to_owned(),
        render_as_header: false,
        section_type: DetailSectionType::Custom,
        content: DetailContent::Table {
            header,
            rows,
            constraints: vec![
                ColumnConstraint::Percentage(30),
                ColumnConstraint::Percentage(70),
            ],
            use_row_selection: false,
        },
    });

    sections
}
