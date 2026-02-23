use cda_database::datatypes::{DiagLayer, DiagService, Parameter};

use crate::tree::{
    builder::TreeBuilder,
    types::{
        CellType, ColumnConstraint, DetailContent, DetailRow, DetailRowType, DetailSectionData,
        DetailSectionType, NodeType,
    },
};

/// Semantic category of a group of DOPs, derived from the DataOperationVariant
#[derive(Clone, Copy, PartialEq, Eq)]
enum DopCategory {
    DtcDops,
    EnvDataDescs,
    EnvDatas,
    DataObjectProps,
    Structures,
    StaticFields,
    DynamicLengthFields,
    EndOfPduFields,
    MuxDops,
}

impl DopCategory {
    fn label(&self) -> &'static str {
        match self {
            Self::DtcDops => "DTC-DOPS",
            Self::EnvDataDescs => "ENV-DATA-DESCS",
            Self::EnvDatas => "ENV-DATAS",
            Self::DataObjectProps => "DATA-OBJECT-PROPS",
            Self::Structures => "STRUCTURES",
            Self::StaticFields => "STATIC-FIELDS",
            Self::DynamicLengthFields => "DYNAMIC-LENGTH-FIELDS",
            Self::EndOfPduFields => "END-OF-PDU-FIELDS",
            Self::MuxDops => "MUX-DOPS",
        }
    }

    /// Build detail sections appropriate for this category
    fn build_detail_sections(&self, dops: &[DopInfo<'_>]) -> Vec<DetailSectionData> {
        match self {
            Self::DtcDops => build_dtc_dops_category_sections(dops),
            _ => build_category_overview_table(dops),
        }
    }

    /// Whether individual DOPs in this category have tree children
    fn has_dop_children(&self) -> bool {
        matches!(self, Self::DtcDops)
    }

    /// Add child tree nodes for an individual DOP in this category
    fn add_dop_children(&self, b: &mut TreeBuilder, dop_info: &DopInfo<'_>, depth: usize) {
        if self == &Self::DtcDops {
            add_dtc_dop_children(b, dop_info, depth);
        }
    }
}

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
    let compu_categories = [
        "IDENTICAL",
        "LINEAR",
        "TEXTTABLE",
        "SCALE",
        "COMPUCODE",
        "TABINTP",
        "RATFUNC",
    ];
    if compu_categories
        .iter()
        .any(|&cat| parts[0].starts_with(cat))
    {
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
    let hex_parts: Vec<&str> = parts
        .iter()
        .filter(|p| p.starts_with("0x") || p.starts_with("0X"))
        .copied()
        .collect();
    if hex_parts.len() >= 2 {
        parsed.range_min = Some(hex_parts[0].to_owned());
        parsed.range_max = Some(hex_parts[1].to_owned());
    } else if hex_parts.len() == 1 {
        parsed.range_min = Some(hex_parts[0].to_owned());
    }

    // Last part might be unit (if not a hex value or data type)
    if let Some(last) = parts.last()
        && !last.starts_with("0x")
        && !last.starts_with("0X")
        && !last.starts_with("A_")
    {
        // Common unit patterns
        let units = [
            "Second",
            "MicroSecond",
            "MilliSecond",
            "Meter",
            "KiloMeter",
            "Volt",
            "Ampere",
            "Celsius",
            "Pascal",
        ];
        if units.iter().any(|&u| last.contains(u)) {
            parsed.unit = Some(last.to_string());
        }
    }

    parsed
}

/// Add DOPs section to the tree by collecting from service/job request/response params
pub fn add_dops_section<'a>(b: &mut TreeBuilder, layer: &DiagLayer<'a>, depth: usize) {
    let mut all_dops: Vec<DopInfo<'a>> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Collect DOPs from own layer's services and jobs only
    collect_dops_from_layer(layer, &mut all_dops, &mut seen);

    // DOPs are already deduplicated via the seen set
    let unique_dops = all_dops;

    if unique_dops.is_empty() {
        return;
    }

    // Group DOPs by semantic category using the actual DataOperationVariant
    use cda_database::datatypes::DataOperationVariant;
    let mut dtc_dops: Vec<DopInfo> = Vec::new();
    let mut env_data_descs: Vec<DopInfo> = Vec::new();
    let mut env_datas: Vec<DopInfo> = Vec::new();
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
                DataOperationVariant::EnvDataDesc(_) => env_data_descs.push(dop_info),
                DataOperationVariant::EnvData(_) => env_datas.push(dop_info),
                DataOperationVariant::Structure(_) => structures.push(dop_info),
                DataOperationVariant::StaticField(_) => static_fields.push(dop_info),
                DataOperationVariant::DynamicLengthField(_) => dynamic_length_fields.push(dop_info),
                DataOperationVariant::EndOfPdu(_) => end_of_pdu_fields.push(dop_info),
                DataOperationVariant::Mux(_) => mux_dops.push(dop_info),
                DataOperationVariant::Normal(_) => data_object_props.push(dop_info),
            }
        } else {
            data_object_props.push(dop_info);
        }
    }

    // Collect categories with their DOPs for the overview table
    let categories: Vec<(DopCategory, &Vec<DopInfo>)> = [
        (DopCategory::DtcDops, &dtc_dops),
        (DopCategory::EnvDataDescs, &env_data_descs),
        (DopCategory::EnvDatas, &env_datas),
        (DopCategory::DataObjectProps, &data_object_props),
        (DopCategory::Structures, &structures),
        (DopCategory::StaticFields, &static_fields),
        (DopCategory::DynamicLengthFields, &dynamic_length_fields),
        (DopCategory::EndOfPduFields, &end_of_pdu_fields),
        (DopCategory::MuxDops, &mux_dops),
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
    for (cat, dops) in &categories {
        if !dops.is_empty() {
            let cat_detail = cat.build_detail_sections(dops);

            b.push_details_structured(
                depth + 1,
                format!("{} ({})", cat.label(), dops.len()),
                false,
                true,
                cat_detail,
                NodeType::Default,
            );

            // Add individual DOPs
            let expandable = cat.has_dop_children();
            for dop_info in *dops {
                let detail_sections = build_dop_detail_sections(dop_info);
                b.push_details_structured(
                    depth + 2,
                    dop_info.name.clone(),
                    false,
                    expandable,
                    detail_sections,
                    NodeType::Default,
                );

                if expandable {
                    cat.add_dop_children(b, dop_info, depth + 3);
                }
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
    let Some(name) = dop_wrap.short_name() else {
        return;
    };
    if !seen.insert(name.to_owned()) {
        return;
    }

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
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    use cda_database::datatypes::DataOperationVariant;
    let Ok(variant) = dop_wrap.variant() else {
        return (None, None, None, None);
    };
    match variant {
        DataOperationVariant::Normal(normal_dop) => {
            let cat = normal_dop
                .compu_method()
                .map(|cm| format!("{:?}", cm.category()));
            let int_unit = normal_dop
                .unit_ref()
                .and_then(|u| u.short_name())
                .map(|s| s.to_owned());
            let phys = normal_dop
                .unit_ref()
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

    // Helper macro: collect DOPs from a field's basic_structure + env_data_desc
    macro_rules! collect_field_dops {
        ($field:expr) => {
            $field
                .basic_structure()
                .into_iter()
                .chain($field.env_data_desc())
                .for_each(|d| {
                    collect_single_dop(cda_database::datatypes::DataOperation(d), all_dops, seen)
                })
        };
    }

    // Structure: collect DOPs from structure params
    raw.specific_data_as_structure()
        .and_then(|s| s.params())
        .into_iter()
        .flat_map(|p| p.iter())
        .for_each(|param| collect_dop_from_param(&Parameter(param), all_dops, seen));

    // EnvDataDesc: collect nested env_data DOPs
    raw.specific_data_as_env_data_desc()
        .and_then(|ed| ed.env_datas())
        .into_iter()
        .flat_map(|v| v.iter())
        .for_each(|env_dop| {
            collect_single_dop(
                cda_database::datatypes::DataOperation(env_dop),
                all_dops,
                seen,
            )
        });

    // EnvData: collect DOPs from env data params
    raw.specific_data_as_env_data()
        .and_then(|ed| ed.params())
        .into_iter()
        .flat_map(|p| p.iter())
        .for_each(|param| collect_dop_from_param(&Parameter(param), all_dops, seen));

    // Mux: switch key DOP, default case structure, case structures
    if let Some(mux_dop) = raw.specific_data_as_muxdop() {
        mux_dop
            .switch_key()
            .and_then(|sk| sk.dop())
            .into_iter()
            .chain(mux_dop.default_case().and_then(|dc| dc.structure()))
            .for_each(|d| {
                collect_single_dop(cda_database::datatypes::DataOperation(d), all_dops, seen)
            });

        mux_dop
            .cases()
            .into_iter()
            .flat_map(|c| c.iter())
            .filter_map(|case| case.structure())
            .for_each(|d| {
                collect_single_dop(cda_database::datatypes::DataOperation(d), all_dops, seen)
            });
    }

    // StaticField / EndOfPdu / DynamicLengthField: field -> basic_structure + env_data_desc
    [
        raw.specific_data_as_static_field()
            .and_then(|sf| sf.field()),
        raw.specific_data_as_end_of_pdu_field()
            .and_then(|ep| ep.field()),
        raw.specific_data_as_dynamic_length_field()
            .and_then(|df| df.field()),
    ]
    .into_iter()
    .flatten()
    .for_each(|field| collect_field_dops!(field));

    // DynamicLengthField: also has determine_number_of_items -> dop
    raw.specific_data_as_dynamic_length_field()
        .and_then(|df| df.determine_number_of_items())
        .and_then(|det| det.dop())
        .into_iter()
        .for_each(|d| {
            collect_single_dop(cda_database::datatypes::DataOperation(d), all_dops, seen)
        });
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

    // Helper macro: collect dop + structure from a table row
    macro_rules! collect_table_row {
        ($row:expr) => {
            $row.dop()
                .into_iter()
                .chain($row.structure())
                .for_each(|d| {
                    collect_single_dop(cda_database::datatypes::DataOperation(d), all_dops, seen)
                })
        };
    }

    // Get the raw DOP from param's specific data depending on type
    match param_type {
        ParamType::Value => param.specific_data_as_value().and_then(|v| v.dop()),
        ParamType::PhysConst => param.specific_data_as_phys_const().and_then(|v| v.dop()),
        ParamType::LengthKey => param
            .specific_data_as_length_key_ref()
            .and_then(|v| v.dop()),
        ParamType::System => param.specific_data_as_system().and_then(|v| v.dop()),
        _ => None,
    }
    .into_iter()
    .for_each(|dop| {
        collect_single_dop(cda_database::datatypes::DataOperation(dop), all_dops, seen)
    });

    // TableKey: collect DOPs from referenced TableDop (key_dop + row dops/structures)
    // or from a direct TableRow reference
    if matches!(param_type, ParamType::TableKey) {
        let tk = param.0.specific_data_as_table_key();

        tk.and_then(|tk| tk.table_key_reference_as_table_dop())
            .into_iter()
            .for_each(|table_dop| {
                table_dop.key_dop().into_iter().for_each(|kd| {
                    collect_single_dop(cda_database::datatypes::DataOperation(kd), all_dops, seen)
                });

                table_dop
                    .rows()
                    .into_iter()
                    .flat_map(|rows| rows.iter())
                    .for_each(|row| collect_table_row!(row));
            });

        tk.and_then(|tk| tk.table_key_reference_as_table_row())
            .into_iter()
            .for_each(|row| collect_table_row!(row));
    }

    // TableEntry: collect DOPs from the referenced TableRow
    if matches!(param_type, ParamType::TableEntry) {
        param
            .0
            .specific_data_as_table_entry()
            .and_then(|te| te.table_row())
            .into_iter()
            .for_each(|row| collect_table_row!(row));
    }

    // TableStruct: recurse into the table_key param
    if matches!(param_type, ParamType::TableStruct) {
        param
            .0
            .specific_data_as_table_struct()
            .and_then(|ts| ts.table_key())
            .into_iter()
            .for_each(|tk_param| collect_dop_from_param(&Parameter(tk_param), all_dops, seen));
    }
}

/// Collect all DOPs from a single DiagLayer (own services + single ECU jobs)
fn collect_dops_from_layer<'a>(
    layer: &DiagLayer<'a>,
    all_dops: &mut Vec<DopInfo<'a>>,
    seen: &mut std::collections::HashSet<String>,
) {
    // Collect from diag services (requests + responses)
    layer
        .diag_services()
        .into_iter()
        .flat_map(|s| s.iter())
        .for_each(|svc| collect_dops_from_service(&DiagService(svc), all_dops, seen));

    // Collect from single ECU jobs (input/output/neg_output params)
    layer
        .single_ecu_jobs()
        .into_iter()
        .flat_map(|j| j.iter())
        .for_each(|job| {
            [
                job.input_params(),
                job.output_params(),
                job.neg_output_params(),
            ]
            .into_iter()
            .flatten()
            .flat_map(|params| params.iter())
            .filter_map(|jp| jp.dop_base())
            .for_each(|dop| {
                collect_single_dop(cda_database::datatypes::DataOperation(dop), all_dops, seen)
            });
        });
}

/// Collect DOPs from a service's request and response params
fn collect_dops_from_service<'a>(
    ds: &DiagService<'a>,
    all_dops: &mut Vec<DopInfo<'a>>,
    seen: &mut std::collections::HashSet<String>,
) {
    macro_rules! collect_params {
        ($params:expr) => {
            $params
                .iter()
                .for_each(|param| collect_dop_from_param(&Parameter(param), all_dops, seen))
        };
    }

    // Collect from request params
    ds.request()
        .and_then(|r| r.params())
        .into_iter()
        .for_each(|params| collect_params!(params));

    // Collect from positive response params
    ds.pos_responses()
        .into_iter()
        .flat_map(|r| r.iter())
        .filter_map(|resp| resp.params())
        .for_each(|params| collect_params!(params));

    // Collect from negative response params
    ds.neg_responses()
        .into_iter()
        .flat_map(|r| r.iter())
        .filter_map(|resp| resp.params())
        .for_each(|params| collect_params!(params));
}

/// Build overview table showing semantic categories and their counts
fn build_dops_overview_table(
    categories: &[(DopCategory, &Vec<DopInfo<'_>>)],
) -> Vec<DetailSectionData> {
    let header = DetailRow {
        cells: vec!["Category".to_owned(), "Count".to_owned()],
        cell_types: vec![CellType::Text, CellType::NumericValue],
        indent: 0,
        ..Default::default()
    };

    let mut rows: Vec<DetailRow> = Vec::new();

    for (cat, dops) in categories {
        rows.push(DetailRow {
            cells: vec![cat.label().to_string(), dops.len().to_string()],
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

/// Build sections for the DTC-DOPS category node.
/// Overview with only SHORT-NAME column.
fn build_dtc_dops_category_sections(dops: &[DopInfo<'_>]) -> Vec<DetailSectionData> {
    let header = DetailRow {
        cells: vec!["SHORT-NAME".to_owned()],
        cell_types: vec![CellType::Text],
        indent: 0,
        ..Default::default()
    };

    let rows: Vec<DetailRow> = dops
        .iter()
        .map(|dop_info| DetailRow {
            cells: vec![dop_info.name.clone()],
            cell_types: vec![CellType::Text],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        })
        .collect();

    vec![DetailSectionData {
        title: "Overview".to_owned(),
        render_as_header: false,
        section_type: DetailSectionType::Overview,
        content: DetailContent::Table {
            header,
            rows,
            constraints: vec![ColumnConstraint::Percentage(100)],
            use_row_selection: true,
        },
    }]
}

// ── SDG helpers ──────────────────────────────────────────────────────────────

/// Convenience: build a key/value detail row.
fn kv_row(key: &str, value: String, value_type: CellType, indent: usize) -> DetailRow {
    DetailRow::normal(
        vec![key.to_owned(), value],
        vec![CellType::Text, value_type],
        indent,
    )
}

/// Pick a label for a raw SD entry: prefer si, then ti, then a numbered fallback.
fn sd_entry_label(
    si: &str,
    ti: &str,
    caption: &str,
    unnamed_idx: &mut usize,
    unnamed_count: usize,
) -> String {
    if !si.is_empty() {
        si.to_owned()
    } else if !ti.is_empty() {
        ti.to_owned()
    } else if unnamed_count > 1 {
        *unnamed_idx += 1;
        format!("{} [{}]", caption, unnamed_idx)
    } else {
        caption.to_owned()
    }
}

/// Helper: count unnamed SD entries and detect named/nested entries in an SDG's
/// `sds()` list. Returns `(has_named_or_nested, unnamed_count)`.
///
/// Expressed as a macro because the raw flatbuf iterator types are `pub(crate)`.
macro_rules! sdg_entry_stats {
    ($sds:expr) => {{
        let has_named_or_nested = $sds.iter().any(|e| {
            e.sd_or_sdg_as_sd()
                .map(|sd| !sd.si().unwrap_or("").is_empty() || !sd.ti().unwrap_or("").is_empty())
                .unwrap_or(true) // nested SDG ⇒ treat as named
        });
        let unnamed_count = $sds
            .iter()
            .filter(|e| {
                e.sd_or_sdg_as_sd()
                    .map(|sd| sd.si().unwrap_or("").is_empty() && sd.ti().unwrap_or("").is_empty())
                    .unwrap_or(false)
            })
            .count();
        (has_named_or_nested, unnamed_count)
    }};
}

/// Helper: emit an SDG header row + optional SI row when the group has named
/// or nested children.  Returns the indent level for child rows.
macro_rules! emit_sdg_header {
    ($rows:expr, $sdg:expr, $caption:expr, $has_named:expr, $base_indent:expr) => {{
        if $has_named {
            $rows.push(DetailRow {
                cells: vec![$caption.clone(), String::new()],
                cell_types: vec![CellType::Text, CellType::Text],
                indent: $base_indent,
                row_type: DetailRowType::Header,
                metadata: None,
            });
            if let Some(si) = $sdg.si() {
                $rows.push(kv_row(
                    "SI",
                    si.to_owned(),
                    CellType::Text,
                    $base_indent + 1,
                ));
            }
            $base_indent + 1
        } else {
            $base_indent
        }
    }};
}

/// Helper: emit SD value rows from an SDG's `sds()` list.
macro_rules! emit_sd_rows {
    ($rows:expr, $sds:expr, $caption:expr, $unnamed_count:expr, $indent:expr) => {{
        let mut unnamed_idx = 0usize;
        for entry in $sds.iter() {
            if let Some(sd) = entry.sd_or_sdg_as_sd() {
                let label = sd_entry_label(
                    sd.si().unwrap_or(""),
                    sd.ti().unwrap_or(""),
                    &$caption,
                    &mut unnamed_idx,
                    $unnamed_count,
                );
                let value = sd.value().unwrap_or("").to_owned();
                $rows.push(kv_row(&label, value, CellType::Text, $indent));
            }
        }
    }};
}

/// Append flattened SDG rows (up to two nesting levels) into `rows`.
///
/// The raw flatbuf `SD` / `SDG` types are `pub(crate)` in `cda_database` and
/// cannot appear in function signatures, so the top-level iteration is done
/// via a macro.  Nested SDGs are handled by a second pass over `sd_or_sdg_as_sdg`
/// entries using the same building-block macros.
macro_rules! append_sdg_rows {
    ($rows:expr, $dtc:expr) => {{
        let sdgs = $dtc.sdgs().and_then(|s| s.sdgs());
        if let Some(groups) = sdgs {
            for sdg in groups.iter() {
                let caption = sdg.caption_sn().unwrap_or("SDG").to_owned();
                let Some(sds) = sdg.sds() else { continue };

                let (has_named, unnamed_count) = sdg_entry_stats!(sds);
                let indent = emit_sdg_header!($rows, sdg, caption, has_named, 0usize);
                emit_sd_rows!($rows, sds, caption, unnamed_count, indent);

                // Second level: nested SDGs
                for entry in sds.iter() {
                    if let Some(nested) = entry.sd_or_sdg_as_sdg() {
                        let n_caption = nested.caption_sn().unwrap_or("SDG").to_owned();
                        if let Some(n_sds) = nested.sds() {
                            let (n_has_named, n_unnamed_count) = sdg_entry_stats!(n_sds);
                            let n_indent =
                                emit_sdg_header!($rows, nested, n_caption, n_has_named, indent);
                            emit_sd_rows!($rows, n_sds, n_caption, n_unnamed_count, n_indent);
                        }
                    }
                }
            }
        }
    }};
}

/// Add DTC child nodes under an individual DTC-DOP tree node
fn add_dtc_dop_children(b: &mut TreeBuilder, dop_info: &DopInfo<'_>, depth: usize) {
    use cda_database::datatypes::DataOperationVariant;

    let Ok(DataOperationVariant::Dtc(dtc_dop)) = dop_info.dop.variant() else {
        return;
    };
    let Some(dtcs) = dtc_dop.dtcs() else {
        return;
    };

    for dtc in dtcs.iter() {
        let short_name = dtc.short_name().unwrap_or("?").to_owned();
        let code_str = dtc
            .display_trouble_code()
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| format!("0x{:06X}", dtc.trouble_code()));
        let text = dtc.text().and_then(|t| t.value()).unwrap_or("").to_owned();

        let display_name = format!("{} - {}", short_name, code_str);

        // Build property rows from fixed fields + optional fields via iterator
        let mut rows: Vec<DetailRow> = vec![
            DetailRow::normal(
                vec!["Short Name".to_owned(), short_name],
                vec![CellType::Text, CellType::Text],
                0,
            ),
            DetailRow::normal(
                vec![
                    "Trouble Code (numeric)".to_owned(),
                    format!("0x{:06X} ({})", dtc.trouble_code(), dtc.trouble_code()),
                ],
                vec![CellType::Text, CellType::Text],
                0,
            ),
        ];

        // Collect optional property rows via flatten
        let optional_rows: Vec<DetailRow> = [
            dtc.display_trouble_code()
                .map(|dc| kv_row("Display Trouble Code", dc.to_owned(), CellType::Text, 0)),
            (!text.is_empty()).then(|| kv_row("Text", text, CellType::Text, 0)),
            dtc.text()
                .and_then(|t| t.ti())
                .map(|ti| kv_row("Text ID (ti)", ti.to_owned(), CellType::Text, 0)),
            dtc.level()
                .map(|l| kv_row("Level (Severity)", l.to_string(), CellType::NumericValue, 0)),
            Some(kv_row(
                "Is Temporary",
                dtc.is_temporary().to_string(),
                CellType::Text,
                0,
            )),
        ]
        .into_iter()
        .flatten()
        .collect();

        rows.extend(optional_rows);

        // Append flattened SDG rows
        append_sdg_rows!(rows, dtc);

        let detail = vec![DetailSectionData {
            title: "Overview".to_owned(),
            render_as_header: false,
            section_type: DetailSectionType::Overview,
            content: DetailContent::Table {
                header: DetailRow::header(
                    vec!["Property".to_owned(), "Value".to_owned()],
                    vec![CellType::Text, CellType::Text],
                ),
                rows,
                constraints: vec![
                    ColumnConstraint::Percentage(30),
                    ColumnConstraint::Percentage(70),
                ],
                use_row_selection: false,
            },
        }];

        b.push_details_structured(depth, display_name, false, false, detail, NodeType::Default);
    }
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
        cell_types: vec![
            CellType::Text,
            CellType::Text,
            CellType::Text,
            CellType::Text,
            CellType::Text,
        ],
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
            cell_types: vec![
                CellType::Text,
                CellType::Text,
                CellType::Text,
                CellType::Text,
                CellType::Text,
            ],
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
        sections.insert(
            0,
            DetailSectionData {
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
            },
        );
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
            cells: vec![
                "Diag Coded Type".to_owned(),
                format!("{:?}", coded_type.base_datatype()),
            ],
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
            cells: vec![
                "Category".to_owned(),
                format!("{:?}", compu_method.category()),
            ],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });

        if let Some(internal_to_phys) = compu_method.internal_to_phys()
            && let Some(scales) = internal_to_phys.compu_scales()
        {
            compu_i2p_rows.push(DetailRow {
                cells: vec!["Scales Count".to_owned(), scales.len().to_string()],
                cell_types: vec![CellType::Text, CellType::NumericValue],
                indent: 0,
                row_type: DetailRowType::Normal,
                metadata: None,
            });
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

    if let Some(compu_method) = normal_dop.compu_method()
        && let Some(phys_to_internal) = compu_method.phys_to_internal()
        && let Some(scales) = phys_to_internal.compu_scales()
    {
        compu_p2i_rows.push(DetailRow {
            cells: vec!["Scales Count".to_owned(), scales.len().to_string()],
            cell_types: vec![CellType::Text, CellType::NumericValue],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
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

/// Build tabbed sections for DTCDOP: Summary (types) + DTCS table
fn build_dtc_dop_tabs(
    dtc_dop: &cda_database::datatypes::DtcDop<'_>,
    types_rows: &mut Vec<DetailRow>,
    sections: &mut Vec<DetailSectionData>,
) {
    if let Ok(coded_type) = dtc_dop.diag_coded_type() {
        types_rows.push(DetailRow {
            cells: vec![
                "Diag Coded Type".to_owned(),
                format!("{:?}", coded_type.base_datatype()),
            ],
            cell_types: vec![CellType::Text, CellType::Text],
            indent: 0,
            row_type: DetailRowType::Normal,
            metadata: None,
        });
    }

    if let Some(compu_method) = dtc_dop.compu_method() {
        types_rows.push(DetailRow {
            cells: vec![
                "Compu Category".to_owned(),
                format!("{:?}", compu_method.category()),
            ],
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

    // DTCS tab: list all individual DTCs with row selection to navigate to children
    if let Some(dtcs) = dtc_dop.dtcs() {
        let dtcs_header = DetailRow {
            cells: vec![
                "ShortName".to_owned(),
                "Trouble Code".to_owned(),
                "Text".to_owned(),
            ],
            cell_types: vec![CellType::Text, CellType::Text, CellType::Text],
            indent: 0,
            ..Default::default()
        };

        let mut dtcs_rows: Vec<DetailRow> = Vec::new();
        for dtc in dtcs.iter() {
            let short_name = dtc.short_name().unwrap_or("?").to_owned();
            let display_code = dtc.display_trouble_code().unwrap_or("");
            let code_str = if !display_code.is_empty() {
                display_code.to_owned()
            } else {
                format!("0x{:06X}", dtc.trouble_code())
            };
            let text = dtc.text().and_then(|t| t.value()).unwrap_or("").to_owned();

            dtcs_rows.push(DetailRow {
                cells: vec![short_name, code_str, text],
                cell_types: vec![CellType::Text, CellType::Text, CellType::Text],
                indent: 0,
                row_type: DetailRowType::Normal,
                metadata: None,
            });
        }

        sections.push(DetailSectionData {
            title: "DTCS".to_owned(),
            render_as_header: false,
            section_type: DetailSectionType::Overview,
            content: DetailContent::Table {
                header: dtcs_header,
                rows: dtcs_rows,
                constraints: vec![
                    ColumnConstraint::Percentage(10),
                    ColumnConstraint::Percentage(10),
                    ColumnConstraint::Percentage(80),
                ],
                use_row_selection: true,
            },
        });
    }
}
