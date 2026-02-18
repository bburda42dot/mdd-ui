mod layers;

use cda_database::datatypes::{DiagLayer, DiagnosticDatabase, EcuDb, Variant as VariantWrap};


// -----------------------------------------------------------------------
// Public types
// -----------------------------------------------------------------------

/// A single row in the flat tree view. Depth controls indentation, and
/// `expanded` / `has_children` drive the collapse/expand behaviour.
#[derive(Clone)]
pub struct TreeNode {
    pub depth: usize,
    pub text: String,
    pub expanded: bool,
    pub has_children: bool,
    pub details: Vec<String>,
}

// -----------------------------------------------------------------------
// Builder
// -----------------------------------------------------------------------

/// Accumulates `TreeNode`s while walking the database model.
///
/// Methods are spread across submodules (`services`, `layers`) via
/// `impl TreeBuilder` blocks so each concern lives in its own file.
pub struct TreeBuilder {
    nodes: Vec<TreeNode>,
}

impl TreeBuilder {
    fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Push a collapsible/expandable node.
    pub(crate) fn push(
        &mut self,
        depth: usize,
        text: String,
        expanded: bool,
        has_children: bool,
        _id: NodeId,
    ) {
        self.nodes.push(TreeNode {
            depth,
            text,
            expanded,
            has_children,
            details: Vec::new(),
        });
    }

    /// Push a node that also carries detail-pane content.
    pub(crate) fn push_details(
        &mut self,
        depth: usize,
        text: String,
        expanded: bool,
        has_children: bool,
        _id: NodeId,
        details: Vec<String>,
    ) {
        self.nodes.push(TreeNode {
            depth,
            text,
            expanded,
            has_children,
            details,
        });
    }

    /// Convenience: push a leaf node (no children, no details).
    pub(crate) fn push_leaf(&mut self, depth: usize, text: String, id: NodeId) {
        self.push(depth, text, false, false, id);
    }

    fn finish(self) -> Vec<TreeNode> {
        self.nodes
    }
}

/// Identifies a node. Kept around for potential future use (e.g. bookmarks).
pub(crate) enum NodeId {
    Root,
    #[allow(dead_code)]
    Static(String),
}

// -----------------------------------------------------------------------
// Top-level entry point
// -----------------------------------------------------------------------

/// Walk the entire database and produce a flat list of tree nodes ready for
/// the TUI to display.
pub fn build_tree(db: &DiagnosticDatabase) -> Vec<TreeNode> {
    let mut b = TreeBuilder::new();

    let ecu_name = db.ecu_name().unwrap_or_else(|_| "Unknown ECU".into());
    let ecu_details = ecu_summary(db, &ecu_name);

    b.push_details(0, format!("ECU: {ecu_name}"), true, true, NodeId::Root, ecu_details);

    if let Ok(ecu_data) = db.ecu_data() {
        let ecu = EcuDb(*ecu_data);
        add_containers(&mut b, &ecu);
    }

    b.finish()
}

// -----------------------------------------------------------------------
// Container-based structure
// -----------------------------------------------------------------------

fn add_containers(b: &mut TreeBuilder, ecu: &EcuDb<'_>) {
    // Add each variant as a container
    if let Some(variants) = ecu.variants() {
        for (vi, variant) in variants.iter().enumerate() {
            let vw = VariantWrap(variant);
            let name = vw
                .diag_layer()
                .and_then(|l| l.short_name().map(str::to_owned))
                .unwrap_or_else(|| format!("variant_{vi}"));
            let is_base = vw.is_base_variant();

            let details = variant_summary(&vw, &name);
            b.push_details(
                1,
                name.clone(),
                is_base,
                true,
                NodeId::Static(format!("container_{vi}")),
                details,
            );

            // Add Admin Data and Company Datas at container level
            if let Some(dl) = vw.diag_layer() {
                let layer = DiagLayer(dl);
                let layer_name = name.as_str();
                
                b.add_admin_data(&layer, 2, layer_name);
                b.add_company_datas(&layer, 2, layer_name);
            }

            // Add Base Variants section
            if is_base {
                if let Some(dl) = vw.diag_layer() {
                    let layer = DiagLayer(dl);
                    b.push(
                        2,
                        "Base Variants".to_string(),
                        true,
                        true,
                        NodeId::Static(format!("container_{vi}_base_variants")),
                    );
                    b.add_diag_layer_structured(&layer, 3, &name, true);
                }
            } else {
                // For non-base variants, show as ECU Variants
                if let Some(dl) = vw.diag_layer() {
                    let layer = DiagLayer(dl);
                    b.push(
                        2,
                        "ECU Variants".to_string(),
                        false,
                        true,
                        NodeId::Static(format!("container_{vi}_ecu_variants")),
                    );
                    b.add_diag_layer_structured(&layer, 3, &name, false);
                }
            }
        }
    }

    // Add functional groups as containers
    if let Some(groups) = ecu.functional_groups() {
        if !groups.is_empty() {
            for (gi, fg) in groups.iter().enumerate() {
                if let Some(dl) = fg.diag_layer() {
                    let layer = DiagLayer(dl);
                    let name = layer.short_name().unwrap_or("unnamed");
                    
                    b.push(
                        1,
                        format!("{} [functional group]", name),
                        false,
                        true,
                        NodeId::Static(format!("container_fg_{gi}")),
                    );
                    
                    b.add_admin_data(&layer, 2, name);
                    b.add_company_datas(&layer, 2, name);
                    
                    b.push(
                        2,
                        "Functional Group Content".to_string(),
                        false,
                        true,
                        NodeId::Static(format!("container_fg_{gi}_content")),
                    );
                    b.add_diag_layer_structured(&layer, 3, name, false);
                }
            }
        }
    }
}

// -----------------------------------------------------------------------
// ECU-level sections
// -----------------------------------------------------------------------

fn ecu_summary(
    db: &DiagnosticDatabase,
    ecu_name: &str,
) -> Vec<String> {
    let mut d = vec![format!("ECU Name: {ecu_name}")];
    let Ok(ecu_data) = db.ecu_data() else {
        return d;
    };

    if let Some(v) = ecu_data.version() {
        d.push(format!("Version: {v}"));
    }
    if let Some(r) = ecu_data.revision() {
        d.push(format!("Revision: {r}"));
    }
    if let Some(v) = ecu_data.variants() {
        d.push(format!("Variants: {}", v.len()));
    }
    if let Some(fg) = ecu_data.functional_groups() {
        d.push(format!("Functional Groups: {}", fg.len()));
    }
    if let Some(dtcs) = ecu_data.dtcs() {
        d.push(format!("DTCs: {}", dtcs.len()));
    }

    for kv in ecu_data.metadata().into_iter().flatten() {
        if let (Some(k), Some(v)) = (kv.key(), kv.value()) {
            d.push(format!("{k}: {v}"));
        }
    }

    d
}

fn variant_summary(
    variant: &VariantWrap<'_>,
    name: &str,
) -> Vec<String> {
    let mut d = vec![
        format!("Variant: {name}"),
        format!("Base Variant: {}", variant.is_base_variant()),
    ];
    if let Some(dl) = variant.diag_layer() {
        if let Some(val) = dl.long_name().and_then(|ln| ln.value()) {
            d.push(format!("Long Name: {val}"));
        }
        if let Some(svcs) = dl.diag_services() {
            d.push(format!("Services: {}", svcs.len()));
        }
        if let Some(jobs) = dl.single_ecu_jobs() {
            d.push(format!("ECU Jobs: {}", jobs.len()));
        }
    }
    d
}
