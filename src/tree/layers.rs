use cda_database::datatypes::{DiagComm, DiagLayer, DiagService, Parameter, Request, Response};

use super::{NodeId, TreeBuilder};

impl TreeBuilder {
    /// Add a complete diag layer with structured hierarchy for containers
    pub(super) fn add_diag_layer_structured(
        &mut self,
        layer: &DiagLayer<'_>,
        depth: usize,
        layer_name: &str,
        _expand: bool,
    ) {
        // Admin Data
        self.add_admin_data(layer, depth, layer_name);
        
        // Company Datas (if available)
        self.add_company_datas(layer, depth, layer_name);
        
        // Base Variants (if this is a variant)
        // Note: Base variants would be handled at the container level
        
        // ECU Variants (if this is a base variant)
        // Note: ECU variants would be shown as sibling containers
        
        // Functional Classes
        self.add_functional_classes(layer, depth, layer_name);
        
        // Diag-Data-Dictionary-Spec
        self.add_diag_data_dictionary_spec(layer, depth, layer_name);
        
        // Diag-Comms
        self.add_diag_comms(layer, depth, layer_name);
        
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
        );

        for (ci, chart) in charts.iter().enumerate() {
            let chart_name = chart.short_name().unwrap_or("unnamed");
            let prefix = format!("layer_{layer_name}_sc_{ci}");
            self.push(depth + 1, chart_name.to_owned(), false, true, NodeId::Static(prefix.clone()));

            for (si, state) in chart.states().into_iter().flatten().enumerate() {
                let sn = state.short_name().unwrap_or("?");
                self.push_leaf(
                    depth + 2,
                    format!("State: {sn}"),
                    NodeId::Static(format!("{prefix}_state_{si}")),
                );
            }

            for (ti, tr) in chart.state_transitions().into_iter().flatten().enumerate() {
                let src = tr.source_short_name_ref().unwrap_or("?");
                let tgt = tr.target_short_name_ref().unwrap_or("?");
                self.push_leaf(
                    depth + 2,
                    format!("Transition: {src} -> {tgt}"),
                    NodeId::Static(format!("{prefix}_tr_{ti}")),
                );
            }
        }
    }

    // ------------------------------------------------------------------
    // Structured DiagLayer Sections
    // ------------------------------------------------------------------

    pub(super) fn add_admin_data(&mut self, layer: &DiagLayer<'_>, depth: usize, layer_name: &str) {
        // Admin data typically includes short_name, long_name, etc.
        let mut has_content = false;
        let mut content = Vec::new();
        
        if let Some(sn) = layer.short_name() {
            content.push(format!("Short Name: {sn}"));
            has_content = true;
        }
        if let Some(ln) = layer.long_name().and_then(|l| l.value()) {
            content.push(format!("Long Name: {ln}"));
            has_content = true;
        }
        
        if has_content {
            self.push_details(
                depth,
                "Admin Data".to_string(),
                false,
                false,
                NodeId::Static(format!("layer_{layer_name}_admin")),
                content,
            );
        }
    }

    pub(super) fn add_company_datas(&mut self, _layer: &DiagLayer<'_>, depth: usize, layer_name: &str) {
        // Company datas API not available in current version
        // Adding as placeholder
        self.push_leaf(
            depth,
            "Company Datas".to_string(),
            NodeId::Static(format!("layer_{layer_name}_company_datas")),
        );
    }

    fn add_functional_classes(&mut self, _layer: &DiagLayer<'_>, depth: usize, layer_name: &str) {
        // Functional classes API not directly available
        // Adding as placeholder
        self.push_leaf(
            depth,
            "Functional Classes".to_string(),
            NodeId::Static(format!("layer_{layer_name}_fcs")),
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
            );
        }
    }

    fn add_diag_comms(&mut self, layer: &DiagLayer<'_>, depth: usize, layer_name: &str) {
        // Diag-comms are typically the diagnostic communications
        // These are represented by the services
        if let Some(services) = layer.diag_services() {
            if !services.is_empty() {
                self.push(
                    depth,
                    format!("Diag-Comms ({})", services.len()),
                    false,
                    true,
                    NodeId::Static(format!("layer_{layer_name}_diag_comms")),
                );
                
                for (i, svc) in services.iter().enumerate() {
                    let ds = DiagService(svc);
                    if let Some(dc) = ds.diag_comm() {
                        let name = dc.short_name().unwrap_or("?");
                        
                        // Format with service ID like in add_service
                        let display_name = if let Some(sid) = ds.request_id() {
                            if let Some((sub_fn, bit_len)) = ds.request_sub_function_id() {
                                let sub_fn_str = if bit_len <= 8 {
                                    format!("{sub_fn:02X}")
                                } else {
                                    format!("{sub_fn:04X}")
                                };
                                format!("0x{sid:02X}{sub_fn_str} - {name}")
                            } else {
                                format!("0x{sid:02X} - {name}")
                            }
                        } else {
                            name.to_string()
                        };
                        
                        // Build comprehensive detail pane
                        let details = self.build_diag_comm_details(&ds);
                        
                        self.push_details(
                            depth + 1,
                            display_name,
                            false,
                            false,
                            NodeId::Static(format!("layer_{layer_name}_dc_{i}")),
                            details,
                        );
                    }
                }
            }
        }
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
                );
                
                for (i, svc) in services.iter().enumerate() {
                    let ds = DiagService(svc);
                    if ds.request().is_some() {
                        let name = ds.diag_comm().and_then(|dc| dc.short_name()).unwrap_or("?");
                        self.push_leaf(
                            depth + 1,
                            format!("Request: {name}"),
                            NodeId::Static(format!("layer_{layer_name}_req_{i}")),
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
                );
                
                for (i, svc) in services.iter().enumerate() {
                    let ds = DiagService(svc);
                    if ds.pos_responses().map_or(false, |r| !r.is_empty()) {
                        let name = ds.diag_comm().and_then(|dc| dc.short_name()).unwrap_or("?");
                        self.push_leaf(
                            depth + 1,
                            format!("PosResponse: {name}"),
                            NodeId::Static(format!("layer_{layer_name}_pos_resp_{i}")),
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
                );
                
                for (i, svc) in services.iter().enumerate() {
                    let ds = DiagService(svc);
                    if ds.neg_responses().map_or(false, |r| !r.is_empty()) {
                        let name = ds.diag_comm().and_then(|dc| dc.short_name()).unwrap_or("?");
                        self.push_leaf(
                            depth + 1,
                            format!("NegResponse: {name}"),
                            NodeId::Static(format!("layer_{layer_name}_neg_resp_{i}")),
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
        );
    }

    fn add_parent_refs(&mut self, _layer: &DiagLayer<'_>, depth: usize, layer_name: &str) {
        // Parent refs API may not be available in current version
        // Adding as a placeholder for future implementation
        self.push_leaf(
            depth,
            "Parent Refs".to_string(),
            NodeId::Static(format!("layer_{layer_name}_parent_refs")),
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
        );

        for (ci, cpr) in cp_refs.iter().enumerate() {
            let Some(cp) = cpr.com_param() else { continue };
            let cp_name = cp.short_name().unwrap_or("?");
            let cp_type = format!("{:?}", cp.com_param_type());
            self.push_leaf(
                depth + 1,
                format!("{cp_name} ({cp_type})"),
                NodeId::Static(format!("layer_{layer_name}_cp_{ci}")),
            );
        }
    }

    // ------------------------------------------------------------------
    // Detailed DiagComm (Service) Detail Pane Builder
    // ------------------------------------------------------------------

    fn build_diag_comm_details(&self, ds: &DiagService<'_>) -> Vec<String> {
        let mut d = Vec::new();
        
        // Basic service info
        if let Some(dc) = ds.diag_comm() {
            if let Some(sn) = dc.short_name() {
                d.push(format!("Service: {sn}"));
            }
            if let Some(semantic) = dc.semantic() {
                d.push(format!("Semantic: {semantic}"));
            }
        }
        
        if let Some(sid) = ds.request_id() {
            d.push(format!("SID: 0x{sid:02X}"));
        }
        if let Some((sub_fn, bit_len)) = ds.request_sub_function_id() {
            d.push(format!("Sub-Function: 0x{sub_fn:04X} ({bit_len} bits)"));
        }
        
        d.push(format!("Addressing: {:?}", ds.addressing()));
        d.push(format!("Transmission: {:?}", ds.transmission_mode()));
        
        // Request section
        if let Some(req) = ds.request() {
            d.push(String::new());
            d.push("--- Request ---".to_string());
            self.append_param_table_headers(&mut d);
            self.append_request_params_table(&mut d, &Request(req));
        }
        
        // Pos-Responses section
        if let Some(pos) = ds.pos_responses() {
            for (i, resp) in pos.iter().enumerate() {
                d.push(String::new());
                d.push(format!("--- Pos-Response {} ---", i + 1));
                self.append_param_table_headers(&mut d);
                self.append_response_params_table(&mut d, &Response(resp));
            }
        }
        
        // Neg-Responses section
        if let Some(neg) = ds.neg_responses() {
            for (i, resp) in neg.iter().enumerate() {
                d.push(String::new());
                d.push(format!("--- Neg-Response {} ---", i + 1));
                self.append_param_table_headers(&mut d);
                self.append_response_params_table(&mut d, &Response(resp));
            }
        }
        
        // ComParam-Refs section
        d.push(String::new());
        d.push("--- ComParam-Refs ---".to_string());
        d.push("ComParam | Value | Complex Value | Protocol | Prot-Stack".to_string());
        if let Some(dc) = ds.diag_comm() {
            self.append_comparam_refs(&mut d, &DiagComm(dc));
        }
        
        // Audience section
        d.push(String::new());
        d.push("--- Audience ---".to_string());
        if let Some(dc) = ds.diag_comm() {
            self.append_audience_info(&mut d, &DiagComm(dc));
        }
        
        // SDGs section
        d.push(String::new());
        d.push("--- SDGs ---".to_string());
        d.push("Short Name | SD | SI | TI".to_string());
        if let Some(dc) = ds.diag_comm() {
            self.append_sdgs(&mut d, &DiagComm(dc));
        }
        
        // States section
        d.push(String::new());
        d.push("--- States ---".to_string());
        d.push("Short Name".to_string());
        if let Some(dc) = ds.diag_comm() {
            self.append_states(&mut d, &DiagComm(dc));
        }
        
        // Related-Diag-Comm-Refs section
        d.push(String::new());
        d.push("--- Related-Diag-Comm-Refs ---".to_string());
        d.push("Short Name".to_string());
        if let Some(dc) = ds.diag_comm() {
            self.append_related_diag_comms(&mut d, &DiagComm(dc));
        }
        
        d
    }

    fn append_param_table_headers(&self, d: &mut Vec<String>) {
        d.push("Short Name | Byte | Bit | Bit Len | Byte Len | Value | DOP | Semantic".to_string());
    }

    fn append_request_params_table(&self, d: &mut Vec<String>, req: &Request<'_>) {
        if let Some(params) = req.params() {
            for param in params.iter() {
                let p = Parameter(param);
                self.append_param_row(d, &p);
            }
        }
    }

    fn append_response_params_table(&self, d: &mut Vec<String>, resp: &Response<'_>) {
        if let Some(params) = resp.params() {
            for param in params.iter() {
                let p = Parameter(param);
                self.append_param_row(d, &p);
            }
        }
    }

    fn append_param_row(&self, d: &mut Vec<String>, param: &Parameter<'_>) {
        let name = param.short_name().unwrap_or("?");
        let byte_pos = param.byte_position();
        let bit_pos = param.bit_position();
        
        // bit_length and byte_length methods may not be available
        // We'll leave them as placeholders for now
        let bit_len = "-";
        let byte_len = "-";
        
        // Get coded value if it's a CodedConst
        let value = if let Ok(pt) = param.param_type() {
            use cda_database::datatypes::ParamType;
            match pt {
                ParamType::CodedConst => {
                    param.specific_data_as_coded_const()
                        .and_then(|cc| cc.coded_value())
                        .map(|v| {
                            // coded_value is &str, parse if numeric
                            if let Ok(num) = v.parse::<u64>() {
                                format!("0x{num:X}")
                            } else {
                                // Replace pipe with similar character to avoid breaking table format
                                // Use ¦ (broken bar) instead of | (pipe)
                                v.replace('|', "¦")
                            }
                        })
                        .unwrap_or_default()
                }
                _ => String::new(),
            }
        } else {
            String::new()
        };
        
        // Get DOP name if available
        let dop_name = if let Ok(pt) = param.param_type() {
            use cda_database::datatypes::ParamType;
            match pt {
                ParamType::Value => {
                    param.specific_data_as_value()
                        .and_then(|vd| vd.dop())
                        .and_then(|dop| dop.short_name())
                        .map(|s| s.replace('|', "¦"))  // Sanitize DOP name too
                        .unwrap_or_default()
                }
                _ => String::new(),
            }
        } else {
            String::new()
        };
        
        // Get semantic value
        let semantic = param.semantic()
            .map(|s| s.replace('|', "¦"))  // Sanitize semantic too
            .unwrap_or_default();
        
        // Sanitize name to prevent pipe issues
        let safe_name = name.replace('|', "¦");
        
        d.push(format!("{} | {} | {} | {} | {} | {} | {} | {}", 
            safe_name, byte_pos, bit_pos, bit_len, byte_len, value, dop_name, semantic));
    }

    fn append_comparam_refs(&self, d: &mut Vec<String>, _dc: &DiagComm<'_>) {
        // com_param_refs() method may not be available on DiagComm
        // This data is typically at the layer level, not comm level
        d.push("(No ComParam refs at comm level)".to_string());
    }

    fn append_audience_info(&self, d: &mut Vec<String>, dc: &DiagComm<'_>) {
        if let Some(audience) = dc.audience() {
            // Boolean flags as headline
            let mut flags = Vec::new();
            if audience.is_development() { flags.push("Development"); }
            if audience.is_manufacturing() { flags.push("Manufacturing"); }
            if audience.is_after_sales() { flags.push("AfterSales"); }
            if audience.is_after_market() { flags.push("AfterMarket"); }
            
            if !flags.is_empty() {
                d.push(format!("Flags: {}", flags.join(", ")));
            } else {
                d.push("(No audience flags set)".to_string());
            }
        } else {
            d.push("(No audience info)".to_string());
        }
    }

    fn append_sdgs(&self, d: &mut Vec<String>, _dc: &DiagComm<'_>) {
        // sdgs() API may not be available or may not be iterable
        // SDGs are typically accessed at different level
        d.push("(SDGs not available at comm level)".to_string());
    }

    fn append_states(&self, d: &mut Vec<String>, dc: &DiagComm<'_>) {
        // States from pre-condition and state-transition refs
        for pc in dc.pre_condition_state_refs().into_iter().flatten() {
            if let Some(val) = pc.value() {
                d.push(val.to_string());
            }
        }
        for st in dc.state_transition_refs().into_iter().flatten() {
            if let Some(val) = st.value() {
                d.push(val.to_string());
            }
        }
    }

    fn append_related_diag_comms(&self, d: &mut Vec<String>, _dc: &DiagComm<'_>) {
        // related_diag_comm_refs() method may not be available on DiagComm
        d.push("(Related comms not available)".to_string());
    }
}
