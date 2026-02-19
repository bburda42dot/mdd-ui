use cda_database::datatypes::{DiagLayer, DiagService, Parameter, ParamType, ParentRef};

use super::{NodeId, TreeBuilder, DetailSectionData, DetailRow, DetailContent, NodeType, CellType, ColumnConstraint};

// Helper functions to extract parameter data
fn extract_coded_value(param: &Parameter<'_>) -> String {
    let Ok(pt) = param.param_type() else {
        return String::new();
    };
    
    if !matches!(pt, ParamType::CodedConst) {
        return String::new();
    }
    
    param.specific_data_as_coded_const()
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

fn extract_dop_name(param: &Parameter<'_>) -> String {
    let Ok(pt) = param.param_type() else {
        return String::new();
    };
    
    if !matches!(pt, ParamType::Value) {
        return String::new();
    }
    
    param.specific_data_as_value()
        .and_then(|vd| vd.dop())
        .and_then(|dop| dop.short_name())
        .map(|s| s.to_owned())
        .unwrap_or_default()
}

impl TreeBuilder {
    /// Add a complete diag layer with structured hierarchy for containers
    /// variant_parent_refs: Optional iterator to parent refs from the variant for fetching inherited services
    pub(super) fn add_diag_layer_structured<'a>(
        &mut self,
        layer: &DiagLayer<'a>,
        depth: usize,
        layer_name: &str,
        _expand: bool,
        variant_parent_refs: Option<impl Iterator<Item = ParentRef<'a>> + 'a>,
    ) {
        // Base Variants (if this is a variant)
        // Note: Base variants would be handled at the container level
        
        // ECU Variants (if this is a base variant)
        // Note: ECU variants would be shown as sibling containers
        
        // Functional Classes
        self.add_functional_classes(layer, depth, layer_name);
        
        // Diag-Data-Dictionary-Spec
        self.add_diag_data_dictionary_spec(layer, depth, layer_name);
        
        // Diag-Comms
        self.add_diag_comms(layer, depth, layer_name, variant_parent_refs);
        
        // Requests (from diag-comms)
        self.add_requests_section(layer, depth, layer_name);
        
        // Pos-Responses (from diag-comms)
        self.add_pos_responses_section(layer, depth, layer_name);
        
        // Neg-Responses (from diag-comms)
        self.add_neg_responses_section(layer, depth, layer_name);
        
        // State-Charts
        self.add_state_charts(layer, depth, layer_name);
        
        // Additional Audiences
        self.add_additional_audiences(layer, depth, layer_name);
        
        // Sub-Components
        self.add_sub_components(layer, depth, layer_name);
        
        // SDGs
        self.add_sdgs(layer, depth, layer_name);
        
        // ComParam Refs
        self.add_com_params(layer, depth, layer_name);
        
        // Parent Refs
        self.add_parent_refs(layer, depth, layer_name);
    }

    // ------------------------------------------------------------------
    // State Charts
    // ------------------------------------------------------------------

    fn add_state_charts(&mut self, layer: &DiagLayer<'_>, depth: usize, layer_name: &str) {
        let Some(charts) = layer.state_charts() else { return };
        if charts.is_empty() {
            return;
        }

        self.push(
            depth,
            format!("State Charts ({})", charts.len()),
            false,
            true,
            NodeId::Static(format!("layer_{layer_name}_state_charts")),
            NodeType::SectionHeader,
        );

        for (ci, chart) in charts.iter().enumerate() {
            let chart_name = chart.short_name().unwrap_or("unnamed");
            let prefix = format!("layer_{layer_name}_sc_{ci}");
            self.push(depth + 1, chart_name.to_owned(), false, true, NodeId::Static(prefix.clone()), NodeType::Default);

            for (si, state) in chart.states().into_iter().flatten().enumerate() {
                let sn = state.short_name().unwrap_or("?");
                self.push_leaf(
                    depth + 2,
                    format!("State: {sn}"),
                    NodeId::Static(format!("{prefix}_state_{si}")),
                    NodeType::Default,
                );
            }

            for (ti, tr) in chart.state_transitions().into_iter().flatten().enumerate() {
                let src = tr.source_short_name_ref().unwrap_or("?");
                let tgt = tr.target_short_name_ref().unwrap_or("?");
                self.push_leaf(
                    depth + 2,
                    format!("Transition: {src} -> {tgt}"),
                    NodeId::Static(format!("{prefix}_tr_{ti}")),
                    NodeType::Default,
                );
            }
        }
    }

    // ------------------------------------------------------------------
    // Structured DiagLayer Sections
    // ------------------------------------------------------------------

    fn add_functional_classes(&mut self, _layer: &DiagLayer<'_>, depth: usize, layer_name: &str) {
        // Functional classes API not directly available
        // Adding as placeholder
        self.push_leaf(
            depth,
            "Functional Classes".to_string(),
            NodeId::Static(format!("layer_{layer_name}_fcs")),
            NodeType::Default,
        );
    }

    fn add_diag_data_dictionary_spec(&mut self, layer: &DiagLayer<'_>, depth: usize, layer_name: &str) {
        // Check if layer has diagnostic data dictionary specifications
        // This would typically be accessed through specific methods if available
        // For now, we'll add a placeholder if the layer has data operations or similar
        let has_spec = layer.diag_services().map_or(false, |s| !s.is_empty());
        
        if has_spec {
            self.push_leaf(
                depth,
                "Diag-Data-Dictionary-Spec".to_string(),
                NodeId::Static(format!("layer_{layer_name}_ddd_spec")),
                NodeType::Default,
            );
        }
    }

    fn add_diag_comms<'a>(
        &mut self,
        layer: &DiagLayer<'a>,
        depth: usize,
        layer_name: &str,
        variant_parent_refs: Option<impl Iterator<Item = ParentRef<'a>> + 'a>,
    ) {
        // Collect own services
        let own_services: Vec<DiagService<'_>> = layer.diag_services()
            .map(|services| services.iter().map(DiagService).collect())
            .unwrap_or_default();
        
        // Collect services from parent refs
        let parent_services: Vec<DiagService<'_>> = if let Some(parent_refs) = variant_parent_refs {
            Self::get_parent_ref_services_recursive(parent_refs)
        } else {
            Vec::new()
        };
        
        let total_count = own_services.len() + parent_services.len();
        
        if total_count > 0 {
            self.push(
                depth,
                format!("Diag-Comms ({})", total_count),
                false,
                true,
                NodeId::Static(format!("layer_{layer_name}_diag_comms")),
                NodeType::SectionHeader,
            );
            
            // Add own services first
            for (i, ds) in own_services.iter().enumerate() {
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
                    
                    let sections = self.build_diag_comm_details(ds);

                    self.push_details_structured(
                        depth + 1,
                        display_name,
                        false,
                        false,
                        NodeId::Static(format!("layer_{layer_name}_dc_{i}")),
                        sections,
                        NodeType::Service,
                    );
                }
            }
            
            // Add parent ref services with different node type
            for (i, ds) in parent_services.iter().enumerate() {
                let ds: &DiagService<'_> = ds;  // Explicit type annotation
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
                    
                    let sections = self.build_diag_comm_details(ds);

                    self.push_details_structured(
                        depth + 1,
                        display_name,
                        false,
                        false,
                        NodeId::Static(format!("layer_{layer_name}_dc_parent_{i}")),
                        sections,
                        NodeType::ParentRefService, // Mark as inherited
                    );
                }
            }
        }
    }
    
    /// Get services from parent references recursively with proper filtering
    fn get_parent_ref_services_recursive<'a>(
        parent_refs: impl Iterator<Item = ParentRef<'a>>,
    ) -> Vec<DiagService<'a>> {
        fn filter_not_inherited_services<'a>(
            diag_services: impl Iterator<Item = impl Into<DiagService<'a>>>,
            not_inherited_names: &[&str],
        ) -> Vec<DiagService<'a>> {
            diag_services
                .into_iter()
                .map(|s| s.into())
                .filter(|service| {
                    service
                        .diag_comm()
                        .and_then(|dc| dc.short_name())
                        .map_or(true, |name| !not_inherited_names.contains(&name))
                })
                .collect()
        }

        fn find_services_recursive<'a>(
            parent_refs: impl Iterator<Item = ParentRef<'a>>,
        ) -> Vec<DiagService<'a>> {
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
                            let services = parent_ref
                                .ref__as_ecu_shared_data()?
                                .diag_layer()?
                                .diag_services()?
                                .iter()
                                .map(DiagService);
                            Some(filter_not_inherited_services(services, &not_inherited_names))
                        }
                        Ok(cda_database::datatypes::ParentRefType::FunctionalGroup) => parent_ref
                            .ref__as_functional_group()
                            .and_then(|fg| fg.parent_refs())
                            .map(|nested_refs| {
                                find_services_recursive(nested_refs.iter().map(ParentRef))
                            }),
                        Ok(cda_database::datatypes::ParentRefType::Protocol) => {
                            let services = parent_ref
                                .ref__as_protocol()?
                                .diag_layer()?
                                .diag_services()?
                                .iter()
                                .map(DiagService);
                            Some(filter_not_inherited_services(services, &not_inherited_names))
                        }
                        Ok(cda_database::datatypes::ParentRefType::Variant) => {
                            let services = parent_ref
                                .ref__as_variant()?
                                .diag_layer()?
                                .diag_services()?
                                .iter()
                                .map(DiagService);
                            Some(filter_not_inherited_services(services, &not_inherited_names))
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

    fn add_requests_section(&mut self, layer: &DiagLayer<'_>, depth: usize, layer_name: &str) {
        if let Some(services) = layer.diag_services() {
            let request_count: usize = services.iter().filter(|&s| {
                DiagService(s).request().is_some()
            }).count();
            
            if request_count > 0 {
                self.push(
                    depth,
                    format!("Requests ({})", request_count),
                    false,
                    true,
                    NodeId::Static(format!("layer_{layer_name}_requests")),
                    NodeType::SectionHeader,
                );
                
                for (i, svc) in services.iter().enumerate() {
                    let ds = DiagService(svc);
                    if ds.request().is_some() {
                        let name = ds.diag_comm().and_then(|dc| dc.short_name()).unwrap_or("?");
                        self.push_leaf(
                            depth + 1,
                            format!("Request: {name}"),
                            NodeId::Static(format!("layer_{layer_name}_req_{i}")),
                            NodeType::Request,
                        );
                    }
                }
            }
        }
    }

    fn add_pos_responses_section(&mut self, layer: &DiagLayer<'_>, depth: usize, layer_name: &str) {
        if let Some(services) = layer.diag_services() {
            let response_count: usize = services.iter().filter(|&s| {
                DiagService(s).pos_responses().map_or(false, |r| !r.is_empty())
            }).count();
            
            if response_count > 0 {
                self.push(
                    depth,
                    format!("Pos-Responses ({})", response_count),
                    false,
                    true,
                    NodeId::Static(format!("layer_{layer_name}_pos_responses")),
                    NodeType::SectionHeader,
                );
                
                for (i, svc) in services.iter().enumerate() {
                    let ds = DiagService(svc);
                    if ds.pos_responses().map_or(false, |r| !r.is_empty()) {
                        let name = ds.diag_comm().and_then(|dc| dc.short_name()).unwrap_or("?");
                        self.push_leaf(
                            depth + 1,
                            format!("PosResponse: {name}"),
                            NodeId::Static(format!("layer_{layer_name}_pos_resp_{i}")),
                            NodeType::PosResponse,
                        );
                    }
                }
            }
        }
    }

    fn add_neg_responses_section(&mut self, layer: &DiagLayer<'_>, depth: usize, layer_name: &str) {
        if let Some(services) = layer.diag_services() {
            let response_count: usize = services.iter().filter(|&s| {
                DiagService(s).neg_responses().map_or(false, |r| !r.is_empty())
            }).count();
            
            if response_count > 0 {
                self.push(
                    depth,
                    format!("Neg-Responses ({})", response_count),
                    false,
                    true,
                    NodeId::Static(format!("layer_{layer_name}_neg_responses")),
                    NodeType::SectionHeader,
                );
                
                for (i, svc) in services.iter().enumerate() {
                    let ds = DiagService(svc);
                    if ds.neg_responses().map_or(false, |r| !r.is_empty()) {
                        let name = ds.diag_comm().and_then(|dc| dc.short_name()).unwrap_or("?");
                        self.push_leaf(
                            depth + 1,
                            format!("NegResponse: {name}"),
                            NodeId::Static(format!("layer_{layer_name}_neg_resp_{i}")),
                            NodeType::NegResponse,
                        );
                    }
                }
            }
        }
    }

    fn add_additional_audiences(&mut self, layer: &DiagLayer<'_>, depth: usize, layer_name: &str) {
        // Additional audiences would be part of admin data or metadata
        // This is typically not directly available in the DiagLayer API
        // Adding placeholder for structure
        if let Some(services) = layer.diag_services() {
            let has_audiences = services.iter().any(|s| {
                DiagService(s).diag_comm().and_then(|dc| dc.audience()).is_some()
            });
            
            if has_audiences {
                self.push_leaf(
                    depth,
                    "Additional Audiences".to_string(),
                    NodeId::Static(format!("layer_{layer_name}_audiences")),
                    NodeType::Default,
                );
            }
        }
    }

    fn add_sub_components(&mut self, layer: &DiagLayer<'_>, depth: usize, layer_name: &str) {
        // Sub-components would be nested diagnostic layers or related structures
        // Placeholder for now
        if let Some(_jobs) = layer.single_ecu_jobs() {
            self.push_leaf(
                depth,
                "Sub-Components".to_string(),
                NodeId::Static(format!("layer_{layer_name}_sub_components")),
                NodeType::Default,
            );
        }
    }

    fn add_sdgs(&mut self, _layer: &DiagLayer<'_>, depth: usize, layer_name: &str) {
        // SDGs (Special Data Groups) from the layer
        // These would be accessed through specific methods if available
        // Placeholder for structure
        self.push_leaf(
            depth,
            "SDGs".to_string(),
            NodeId::Static(format!("layer_{layer_name}_sdgs")),
            NodeType::Default,
        );
    }

    fn add_parent_refs(&mut self, _layer: &DiagLayer<'_>, depth: usize, layer_name: &str) {
        // Parent refs API may not be available in current version
        // Adding as a placeholder for future implementation
        self.push_leaf(
            depth,
            "Parent Refs".to_string(),
            NodeId::Static(format!("layer_{layer_name}_parent_refs")),
            NodeType::Default,
        );
    }

    // ------------------------------------------------------------------
    // ComParam Refs
    // ------------------------------------------------------------------

    fn add_com_params(&mut self, layer: &DiagLayer<'_>, depth: usize, layer_name: &str) {
        let Some(cp_refs) = layer.com_param_refs() else { return };
        if cp_refs.is_empty() {
            return;
        }

        self.push(
            depth,
            format!("ComParam Refs ({})", cp_refs.len()),
            false,
            true,
            NodeId::Static(format!("layer_{layer_name}_comparams")),
            NodeType::SectionHeader,
        );

        for (ci, cpr) in cp_refs.iter().enumerate() {
            let Some(cp) = cpr.com_param() else { continue };
            let cp_name = cp.short_name().unwrap_or("?");
            let cp_type = format!("{:?}", cp.com_param_type());
            self.push_leaf(
                depth + 1,
                format!("{cp_name} ({cp_type})"),
                NodeId::Static(format!("layer_{layer_name}_cp_{ci}")),
                NodeType::Default,
            );
        }
    }

    // ------------------------------------------------------------------
    // Detailed DiagComm (Service) Detail Pane Builder
    // ------------------------------------------------------------------

    fn build_diag_comm_details(&self, ds: &DiagService<'_>) -> Vec<DetailSectionData> {
        let mut sections: Vec<DetailSectionData> = Vec::new();

        // Overview - plain text key-value pairs (no table header)
        let mut overview_lines = Vec::new();
        if let Some(dc) = ds.diag_comm() {
            if let Some(sn) = dc.short_name() {
                overview_lines.push(format!("Service: {}", sn));
            }
            if let Some(semantic) = dc.semantic() {
                overview_lines.push(format!("Semantic: {}", semantic));
            }
        }
        if let Some(sid) = ds.request_id() {
            overview_lines.push(format!("SID: 0x{sid:02X}"));
        }
        if let Some((sub_fn, bit_len)) = ds.request_sub_function_id() {
            overview_lines.push(format!("Sub-Function: 0x{sub_fn:04X} ({bit_len} bits)"));
        }
        overview_lines.push(format!("Addressing: {:?}", ds.addressing()));
        overview_lines.push(format!("Transmission: {:?}", ds.transmission_mode()));

        sections.push(DetailSectionData { 
            title: "Overview".to_owned(), 
            content: DetailContent::PlainText(overview_lines),
        });

        // Helper to build parameter table sections
        fn build_param_section<'a, I>(title: &str, params: I) -> DetailSectionData
        where
            I: IntoIterator<Item = Parameter<'a>>
        {
            use crate::tree::ColumnConstraint;
            
            let header = DetailRow { 
                cells: vec![
                    "Short Name".to_owned(), 
                    "Byte".to_owned(), 
                    "Bit".to_owned(), 
                    "Bit\nLen".to_owned(), 
                    "Byte\nLen".to_owned(), 
                    "Value".to_owned(), 
                    "DOP".to_owned(), 
                    "Semantic".to_owned()
                ], 
                cell_types: vec![CellType::Text, CellType::NumericValue, CellType::NumericValue, CellType::NumericValue, CellType::NumericValue, CellType::Text, CellType::Text, CellType::Text],
                indent: 0, 
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
                    cells: vec![name, byte_pos.to_string(), bit_pos.to_string(), bit_len, byte_len, value, dop_name, semantic],
                    cell_types: vec![
                        CellType::ParameterName, 
                        CellType::NumericValue, 
                        CellType::NumericValue, 
                        CellType::Text, 
                        CellType::Text, 
                        CellType::NumericValue, 
                        if has_dop { CellType::DopReference } else { CellType::Text },
                        CellType::Text
                    ],
                    indent: 0,
                });
            }

            DetailSectionData { 
                title: title.to_owned(), 
                content: DetailContent::Table { 
                    header, 
                    rows,
                    constraints: vec![
                        ColumnConstraint::Percentage(45),  // Short Name (increased from 35)
                        ColumnConstraint::Fixed(4),        // Byte (decreased from 6)
                        ColumnConstraint::Fixed(3),        // Bit (decreased from 5)
                        ColumnConstraint::Fixed(4),        // Bit Len (decreased from 5)
                        ColumnConstraint::Fixed(5),        // Byte Len (decreased from 6)
                        ColumnConstraint::Percentage(15),  // Value
                        ColumnConstraint::Percentage(15),  // DOP
                        ColumnConstraint::Percentage(25),  // Semantic (decreased from 35)
                    ],
                },
            }
        }

        // Request params
        if let Some(req) = ds.request() {
            let params = req.params().into_iter().flatten().map(Parameter);
            sections.push(build_param_section("Request", params));
        }

        // Pos responses
        if let Some(pos) = ds.pos_responses() {
            for (i, resp) in pos.iter().enumerate() {
                let params = resp.params().into_iter().flatten().map(Parameter);
                sections.push(build_param_section(&format!("Pos-Response {}", i + 1), params));
            }
        }

        // Neg responses
        if let Some(neg) = ds.neg_responses() {
            for (i, resp) in neg.iter().enumerate() {
                let params = resp.params().into_iter().flatten().map(Parameter);
                sections.push(build_param_section(&format!("Neg-Response {}", i + 1), params));
            }
        }

        // ComParam refs
        let comparam_header = DetailRow {
            cells: vec!["ComParam".to_owned(), "Value".to_owned(), "Complex Value".to_owned(), "Protocol".to_owned(), "Prot-Stack".to_owned()],
            cell_types: vec![CellType::Text, CellType::Text, CellType::Text, CellType::Text, CellType::Text],
            indent: 0,
        };
        sections.push(DetailSectionData { 
            title: "ComParam-Refs".to_owned(), 
            content: DetailContent::Table {
                header: comparam_header,
                rows: vec![DetailRow { 
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
            },
        });

        // Audience section - Combined into single tab with composite content
        if let Some(dc) = ds.diag_comm() {
            if let Some(audience) = dc.audience() {
                let mut subsections = Vec::new();
                
                // Flags subsection
                let mut flag_lines = Vec::new();
                flag_lines.push(format!("IS_MANUFACTURER: {}", if audience.is_manufacturing() { "true" } else { "false" }));
                flag_lines.push(format!("IS_DEVELOPMENT: {}", if audience.is_development() { "true" } else { "false" }));
                flag_lines.push(format!("IS_AFTERSALES: {}", if audience.is_after_sales() { "true" } else { "false" }));
                flag_lines.push(format!("IS_AFTERMARKET: {}", if audience.is_after_market() { "true" } else { "false" }));
                
                subsections.push(DetailSectionData { 
                    title: "Audience Flags".to_owned(), 
                    content: DetailContent::PlainText(flag_lines),
                });
                
                // Additional audiences subsection
                if let Some(audiences) = audience.enabled_audiences() {
                    let audiences_list: Vec<_> = audiences.iter()
                        .filter_map(|aa| aa.short_name().map(|s| s.to_owned()))
                        .collect();
                    
                    if !audiences_list.is_empty() {
                        subsections.push(DetailSectionData { 
                            title: "Additional Audiences".to_owned(), 
                            content: DetailContent::PlainText(audiences_list),
                        });
                    }
                }
                
                sections.push(DetailSectionData { 
                    title: "Audience".to_owned(), 
                    content: DetailContent::Composite(subsections),
                });
            } else {
                sections.push(DetailSectionData { 
                    title: "Audience".to_owned(), 
                    content: DetailContent::PlainText(vec!["(No audience info)".to_owned()]),
                });
            }
        }

        // SDGs
        let sdg_header = DetailRow {
            cells: vec!["Short Name".to_owned(), "SD".to_owned(), "SI".to_owned(), "TI".to_owned()],
            cell_types: vec![CellType::Text, CellType::Text, CellType::Text, CellType::Text],
            indent: 0,
        };
        sections.push(DetailSectionData { 
            title: "SDGs".to_owned(), 
            content: DetailContent::Table {
                header: sdg_header,
                rows: vec![DetailRow { 
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
            },
        });

        // States
        if let Some(dc) = ds.diag_comm() {
            let header = DetailRow {
                cells: vec!["Short Name".to_owned()],
                cell_types: vec![CellType::Text],
                indent: 0,
            };
            
            let mut rows = Vec::new();
            for pc in dc.pre_condition_state_refs().into_iter().flatten() {
                if let Some(val) = pc.value() {
                    rows.push(DetailRow { 
                        cells: vec![val.to_owned()], 
                        cell_types: vec![CellType::Text],
                        indent: 0, 
                    });
                }
            }
            for st in dc.state_transition_refs().into_iter().flatten() {
                if let Some(val) = st.value() {
                    rows.push(DetailRow { 
                        cells: vec![val.to_owned()], 
                        cell_types: vec![CellType::Text],
                        indent: 0, 
                    });
                }
            }
            sections.push(DetailSectionData { 
                title: "States".to_owned(), 
                content: DetailContent::Table { 
                    header, 
                    rows,
                    constraints: vec![ColumnConstraint::Percentage(100)],
                },
            });
        }

        // Related diag comm refs
        let related_header = DetailRow {
            cells: vec!["Short Name".to_owned()],
            cell_types: vec![CellType::Text],
            indent: 0,
        };
        sections.push(DetailSectionData { 
            title: "Related-Diag-Comm-Refs".to_owned(), 
            content: DetailContent::Table {
                header: related_header,
                rows: vec![DetailRow { 
                    cells: vec!["(Related comms not available)".to_owned()], 
                    cell_types: vec![CellType::Text],
                    indent: 0, 
                }],
                constraints: vec![ColumnConstraint::Percentage(100)],
            },
        });

        sections
    }
}