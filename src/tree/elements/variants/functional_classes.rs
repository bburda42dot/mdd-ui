use cda_database::datatypes::DiagLayer;

use crate::tree::{
    builder::TreeBuilder,
    types::{
        CellType, ColumnConstraint, DetailContent, DetailRow, DetailSectionData,
        DetailSectionType, NodeType,
    },
};

/// Represents a functional class item (either a service or a job)
#[derive(Clone, Debug)]
enum FunctionalClassItem {
    Service(String),
    Job(String),
}

impl FunctionalClassItem {
    fn name(&self) -> &str {
        match self {
            FunctionalClassItem::Service(name) | FunctionalClassItem::Job(name) => name,
        }
    }

    fn item_type(&self) -> &str {
        match self {
            FunctionalClassItem::Service(_) => "Service",
            FunctionalClassItem::Job(_) => "Job",
        }
    }
}

/// Add functional classes section from the diagnostic layer
pub fn add_functional_classes(b: &mut TreeBuilder, layer: &DiagLayer<'_>, depth: usize) {
    // Collect services
    let services: Vec<FunctionalClassItem> = layer
        .diag_services()
        .map(|services| {
            services
                .iter()
                .map(|s| {
                    let ds = cda_database::datatypes::DiagService(s);
                    let name = ds
                        .diag_comm()
                        .and_then(|dc| dc.short_name())
                        .unwrap_or("Unnamed")
                        .to_owned();
                    FunctionalClassItem::Service(name)
                })
                .collect()
        })
        .unwrap_or_default();

    // Collect jobs
    let jobs: Vec<FunctionalClassItem> = layer
        .single_ecu_jobs()
        .map(|jobs| {
            jobs.iter()
                .map(|job| {
                    let name = job
                        .diag_comm()
                        .and_then(|dc| dc.short_name())
                        .unwrap_or("Unnamed")
                        .to_owned();
                    FunctionalClassItem::Job(name)
                })
                .collect()
        })
        .unwrap_or_default();

    // Combine services and jobs
    let mut items = services;
    items.extend(jobs);

    if items.is_empty() {
        return;
    }

    // Build table section for the Functional Classes header
    let detail_section = build_functional_classes_table_section(&items);

    let total_count = items.len();

    b.push_service_list_header(
        depth,
        format!("Functional Classes ({})", total_count),
        false,
        true,
        vec![detail_section],
        crate::tree::ServiceListType::FunctionalClasses,
    );

    // Add each functional class as a child node with details
    for item in items.iter() {
        let sections = build_functional_class_details(item);

        b.push_details_structured(
            depth + 1,
            item.name().to_owned(),
            false,
            false,
            sections,
            NodeType::Default,
        );
    }
}

/// Build a table section for the Functional Classes header showing all classes
fn build_functional_classes_table_section(items: &[FunctionalClassItem]) -> DetailSectionData {
    let header = DetailRow::header(
        vec!["Short Name".to_owned()],
        vec![CellType::Text],
    );

    let mut rows = Vec::new();

    // Add each functional class to the table
    for item in items.iter() {
        rows.push(DetailRow::normal(
            vec![item.name().to_owned()],
            vec![CellType::Text],
            0,
        ));
    }

    DetailSectionData::new(
        "Overview".to_owned(),
        DetailContent::Table {
            header,
            rows,
            constraints: vec![ColumnConstraint::Percentage(100)],
            use_row_selection: true,
        },
        false,
    )
    .with_type(DetailSectionType::FunctionalClass)
}

/// Build detailed sections for a functional class (dummy implementation)
fn build_functional_class_details(item: &FunctionalClassItem) -> Vec<DetailSectionData> {
    let mut sections = Vec::new();

    // Add header section with the name
    sections.push(DetailSectionData {
        title: format!("Functional Class - {}", item.name()),
        render_as_header: true,
        content: DetailContent::PlainText(vec![]),
        section_type: DetailSectionType::Header,
    });

    // Overview tab (dummy content)
    let header = DetailRow::header(
        vec!["Property".to_owned(), "Value".to_owned()],
        vec![CellType::Text, CellType::Text],
    );

    let rows = vec![
        DetailRow::normal(
            vec!["Name".to_owned(), item.name().to_owned()],
            vec![CellType::Text, CellType::Text],
            0,
        ),
        DetailRow::normal(
            vec!["Type".to_owned(), item.item_type().to_owned()],
            vec![CellType::Text, CellType::Text],
            0,
        ),
        DetailRow::normal(
            vec!["Status".to_owned(), "Active".to_owned()],
            vec![CellType::Text, CellType::Text],
            0,
        ),
    ];

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
                use_row_selection: false,
            },
            false,
        )
        .with_type(DetailSectionType::Overview),
    );

    // Services tab - List of services in this functional class
    let service_header = DetailRow::header(
        vec!["Short Name".to_owned()],
        vec![CellType::Text],
    );

    let service_rows = vec![
        // Placeholder row - in a real implementation, this would query
        // the services associated with this functional class
        DetailRow::normal(
            vec!["No services available yet".to_owned()],
            vec![CellType::Text],
            0,
        ),
    ];

    sections.push(
        DetailSectionData::new(
            "Services".to_owned(),
            DetailContent::Table {
                header: service_header,
                rows: service_rows,
                constraints: vec![ColumnConstraint::Percentage(100)],
                use_row_selection: true,
            },
            false,
        )
        .with_type(DetailSectionType::Services),
    );

    // Details tab (dummy content)
    sections.push(DetailSectionData::new(
        "Details".to_owned(),
        DetailContent::PlainText(vec![
            format!(
                "Functional class {} details will be displayed here.",
                item.item_type().to_lowercase()
            ),
            String::new(),
            "This is a placeholder implementation.".to_owned(),
        ]),
        false,
    ));

    sections
}
