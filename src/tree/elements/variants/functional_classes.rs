use cda_database::datatypes::{DiagLayer, DiagService};

// Import the detail building functions from services module
use super::services::{build_diag_comm_details_with_parent, build_simple_job_details_with_name};
use crate::tree::{
    builder::TreeBuilder,
    types::{
        CellType, ColumnConstraint, DetailContent, DetailRow, DetailSectionData, DetailSectionType,
        NodeType,
    },
};

/// Represents a functional class item (either a service or a job)
#[derive(Clone, Debug)]
enum FunctionalClassItem<'a> {
    Service(DiagService<'a>),
    Job(String),
}

impl<'a> FunctionalClassItem<'a> {
    fn name(&self) -> String {
        match self {
            FunctionalClassItem::Service(ds) => ds
                .diag_comm()
                .and_then(|dc| dc.short_name())
                .unwrap_or("?")
                .to_owned(),
            FunctionalClassItem::Job(name) => name.clone(),
        }
    }

    fn item_type(&self) -> &str {
        match self {
            FunctionalClassItem::Service(_) => "Service",
            FunctionalClassItem::Job(_) => "Job",
        }
    }

    fn id(&self) -> String {
        match self {
            FunctionalClassItem::Service(ds) => {
                if let Some(sid) = ds.request_id() {
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
                }
            }
            FunctionalClassItem::Job(_) => "-".to_owned(),
        }
    }

    fn display_name(&self) -> String {
        match self {
            FunctionalClassItem::Service(ds) => {
                let name = self.name();
                // Format with service ID with proper padding for alignment
                if let Some(sid) = ds.request_id() {
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
                    name
                }
            }
            FunctionalClassItem::Job(name) => name.clone(),
        }
    }

    fn build_details(&self) -> Vec<DetailSectionData> {
        match self {
            FunctionalClassItem::Service(ds) => {
                // Reuse the exact same detail view as diag-comm services
                build_diag_comm_details_with_parent(ds, None)
            }
            FunctionalClassItem::Job(name) => {
                // Reuse the exact same detail view as diag-comm jobs
                build_simple_job_details_with_name(name)
            }
        }
    }
}

/// Add functional classes section from the diagnostic layer
pub fn add_functional_classes(b: &mut TreeBuilder, layer: &DiagLayer<'_>, depth: usize) {
    // Collect services - store actual DiagService references
    let services: Vec<FunctionalClassItem<'_>> = layer
        .diag_services()
        .map(|services| {
            services
                .iter()
                .map(|s| FunctionalClassItem::Service(DiagService(s)))
                .collect()
        })
        .unwrap_or_default();

    // Collect jobs - store job names
    let jobs: Vec<FunctionalClassItem<'_>> = layer
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

    // Count services and jobs separately
    let service_count = services.len();
    let job_count = jobs.len();

    // Combine services and jobs
    let mut items = services;
    items.extend(jobs);

    if items.is_empty() {
        return;
    }

    // Build table section for the Functional Classes header
    let detail_section = build_functional_classes_table_section(&items, service_count, job_count);

    let total_count = service_count + job_count;

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
        // Use the exact same detail views as diag-comm
        let sections = item.build_details();

        let node_type = match item {
            FunctionalClassItem::Service(_) => NodeType::Service,
            FunctionalClassItem::Job(_) => NodeType::Default,
        };

        let prefix = match item {
            FunctionalClassItem::Service(_) => "[Service] ",
            FunctionalClassItem::Job(_) => "[Job] ",
        };

        b.push_details_structured(
            depth + 1,
            format!("{}{}", prefix, item.display_name()),
            false,
            false,
            sections,
            node_type,
        );
    }
}

/// Build a table section for the Functional Classes header showing all classes
fn build_functional_classes_table_section(
    items: &[FunctionalClassItem],
    service_count: usize,
    job_count: usize,
) -> DetailSectionData {
    let header = DetailRow::header(
        vec![
            "ID".to_owned(),
            "Short Name".to_owned(),
            "Type".to_owned(),
            "Inherited".to_owned(),
        ],
        vec![
            CellType::Text,
            CellType::Text,
            CellType::Text,
            CellType::Text,
        ],
    );

    let mut rows = Vec::new();

    // Add each functional class to the table
    for item in items.iter() {
        rows.push(DetailRow::normal(
            vec![
                item.id(),
                item.name().to_owned(),
                item.item_type().to_owned(),
                "false".to_owned(),
            ],
            vec![
                CellType::Text,
                CellType::Text,
                CellType::Text,
                CellType::Text,
            ],
            0,
        ));
    }

    DetailSectionData::new(
        format!(
            "Functional Classes ({} services, {} jobs)",
            service_count, job_count
        ),
        DetailContent::Table {
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
        false,
    )
    .with_type(DetailSectionType::FunctionalClass)
}
