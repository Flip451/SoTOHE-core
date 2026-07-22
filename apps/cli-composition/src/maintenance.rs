//! Composition root for disk maintenance.

use std::sync::Arc;

use cli_driver::maintenance::{MaintenanceCommandDriver, MaintenanceQueryDriver};
use infrastructure::disk_maintenance::FsDiskMaintenanceAdapter;
use usecase::disk_maintenance::{DiskMaintenanceCommandInteractor, DiskMaintenanceQueryInteractor};

/// Wire-only composition root for disk maintenance.
pub struct MaintenanceCompositionRoot;

impl MaintenanceCompositionRoot {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    #[must_use]
    pub fn maintenance_command_driver(&self) -> MaintenanceCommandDriver {
        let adapter = Arc::new(FsDiskMaintenanceAdapter::new());
        let command = Arc::new(DiskMaintenanceCommandInteractor::new(adapter));
        MaintenanceCommandDriver::new(command)
    }

    #[must_use]
    pub fn maintenance_query_driver(&self) -> MaintenanceQueryDriver {
        let adapter = Arc::new(FsDiskMaintenanceAdapter::new());
        let query = Arc::new(DiskMaintenanceQueryInteractor::new(adapter));
        MaintenanceQueryDriver::new(query)
    }
}
impl Default for MaintenanceCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_composition_root_wires_distinct_maintenance_drivers() {
        let root = MaintenanceCompositionRoot::new();
        let _command = root.maintenance_command_driver();
        let _query = root.maintenance_query_driver();
    }
}
