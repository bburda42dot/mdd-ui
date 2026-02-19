mod layers;

use cda_database::datatypes::{DiagLayer, DiagnosticDatabase, EcuDb, Variant as VariantWrap};


// -----------------------------------------------------------------------
// Public types
// -----------------------------------------------------------------------

/// Type of node for styling purposes
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeType {
    Ecu,
    Container,
    SectionHeader,
    Service,
    ParentRefService, // Service inherited from parent reference
    Request,
    PosResponse,
    NegResponse,
    Default,
}

/// A single row in the flat tree view. Depth controls indentation, and
/// `expanded` / `has_children` drive the collapse/expand behaviour.
#[derive(Clone)]
pub struct TreeNode {
    pub depth: usize,
    pub text: String,
    pub expanded: bool,
    pub has_children: bool,
    pub detail_sections: Vec<DetailSectionData>,
    pub node_type: NodeType,
}

// -----------------------------------------------------------------------
// Builder
// -----------------------------------------------------------------------

/// Type of cell content for interaction purposes
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CellType {
    /// Regular text cell
    Text,
    /// Cell containing a DOP (Data Object Property) reference
    DopReference,
    /// Cell containing a numeric value
    NumericValue,
    /// Cell containing a parameter name
    ParameterName,
}

/// Accumulates `TreeNode`s while walking the database model.
///
/// Methods are spread across submodules (`services`, `layers`) via
/// `impl TreeBuilder` blocks so each concern lives in its own file.
pub struct TreeBuilder {
    nodes: Vec<TreeNode>,
}

#[derive(Clone, Debug)]
pub struct DetailRow {
    pub cells: Vec<String>,
    pub cell_types: Vec<CellType>,
    pub indent: usize,
}

/// Column constraint for table layout
#[derive(Clone, Debug)]
pub enum ColumnConstraint {
    /// Fixed width in characters
    Fixed(u16),
    /// Percentage of available width
    Percentage(u16),
}

/// Different types of content that can be displayed in a detail section
#[derive(Clone, Debug)]
pub enum DetailContent {
    /// Plain text lines (no table structure)
    PlainText(Vec<String>),
    /// A table with header, data rows, and column constraints
    Table {
        header: DetailRow,
        rows: Vec<DetailRow>,
        constraints: Vec<ColumnConstraint>,
    },
    /// Multiple subsections within a single tab, each with its own title and content
    Composite(Vec<DetailSectionData>),
}

#[derive(Clone, Debug)]
pub struct DetailSectionData {
    pub title: String,
    pub content: DetailContent,
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
        node_type: NodeType,
    ) {
        self.nodes.push(TreeNode {
            depth,
            text,
            expanded,
            has_children,
            detail_sections: Vec::new(),
            node_type,
        });
    }

    /// Push a node that carries structured detail sections (preferred).
    pub(crate) fn push_details_structured(
        &mut self,
        depth: usize,
        text: String,
        expanded: bool,
        has_children: bool,
        _id: NodeId,
        sections: Vec<DetailSectionData>,
        node_type: NodeType,
    ) {
        self.nodes.push(TreeNode {
            depth,
            text,
            expanded,
            has_children,
            detail_sections: sections,
            node_type,
        });
    }

    /// Convenience: push a leaf node (no children, no details).
    pub(crate) fn push_leaf(&mut self, depth: usize, text: String, id: NodeId, node_type: NodeType) {
        self.push(depth, text, false, false, id, node_type);
    }

    fn finish(self) -> Vec<TreeNode> {
        self.nodes
    }
}

fn lines_to_single_section(title: &str, lines: Vec<String>) -> DetailSectionData {
    DetailSectionData {
        title: title.to_owned(),
        content: DetailContent::PlainText(lines),
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
    let ecu_section = lines_to_single_section("Summary", ecu_details.clone());

    b.push_details_structured(0, format!("ECU: {ecu_name}"), true, true, NodeId::Root, vec![ecu_section], NodeType::Ecu);

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
            let sec = lines_to_single_section("Summary", details.clone());
            b.push_details_structured(
                1,
                name.clone(),
                is_base,
                true,
                NodeId::Static(format!("container_{vi}")),
                vec![sec],
                NodeType::Container,
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
                        NodeType::SectionHeader,
                    );
                    // Pass parent refs from variant for inherited service lookup
                    let parent_refs_iter = vw.parent_refs().map(|pr| pr.iter().map(cda_database::datatypes::ParentRef));
                    b.add_diag_layer_structured(&layer, 3, &name, true, parent_refs_iter);
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
                        NodeType::SectionHeader,
                    );
                    // Pass parent refs from variant for inherited service lookup
                    let parent_refs_iter = vw.parent_refs().map(|pr| pr.iter().map(cda_database::datatypes::ParentRef));
                    b.add_diag_layer_structured(&layer, 3, &name, false, parent_refs_iter);
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
                        NodeId::Static(format!("fg_{gi}")),
                        NodeType::Container,
                    );
                    
                    b.add_admin_data(&layer, 2, name);
                    b.add_company_datas(&layer, 2, name);
                    
                    b.push(
                        2,
                        "Functional Group Content".to_string(),
                        false,
                        true,
                        NodeId::Static(format!("container_fg_{gi}_content")),
                        NodeType::SectionHeader,
                    );
                    // Functional groups don't have parent refs like variants
                    b.add_diag_layer_structured(&layer, 3, name, false, None::<std::iter::Empty<cda_database::datatypes::ParentRef>>);
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
