use cda_database::datatypes::{DiagnosticDatabase, EcuDb};

/// Data extracted from the database, separated from tree building logic
pub struct DatabaseData<'a> {
    pub ecu_name: String,
    pub ecu: Option<EcuDb<'a>>,
}

/// Read and extract data from the database without building tree structure
pub fn extract_data(db: &DiagnosticDatabase) -> DatabaseData<'_> {
    let ecu_name = db.ecu_name().unwrap_or_else(|_| "Unknown ECU".into());
    let ecu = db.ecu_data().ok().map(|ecu_data| EcuDb(*ecu_data));

    DatabaseData { ecu_name, ecu }
}

/// Get ECU summary lines
pub fn get_ecu_summary(db: &DiagnosticDatabase, ecu_name: &str) -> Vec<String> {
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
