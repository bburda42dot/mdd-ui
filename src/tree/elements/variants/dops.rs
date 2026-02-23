use cda_database::datatypes::{DiagLayer, DiagService, Parameter};
use crate::tree::{
    builder::TreeBuilder,
    types::{
        CellType, ColumnConstraint, DetailContent, DetailRow, DetailRowType,
        DetailSectionData, DetailSectionType, NodeType,
    },
};

/// Structure to hold DOP information with reference to actual DOP for detail extraction
struct DopInfo<'a> {
    name: String,
    dop_type: String, // Stores the EXACT SpecificDOPData union variant
    dop: cda_database::datatypes::DataOperation<'a>, // Actual DOP for extracting details
    category: Option<String>, // Compu category
    internal_unit: Option<String>, // Internal unit (from diag coded type or parsed name)
    phys_unit: Option<String>, // Physical unit (from unit_ref)
    desc_id: Option<String>, // Description ID
}

/// Parsed information from DOP name (e.g., IDENTICAL_A_UINT32_0x00_0xFFFFFFFF_MicroSecond)
struct ParsedDopName {
    compu_category: Option<String>,
    data_type: Option<String>,
    range_min: Option<String>,
    range_max: Option<String>,
    unit: Option<String>,
}

/// Parse encoded information from DOP short name
fn parse_dop_name(name: &str) -> ParsedDopName {
    let parts: Vec<&str> = name.split('_').collect();
    let mut parsed = ParsedDopName {
        compu_category: None,
        data_type: None,
        range_min: None,
        range_max: None,
        unit: None,
    };

    if parts.is_empty() {
        return parsed;
    }

    // First part might be compu category (IDENTICAL, LINEAR, TEXT_TABLE, etc.)
    let compu_categories = ["IDENTICAL", "LINEAR", "TEXTTABLE", "SCALE", "COMPUCODE", "TABINTP", "RATFUNC"];
    if compu_categories.iter().any(|&cat| parts[0].starts_with(cat)) {
        parsed.compu_category = Some(parts[0].to_owned());
    }

    // Look for data type patterns (A_UINT32, A_INT32, A_FLOAT32, A_ASCIISTRING, etc.)
    for (i, part) in parts.iter().enumerate() {
        if part.starts_with("A_") {
            // Combine A_ with next part if it exists
            if i + 1 < parts.len() {
                parsed.data_type = Some(format!("{}_{}", part, parts[i + 1]));
            } else {
                parsed.data_type = Some(part.to_string());
            }
        }
    }

    // Look for hex ranges (0x patterns)
    let hex_parts: Vec<&str> = parts.iter().filter(|p| p.starts_with("0x") || p.starts_with("0X")).copied().collect();
    if hex_parts.len() >= 2 {
        parsed.range_min = Some(hex_parts[0].to_owned());
        parsed.range_max = Some(hex_parts[1].to_owned());
    } else if hex_parts.len() == 1 {
        parsed.range_min = Some(hex_parts[0].to_owned());
    }

    // Last part might be unit (if not a hex value or data type)
    if let Some(last) = parts.last() {
        if !last.starts_with("0x") && !last.starts_with("0X") && !last.starts_with("A_") {
            // Common unit patterns
            let units = ["Second", "MicroSecond", "MilliSecond", "Meter", "KiloMeter", "Volt", "Ampere", "Celsius", "Pascal"];
            if units.iter().any(|&u| last.contains(u)) {
                parsed.unit = Some(last.to_string());
            }
        }
    }

    parsed
}

/// Add DOPs section to the tree by collecting from service/job request/response params
pub fn add_dops_section<'a>(
    b: &mut TreeBuilder,
    layer: &DiagLayer<'a>,
    depth: usize,
) {
    let mut all_dops: Vec<DopInfo<'a>> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Collect DOPs from own services (requests + responses)
    if let Some(services) = layer.diag_services() {
        for svc in services.iter() {
            let ds = DiagService(svc);
            collect_dops_from_service(&ds, &mut all_dops, &mut seen);
        }
    }

    // Collect DOPs from single ECU jobs (input/output/neg_output params)
    if let Some(jobs) = layer.single_ecu_jobs() {
        for job in jobs.iter() {
            // Collect from input params
            if let Some(input_params) = job.input_params() {
                for jp in input_params.iter() {
                    if let Some(dop) = jp.dop_base() {
                        collect_single_dop(cda_database::datatypes::DataOperation(dop), &mut all_dops, &mut seen);
                    }
                }
            }
            // Collect from output params
            if let Some(output_params) = job.output_params() {
                for jp in output_params.iter() {
                    if let Some(dop) = jp.dop_base() {
                        collect_single_dop(cda_database::datatypes::DataOperation(dop), &mut all_dops, &mut seen);
                    }
                }
            }
            // Collect from negative output params
            if let Some(neg_output_params) = job.neg_output_params() {
                for jp in neg_output_params.iter() {
                    if let Some(dop) = jp.dop_base() {
                        collect_single_dop(cda_database::datatypes::DataOperation(dop), &mut all_dops, &mut seen);
                    }
                }
            }
        }
    }

    // DOPs are already deduplicated via the seen set
    let unique_dops = all_dops;

    if unique_dops.is_empty() {
        return;
    }

    // Group DOPs by semantic category using the actual DataOperationVariant
    use cda_database::datatypes::DataOperationVariant;
    let mut dtc_dops: Vec<DopInfo> = Vec::new();
    let mut env_data_descs: Vec<DopInfo> = Vec::new();
    let mut data_object_props: Vec<DopInfo> = Vec::new();
    let mut structures: Vec<DopInfo> = Vec::new();
    let mut static_fields: Vec<DopInfo> = Vec::new();
    let mut dynamic_length_fields: Vec<DopInfo> = Vec::new();
    let mut end_of_pdu_fields: Vec<DopInfo> = Vec::new();
    let mut mux_dops: Vec<DopInfo> = Vec::new();

    for dop_info in unique_dops {
        if let Ok(variant) = dop_info.dop.variant() {
            match variant {
                DataOperationVariant::Dtc(_) => dtc_dops.push(dop_info),
                DataOperationVariant::EnvDataDesc(_) | DataOperationVariant::EnvData(_) => {
                    env_data_descs.push(dop_info)
                }
                DataOperationVariant::Structure(_) => structures.push(dop_info),
                DataOperationVariant::StaticField(_) => static_fields.push(dop_info),
                DataOperationVariant::DynamicLengthField(_) => {
                    dynamic_length_fields.push(dop_info)
                }
                DataOperationVariant::EndOfPdu(_) => end_of_pdu_fields.push(dop_info),
                DataOperationVariant::Mux(_) => mux_dops.push(dop_info),
                DataOperationVariant::Normal(_) => data_object_props.push(dop_info),
            }
        } else {
            data_object_props.push(dop_info);
        }
    }

    // Collect categories with their DOPs for the overview table
    let categories: Vec<(&str, &Vec<DopInfo>)> = [
        ("DTC-DOPS", &dtc_dops),
        ("ENV-DATA-DESCS", &env_data_descs),
        ("DATA-OBJECT-PROPS", &data_object_props),
        ("STRUCTURES", &structures),
        ("STATIC-FIELDS", &static_fields),
        ("DYNAMIC-LENGTH-FIELDS", &dynamic_length_fields),
        ("END-OF-PDU-FIELDS", &end_of_pdu_fields),
        ("MUX-DOPS", &mux_dops),
    ]
    .into_iter()
    .filter(|(_, dops)| !dops.is_empty())
    .collect();

    // Build DOPs overview table
    let dops_detail = build_dops_overview_table(&categories);

    // Add DiagDataDictionarySpec section header (yellow)
    b.push_details_structured(
        depth,
        "DiagDataDictionarySpec".to_owned(),
        false,
        true,
        dops_detail,
        NodeType::DOP,
    );

    // Add category nodes
    for (cat_name, dops) in &categories {
        if !dops.is_empty() {
            let cat_detail = build_category_overview_table(dops);

            b.push_details_structured(
                depth + 1,
                format!("{} ({})", cat_name, dops.len()),
                false,
                true,
                cat_detail,
                NodeType::Default,
            );

            // Add individual DOPs
            for dop_info in *dops {
                let detail_sections = build_dop_detail_sections(dop_info);
                b.push_details_structured(
                    depth + 2,
                    dop_info.name.clone(),
                    false,
                    false,
                    detail_sections,
                    NodeType::Default,
                );
            }
        }
    }
}

/// Push a DOP to the collection and recursively collect nested DOPs
fn collect_single_dop<'a>(
    dop_wrap: cda_database::datatypes::DataOperation<'a>,
    all_dops: &mut Vec<DopInfo<'a>>,
    seen: &mut std::collections::HashSet<String>,
) {
    let Some(name) = dop_wrap.short_name() else { return };
    if !seen.insert(name.to_owned()) { return; }

    let specific_type_raw = format!("{:?}", dop_wrap.specific_data_type());
    let dop_type = specific_type_raw.trim_matches('"').to_owned();

    let (category, internal_unit, phys_unit, desc_id) = extract_dop_metadata(&dop_wrap);

    // Recursively collect nested DOPs (from structures, env data descs, mux, fields, etc.)
    collect_nested_dops(&dop_wrap, all_dops, seen);

    all_dops.push(DopInfo {
        name: name.to_owned(),
        dop_type,
        dop: dop_wrap,
        category,
        internal_unit,
        phys_unit,
        desc_id,
    });
}

/// Extract metadata (category, units, desc_id) from a DataOperation
fn extract_dop_metadata(
    dop_wrap: &cda_database::datatypes::DataOperation<'_>,
) -> (Option<String>, Option<String>, Option<String>, Option<String>) {
    use cda_database::datatypes::DataOperationVariant;
    if let Ok(variant) = dop_wrap.variant() {
        match variant {
            DataOperationVariant::Normal(normal_dop) => {
                let cat = normal_dop.compu_method()
                    .map(|cm| format!("{:?}", cm.category()));
                let int_unit = normal_dop.unit_ref()
                    .and_then(|u| u.short_name())
                    .map(|s| s.to_owned());
                let phys = normal_dop.unit_ref()
                    .and_then(|u| u.display_name())
                    .or_else(|| normal_dop.unit_ref().and_then(|u| u.short_name()))
                    .map(|s| s.to_owned());
                (cat, int_unit, phys, None)
            }
            DataOperationVariant::EnvDataDesc(env_desc) => {
                let did = env_desc.param_short_name().map(|s| s.to_owned());
                (None, None, None, did)
            }
            _ => (None, None, None, None),
        }
    } else {
        (None, None, None, None)
    }
}

/// Recursively collect nested DOPs from compound DOP types.
/// Uses raw FlatBuffer accessors (via .0) to preserve the buffer lifetime 'a.
fn collect_nested_dops<'a>(
    dop_wrap: &cda_database::datatypes::DataOperation<'a>,
    all_dops: &mut Vec<DopInfo<'a>>,
    seen: &mut std::collections::HashSet<String>,
) {
    // Access the inner raw DOP directly to get proper 'a lifetimes
    // (the wrapper's variant() method has restrictive lifetime bounds)
    let raw = dop_wrap.0;

    // Structure: collect DOPs from structure params
    if let Some(structure) = raw.specific_data_as_structure() {
        if let Some(params) = structure.params() {
            for param in params.iter() {
                let pw = Parameter(param);
                collect_dop_from_param(&pw, all_dops, seen);
            }
        }
    }

    // EnvDataDesc: collect nested env_data DOPs
    if let Some(env_desc) = raw.specific_data_as_env_data_desc() {
        if let Some(env_datas) = env_desc.env_datas() {
            for env_dop in env_datas.iter() {
                collect_single_dop(
                    cda_database::datatypes::DataOperation(env_dop),
                    all_dops,
                    seen,
                );
            }
        }
    }

    // EnvData: collect DOPs from env data params
    if let Some(env_data) = raw.specific_data_as_env_data() {
        if let Some(params) = env_data.params() {
            for param in params.iter() {
                let pw = Parameter(param);
                collect_dop_from_param(&pw, all_dops, seen);
            }
        }
    }

    // Mux: switch key DOP, default case structure, case structures
    if let Some(mux_dop) = raw.specific_data_as_muxdop() {
        if let Some(switch_key) = mux_dop.switch_key() {
            if let Some(sk_dop) = switch_key.dop() {
                collect_single_dop(
                    cda_database::datatypes::DataOperation(sk_dop),
                    all_dops,
                    seen,
                );
            }
        }
        if let Some(default_case) = mux_dop.default_case() {
            if let Some(dc_dop) = default_case.structure() {
                collect_single_dop(
                    cda_database::datatypes::DataOperation(dc_dop),
                    all_dops,
                    seen,
                );
            }
        }
        if let Some(cases) = mux_dop.cases() {
            for case in cases.iter() {
                if let Some(c_dop) = case.structure() {
                    collect_single_dop(
                        cda_database::datatypes::DataOperation(c_dop),
                        all_dops,
                        seen,
                    );
                }
            }
        }
    }

    // StaticField: field -> basic_structure + env_data_desc
    if let Some(static_field) = raw.specific_data_as_static_field() {
        if let Some(field) = static_field.field() {
            if let Some(bs) = field.basic_structure() {
                collect_single_dop(
                    cda_database::datatypes::DataOperation(bs),
                    all_dops,
                    seen,
                );
            }
            if let Some(ed) = field.env_data_desc() {
                collect_single_dop(
                    cda_database::datatypes::DataOperation(ed),
                    all_dops,
                    seen,
                );
            }
        }
    }

    // EndOfPdu: field -> basic_structure + env_data_desc
    if let Some(end_of_pdu) = raw.specific_data_as_end_of_pdu_field() {
        if let Some(field) = end_of_pdu.field() {
            if let Some(bs) = field.basic_structure() {
                collect_single_dop(
                    cda_database::datatypes::DataOperation(bs),
                    all_dops,
                    seen,
                );
            }
            if let Some(ed) = field.env_data_desc() {
                collect_single_dop(
                    cda_database::datatypes::DataOperation(ed),
                    all_dops,
                    seen,
                );
            }
        }
    }

    // DynamicLengthField: field + determine_number_of_items
    if let Some(dyn_field) = raw.specific_data_as_dynamic_length_field() {
        if let Some(field) = dyn_field.field() {
            if let Some(bs) = field.basic_structure() {
                collect_single_dop(
                    cda_database::datatypes::DataOperation(bs),
                    all_dops,
                    seen,
                );
            }
            if let Some(ed) = field.env_data_desc() {
                collect_single_dop(
                    cda_database::datatypes::DataOperation(ed),
                    all_dops,
                    seen,
                );
            }
        }
        if let Some(det) = dyn_field.determine_number_of_items() {
            if let Some(det_dop) = det.dop() {
                collect_single_dop(
                    cda_database::datatypes::DataOperation(det_dop),
                    all_dops,
                    seen,
                );
            }
        }
    }
}

/// Extract a DOP from a Parameter wrapper and add it to the collection
fn collect_dop_from_param<'a>(
    param: &Parameter<'a>,
    all_dops: &mut Vec<DopInfo<'a>>,
    seen: &mut std::collections::HashSet<String>,
) {
    use cda_database::datatypes::ParamType;

    let Ok(param_type) = param.param_type() else {
        return;
    };

    // Get the raw DOP from param's specific data depending on type
    let raw_dop = match param_type {
        ParamType::Value => param.specific_data_as_value().and_then(|v| v.dop()),
        ParamType::PhysConst => param.specific_data_as_phys_const().and_then(|v| v.dop()),
        ParamType::LengthKey => param.specific_data_as_length_key_ref().and_then(|v| v.dop()),
        ParamType::System => param.specific_data_as_system().and_then(|v| v.dop()),
        _ => None,
    };

    if let Some(dop) = raw_dop {
        collect_single_dop(cda_database::datatypes::DataOperation(dop), all_dops, seen);
    }
}

/// Collect DOPs from a service's request and response params
fn collect_dops_from_service<'a>(
    ds: &DiagService<'a>,
    all_dops: &mut Vec<DopInfo<'a>>,
    seen: &mut std::collections::HashSet<String>,
) {
    // Collect from request params
    if let Some(request) = ds.request() {
        if let Some(params) = request.params() {
            for param in params.iter() {
                let pw = Parameter(param);
                collect_dop_from_param(&pw, all_dops, seen);
            }
        }
    }

    // Collect from positive response params
    if let Some(pos_responses) = ds.pos_responses() {
        for response in pos_responses.iter() {
            if let Some(params) = response.params() {
                for param in params.iter() {
                    let pw = Parameter(param);
                    collect_dop_from_param(&pw, all_dops, seen);
                }
            }
        }
    }

    // Collect from negative response params
    if let Some(neg_responses) = ds.neg_responses() {
        for response in neg_responses.iter() {
            if let Some(params) = response.params() {
                for param in params.iter() {
                    let pw = Parameter(param);
                    collect_dop_from_param(&pw, all_dops, seen);
                }
            }
        }
    }
}


/// Build overview table showing semantic categories and their counts
fn build_dops_overview_table(categories: &[(&str, &Vec<DopInfo<'_>>)]) -> Vec<DetailSectionData> {
    let header = DetailRow {
        cells: vec![
            "Category".to_owned(),
            "Count".to_owned(),
        ],
        cell_types: vec![CellType::Text, CellType::NumericValue],
        indent: 0,
        ..Default::default()
    };

    let mut rows: Vec<DetailRow> = Vec::new();

    for (cat_name, dops) in categories {
        rows.push(DetailRow {
            cells: vec![
                cat_name.to_string(),
                dops.len().to_string(),
            ],
            cell_types: vec![CellType::Text, CellType::NumericValue],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    vec![DetailSectionData {
        title: "Overview".to_owned(),
        render_as_header: false,
        section_type: DetailSectionType::Overview,
        content: DetailContent::Table {
            header,
            rows,
            constraints: vec![
                ColumnConstraint::Percentage(70),
                ColumnConstraint::Percentage(30),
            ],
            use_row_selection: true,
        },
    }]
}

/// Build overview table for a semantic DOP category
fn build_category_overview_table(dops: &[DopInfo<'_>]) -> Vec<DetailSectionData> {
    let header = DetailRow {
        cells: vec![
            "SHORT-NAME".to_owned(),
            "CATEGORY".to_owned(),
            "Internal".to_owned(),
            "Physical".to_owned(),
            "DESC ID".to_owned(),
        ],
        cell_types: vec![CellType::Text, CellType::Text, CellType::Text, CellType::Text, CellType::Text],
        indent: 0,
        ..Default::default()
    };

    let mut rows: Vec<DetailRow> = Vec::new();

    for dop_info in dops.iter() {
        rows.push(DetailRow {
            cells: vec![
                dop_info.name.clone(),
                dop_info.category.as_deref().unwrap_or("").to_owned(),
                dop_info.internal_unit.as_deref().unwrap_or("").to_owned(),
                dop_info.phys_unit.as_deref().unwrap_or("").to_owned(),
                dop_info.desc_id.as_deref().unwrap_or("").to_owned(),
            ],
            cell_types: vec![CellType::Text, CellType::Text, CellType::Text, CellType::Text, CellType::Text],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    vec![DetailSectionData {
        title: "Overview".to_owned(),
        render_as_header: false,
        section_type: DetailSectionType::Overview,
        content: DetailContent::Table {
            header,
            rows,
            constraints: vec![
                ColumnConstraint::Percentage(25),
                ColumnConstraint::Percentage(20),
                ColumnConstraint::Percentage(20),
                ColumnConstraint::Percentage(20),
                ColumnConstraint::Percentage(15),
            ],
            use_row_selection: true,
        },
    }]
}

/// Helper to push a "Types" tab section from accumulated rows
fn push_types_section(types_rows: Vec<DetailRow>, sections: &mut Vec<DetailSectionData>) {
    if !types_rows.is_empty() {
        let header = DetailRow {
            cells: vec!["Property".to_owned(), "Value".to_owned()],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
            ..Default::default()
        };
        sections.insert(0, DetailSectionData {
            title: "Types".to_owned(),
            render_as_header: false,
            section_type: DetailSectionType::Custom,
            content: DetailContent::Table {
                header,
                rows: types_rows,
                constraints: vec![
                    ColumnConstraint::Percentage(40),
                    ColumnConstraint::Percentage(60),
                ],
                use_row_selection: true,
            },
        });
    }
}

/// Build detail sections for a single DOP with full type-specific information
fn build_dop_detail_sections(dop_info: &DopInfo<'_>) -> Vec<DetailSectionData> {
    let mut sections = Vec::new();

    // Parse name to extract encoded information
    let parsed_name = parse_dop_name(&dop_info.name);

    // Build base "Types" rows (formerly Overview data)
    let mut types_rows = Vec::new();
    types_rows.push(DetailRow {
        cells: vec!["Short Name".to_owned(), dop_info.name.clone()],
        cell_types: vec![CellType::Text, CellType::Text],
        indent: 0,
        row_type: DetailRowType::Normal,
        metadata: None,
    });
    types_rows.push(DetailRow {
        cells: vec!["DOP Variant".to_owned(), dop_info.dop_type.clone()],
        cell_types: vec![CellType::Text, CellType::Text],
        indent: 0,
        row_type: DetailRowType::Normal,
        metadata: None,
    });

    if let Some(ref compu) = parsed_name.compu_category {
        types_rows.push(DetailRow {
            cells: vec!["Compu Category (from name)".to_owned(), compu.clone()],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }
    if let Some(ref unit) = parsed_name.unit {
        types_rows.push(DetailRow {
            cells: vec!["Unit (from name)".to_owned(), unit.clone()],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    // Extract type-specific details based on DataOperationVariant
    // Each variant builder appends its rows to types_rows, then pushes the "Types" section
    use cda_database::datatypes::DataOperationVariant;
    if let Ok(variant) = dop_info.dop.variant() {
        match variant {
            DataOperationVariant::Normal(normal_dop) => {
                build_normal_dop_tabs(&normal_dop, &parsed_name, &mut types_rows, &mut sections);
            }
            DataOperationVariant::Structure(structure) => {
                build_structure_dop_tabs(&structure, &mut types_rows, &mut sections);
            }
            DataOperationVariant::StaticField(static_field) => {
                build_static_field_dop_tabs(&static_field, &mut types_rows, &mut sections);
            }
            DataOperationVariant::EndOfPdu(eof_field) => {
                build_end_of_pdu_dop_tabs(&eof_field, &mut types_rows, &mut sections);
            }
            DataOperationVariant::DynamicLengthField(dyn_field) => {
                build_dynamic_length_field_dop_tabs(&dyn_field, &mut types_rows, &mut sections);
            }
            DataOperationVariant::EnvDataDesc(env_desc) => {
                build_env_data_desc_dop_tabs(&env_desc, &mut types_rows, &mut sections);
            }
            DataOperationVariant::EnvData(env_data) => {
                build_env_data_dop_tabs(&env_data, &mut types_rows, &mut sections);
            }
            DataOperationVariant::Mux(mux_dop) => {
                build_mux_dop_tabs(&mux_dop, &mut types_rows, &mut sections);
            }
            DataOperationVariant::Dtc(dtc_dop) => {
                build_dtc_dop_tabs(&dtc_dop, &mut types_rows, &mut sections);
            }
        }
    } else {
        // No variant data - just push base types
        push_types_section(types_rows, &mut sections);
    }

    sections
}

/// Build tabbed sections for NormalDOP with Types, Constraints, and Compu tabs
fn build_normal_dop_tabs(
    normal_dop: &cda_database::datatypes::NormalDop<'_>,
    parsed_name: &ParsedDopName,
    types_rows: &mut Vec<DetailRow>,
    sections: &mut Vec<DetailSectionData>,
) {
    // Append type-specific rows to the shared types_rows (merges with Overview data)
    if let Ok(coded_type) = normal_dop.diag_coded_type() {
        types_rows.push(DetailRow {
            cells: vec!["Diag Coded Type".to_owned(), format!("{:?}", coded_type.base_datatype())],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });

        if let Some(bit_len) = coded_type.bit_len() {
            types_rows.push(DetailRow {
                cells: vec!["Bit Length".to_owned(), bit_len.to_string()],
                cell_types: vec![CellType::Text, CellType::NumericValue],
                indent: 0,
                row_type: DetailRowType::Normal,
                metadata: None,
            });
        }
    }

    if let Some(ref data_type) = parsed_name.data_type {
        types_rows.push(DetailRow {
            cells: vec!["Data Type (from name)".to_owned(), data_type.clone()],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    // Push the merged "Types" section
    push_types_section(std::mem::take(types_rows), sections);

    // Tab 2: Internal Constraints (always shown)
    let mut constraints_rows = Vec::new();

    if let Some(ref min) = parsed_name.range_min {
        constraints_rows.push(DetailRow {
            cells: vec!["Range Min (from name)".to_owned(), min.clone()],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    if let Some(ref max) = parsed_name.range_max {
        constraints_rows.push(DetailRow {
            cells: vec!["Range Max (from name)".to_owned(), max.clone()],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    let constraints_header = DetailRow {
        cells: vec!["Property".to_owned(), "Value".to_owned()],
        cell_types: vec![CellType::Text, CellType::Text],
        indent: 0,
        ..Default::default()
    };

    sections.push(DetailSectionData {
        title: "Internal-Constr".to_owned(),
        render_as_header: false,
        section_type: DetailSectionType::Custom,
        content: DetailContent::Table {
            header: constraints_header,
            rows: constraints_rows,
            constraints: vec![
                ColumnConstraint::Percentage(40),
                ColumnConstraint::Percentage(60),
            ],
            use_row_selection: true,
        },
    });

    // Tab 3: Compu-Internal-To-Phys (always shown)
    let mut compu_i2p_rows = Vec::new();

    if let Some(compu_method) = normal_dop.compu_method() {
        compu_i2p_rows.push(DetailRow {
            cells: vec!["Category".to_owned(), format!("{:?}", compu_method.category())],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });

        if let Some(internal_to_phys) = compu_method.internal_to_phys() {
            if let Some(scales) = internal_to_phys.compu_scales() {
                compu_i2p_rows.push(DetailRow {
                    cells: vec!["Scales Count".to_owned(), scales.len().to_string()],
                    cell_types: vec![CellType::Text, CellType::NumericValue],
                    indent: 0,
                    row_type: DetailRowType::Normal,
                    metadata: None,
                });
            }
        }
    }

    let compu_i2p_header = DetailRow {
        cells: vec!["Property".to_owned(), "Value".to_owned()],
        cell_types: vec![CellType::Text, CellType::Text],
        indent: 0,
        ..Default::default()
    };

    sections.push(DetailSectionData {
        title: "Compu-Internal-To-Phys".to_owned(),
        render_as_header: false,
        section_type: DetailSectionType::Custom,
        content: DetailContent::Table {
            header: compu_i2p_header,
            rows: compu_i2p_rows,
            constraints: vec![
                ColumnConstraint::Percentage(40),
                ColumnConstraint::Percentage(60),
            ],
            use_row_selection: true,
        },
    });

    // Tab 4: Compu-Phys-To-Internal (always shown)
    let mut compu_p2i_rows = Vec::new();

    if let Some(compu_method) = normal_dop.compu_method() {
        if let Some(phys_to_internal) = compu_method.phys_to_internal() {
            if let Some(scales) = phys_to_internal.compu_scales() {
                compu_p2i_rows.push(DetailRow {
                    cells: vec!["Scales Count".to_owned(), scales.len().to_string()],
                    cell_types: vec![CellType::Text, CellType::NumericValue],
                    indent: 0,
                    row_type: DetailRowType::Normal,
                    metadata: None,
                });
            }
        }
    }

    let compu_p2i_header = DetailRow {
        cells: vec!["Property".to_owned(), "Value".to_owned()],
        cell_types: vec![CellType::Text, CellType::Text],
        indent: 0,
        ..Default::default()
    };

    sections.push(DetailSectionData {
        title: "Compu-Phys-To-Internal".to_owned(),
        render_as_header: false,
        section_type: DetailSectionType::Custom,
        content: DetailContent::Table {
            header: compu_p2i_header,
            rows: compu_p2i_rows,
            constraints: vec![
                ColumnConstraint::Percentage(40),
                ColumnConstraint::Percentage(60),
            ],
            use_row_selection: true,
        },
    });
}

/// Build tabbed sections for Structure DOP
fn build_structure_dop_tabs(
    structure: &cda_database::datatypes::StructureDop<'_>,
    types_rows: &mut Vec<DetailRow>,
    sections: &mut Vec<DetailSectionData>,
) {
    if let Some(byte_size) = structure.byte_size() {
        types_rows.push(DetailRow {
            cells: vec!["Byte Size".to_owned(), byte_size.to_string()],
            cell_types: vec![CellType::Text, CellType::NumericValue],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    types_rows.push(DetailRow {
        cells: vec!["Is Visible".to_owned(), structure.is_visible().to_string()],
        cell_types: vec![CellType::Text, CellType::Text],
        indent: 0,
        row_type: DetailRowType::Normal,
        metadata: None,
    });

    if let Some(params) = structure.params() {
        types_rows.push(DetailRow {
            cells: vec!["Param Count".to_owned(), params.len().to_string()],
            cell_types: vec![CellType::Text, CellType::NumericValue],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    push_types_section(std::mem::take(types_rows), sections);
}

/// Build tabbed sections for StaticField DOP
fn build_static_field_dop_tabs(
    static_field: &cda_database::datatypes::StaticFieldDop<'_>,
    types_rows: &mut Vec<DetailRow>,
    sections: &mut Vec<DetailSectionData>,
) {
    let fixed_items = static_field.fixed_number_of_items();
    types_rows.push(DetailRow {
        cells: vec!["Fixed Number of Items".to_owned(), fixed_items.to_string()],
        cell_types: vec![CellType::Text, CellType::NumericValue],
        indent: 0,
        row_type: DetailRowType::Normal,
        metadata: None,
    });

    let item_size = static_field.item_byte_size();
    types_rows.push(DetailRow {
        cells: vec!["Item Byte Size".to_owned(), item_size.to_string()],
        cell_types: vec![CellType::Text, CellType::NumericValue],
        indent: 0,
        row_type: DetailRowType::Normal,
        metadata: None,
    });

    push_types_section(std::mem::take(types_rows), sections);
}

/// Build tabbed sections for EndOfPdu DOP
fn build_end_of_pdu_dop_tabs(
    eof_field: &cda_database::datatypes::EndOfPdu<'_>,
    types_rows: &mut Vec<DetailRow>,
    sections: &mut Vec<DetailSectionData>,
) {
    if let Some(max_items) = eof_field.max_number_of_items() {
        types_rows.push(DetailRow {
            cells: vec!["Max Number of Items".to_owned(), max_items.to_string()],
            cell_types: vec![CellType::Text, CellType::NumericValue],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    if let Some(min_items) = eof_field.min_number_of_items() {
        types_rows.push(DetailRow {
            cells: vec!["Min Number of Items".to_owned(), min_items.to_string()],
            cell_types: vec![CellType::Text, CellType::NumericValue],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    push_types_section(std::mem::take(types_rows), sections);
}

/// Build tabbed sections for DynamicLengthField DOP
fn build_dynamic_length_field_dop_tabs(
    dyn_field: &cda_database::datatypes::DynamicLengthDop<'_>,
    types_rows: &mut Vec<DetailRow>,
    sections: &mut Vec<DetailSectionData>,
) {
    let offset = dyn_field.offset();
    types_rows.push(DetailRow {
        cells: vec!["Offset".to_owned(), offset.to_string()],
        cell_types: vec![CellType::Text, CellType::NumericValue],
        indent: 0,
        row_type: DetailRowType::Normal,
        metadata: None,
    });

    push_types_section(std::mem::take(types_rows), sections);
}

/// Build tabbed sections for EnvDataDesc DOP
fn build_env_data_desc_dop_tabs(
    env_desc: &cda_database::datatypes::EnvDataDescDop<'_>,
    types_rows: &mut Vec<DetailRow>,
    sections: &mut Vec<DetailSectionData>,
) {
    if let Some(param_name) = env_desc.param_short_name() {
        types_rows.push(DetailRow {
            cells: vec!["Param Short Name".to_owned(), param_name.to_owned()],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    if let Some(param_path) = env_desc.param_path_short_name() {
        types_rows.push(DetailRow {
            cells: vec!["Param Path Short Name".to_owned(), param_path.to_owned()],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    if let Some(env_datas) = env_desc.env_datas() {
        types_rows.push(DetailRow {
            cells: vec!["Env Data Count".to_owned(), env_datas.len().to_string()],
            cell_types: vec![CellType::Text, CellType::NumericValue],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    push_types_section(std::mem::take(types_rows), sections);
}

/// Build tabbed sections for EnvData DOP
fn build_env_data_dop_tabs(
    env_data: &cda_database::datatypes::EnvDataDop<'_>,
    types_rows: &mut Vec<DetailRow>,
    sections: &mut Vec<DetailSectionData>,
) {
    if let Some(dtc_values) = env_data.dtc_values() {
        types_rows.push(DetailRow {
            cells: vec!["DTC Values Count".to_owned(), dtc_values.len().to_string()],
            cell_types: vec![CellType::Text, CellType::NumericValue],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    if let Some(params) = env_data.params() {
        types_rows.push(DetailRow {
            cells: vec!["Param Count".to_owned(), params.len().to_string()],
            cell_types: vec![CellType::Text, CellType::NumericValue],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    push_types_section(std::mem::take(types_rows), sections);
}

/// Build tabbed sections for MUXDOP
fn build_mux_dop_tabs(
    mux_dop: &cda_database::datatypes::MuxDop<'_>,
    types_rows: &mut Vec<DetailRow>,
    sections: &mut Vec<DetailSectionData>,
) {
    let byte_pos = mux_dop.byte_position();
    types_rows.push(DetailRow {
        cells: vec!["Byte Position".to_owned(), byte_pos.to_string()],
        cell_types: vec![CellType::Text, CellType::NumericValue],
        indent: 0,
        row_type: DetailRowType::Normal,
        metadata: None,
    });

    types_rows.push(DetailRow {
        cells: vec!["Is Visible".to_owned(), mux_dop.is_visible().to_string()],
        cell_types: vec![CellType::Text, CellType::Text],
        indent: 0,
        row_type: DetailRowType::Normal,
        metadata: None,
    });

    if let Some(cases) = mux_dop.cases() {
        types_rows.push(DetailRow {
            cells: vec!["Case Count".to_owned(), cases.len().to_string()],
            cell_types: vec![CellType::Text, CellType::NumericValue],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    push_types_section(std::mem::take(types_rows), sections);
}

/// Build tabbed sections for DTCDOP
fn build_dtc_dop_tabs(
    dtc_dop: &cda_database::datatypes::DtcDop<'_>,
    types_rows: &mut Vec<DetailRow>,
    sections: &mut Vec<DetailSectionData>,
) {
    if let Ok(coded_type) = dtc_dop.diag_coded_type() {
        types_rows.push(DetailRow {
            cells: vec!["Diag Coded Type".to_owned(), format!("{:?}", coded_type.base_datatype())],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    if let Some(compu_method) = dtc_dop.compu_method() {
        types_rows.push(DetailRow {
            cells: vec!["Compu Category".to_owned(), format!("{:?}", compu_method.category())],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    if let Some(dtcs) = dtc_dop.dtcs() {
        types_rows.push(DetailRow {
            cells: vec!["DTC Count".to_owned(), dtcs.len().to_string()],
            cell_types: vec![CellType::Text, CellType::NumericValue],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    push_types_section(std::mem::take(types_rows), sections);
}
