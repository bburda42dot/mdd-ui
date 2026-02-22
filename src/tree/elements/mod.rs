pub mod layers;
pub mod variants;
pub mod dtcs;

// Re-export commonly used items
pub use variants::{add_ecu_shared_data, add_functional_groups, add_protocols, add_variants};
pub use dtcs::add_dtcs;
