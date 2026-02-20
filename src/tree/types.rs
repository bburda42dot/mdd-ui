/// Type of node for styling purposes
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeType {
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

/// A row in a detail table
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
        is_diag_comms: bool,
    },
    /// Multiple subsections within a single tab, each with its own title and content
    Composite(Vec<DetailSectionData>),
}

/// A detail section with a title and content
#[derive(Clone, Debug)]
pub struct DetailSectionData {
    pub title: String,
    pub content: DetailContent,
}

/// Helper to create a plain text detail section
pub fn lines_to_single_section(title: &str, lines: Vec<String>) -> DetailSectionData {
    DetailSectionData {
        title: title.to_owned(),
        content: DetailContent::PlainText(lines),
    }
}
