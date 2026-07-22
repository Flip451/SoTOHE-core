//! Primary adapters for disk-maintenance commands and queries.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::disk_maintenance::{
    CleanupPlanQuery, DiskMaintenanceCommand, DiskMaintenanceCommandResponse,
    DiskMaintenanceCommandService, DiskMaintenanceQueryService,
};

use crate::render::CommandOutcome;

/// Parsed input for mutating disk-maintenance operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaintenanceCommandInput {
    ConfigureSccache { project_root: PathBuf },
    ApplyCleanup { project_root: PathBuf },
}

/// Parsed input for read-only disk-maintenance operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaintenanceQueryInput {
    PlanCleanup { project_root: PathBuf },
}

/// Primary adapter for mutating disk-maintenance operations.
pub struct MaintenanceCommandDriver {
    command_service: Arc<dyn DiskMaintenanceCommandService>,
}

impl MaintenanceCommandDriver {
    #[must_use]
    pub fn new(command_service: Arc<dyn DiskMaintenanceCommandService>) -> Self {
        Self { command_service }
    }

    #[must_use]
    pub fn handle(&self, input: MaintenanceCommandInput) -> CommandOutcome {
        match input {
            MaintenanceCommandInput::ConfigureSccache { project_root } => match self
                .command_service
                .execute_command(DiskMaintenanceCommand::ConfigureSccache { project_root })
            {
                Ok(DiskMaintenanceCommandResponse::SccacheConfigured(size)) => {
                    CommandOutcome::success(Some(format!(
                        "sccache cache size configured: {}",
                        size.as_str()
                    )))
                }
                Ok(_) => CommandOutcome::failure(Some(
                    "unexpected maintenance command response".to_owned(),
                )),
                Err(error) => CommandOutcome::failure(Some(error.to_string())),
            },
            MaintenanceCommandInput::ApplyCleanup { project_root } => match self
                .command_service
                .execute_command(DiskMaintenanceCommand::ApplyCleanup { project_root })
            {
                Ok(DiskMaintenanceCommandResponse::CleanupApplied(scopes)) => {
                    CommandOutcome::success(Some(format!(
                        "cleanup applied: {}",
                        scopes
                            .as_slice()
                            .iter()
                            .map(|scope| scope.as_path().display().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )))
                }
                Ok(_) => CommandOutcome::failure(Some(
                    "unexpected maintenance command response".to_owned(),
                )),
                Err(error) => CommandOutcome::failure(Some(error.to_string())),
            },
        }
    }
}

/// Primary adapter for read-only disk-maintenance operations.
pub struct MaintenanceQueryDriver {
    query_service: Arc<dyn DiskMaintenanceQueryService>,
}

impl MaintenanceQueryDriver {
    #[must_use]
    pub fn new(query_service: Arc<dyn DiskMaintenanceQueryService>) -> Self {
        Self { query_service }
    }

    #[must_use]
    pub fn handle(&self, input: MaintenanceQueryInput) -> CommandOutcome {
        match input {
            MaintenanceQueryInput::PlanCleanup { project_root } => {
                match self.query_service.plan_cleanup(CleanupPlanQuery { project_root }) {
                    Ok(response) => CommandOutcome::success(Some(format!(
                        "cleanup plan: {}",
                        response
                            .scopes
                            .as_slice()
                            .iter()
                            .map(|scope| scope.as_path().display().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))),
                    Err(error) => CommandOutcome::failure(Some(error.to_string())),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use usecase::disk_maintenance::{CleanupPlanResponse, DiskMaintenanceError};

    struct RecordingCommandService {
        command: Mutex<Option<DiskMaintenanceCommand>>,
    }

    impl DiskMaintenanceCommandService for RecordingCommandService {
        fn execute_command(
            &self,
            command: DiskMaintenanceCommand,
        ) -> Result<DiskMaintenanceCommandResponse, DiskMaintenanceError> {
            *self.command.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(command);
            Err(DiskMaintenanceError::new("command failure"))
        }
    }

    struct RecordingQueryService {
        query: Mutex<Option<CleanupPlanQuery>>,
    }

    impl DiskMaintenanceQueryService for RecordingQueryService {
        fn plan_cleanup(
            &self,
            query: CleanupPlanQuery,
        ) -> Result<CleanupPlanResponse, DiskMaintenanceError> {
            *self.query.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(query);
            Err(DiskMaintenanceError::new("query failure"))
        }
    }

    #[test]
    fn test_command_driver_routes_apply_input_to_its_single_service() {
        let service = Arc::new(RecordingCommandService { command: Mutex::new(None) });
        let driver = MaintenanceCommandDriver::new(service.clone());
        let project_root = PathBuf::from("project");

        let outcome = driver
            .handle(MaintenanceCommandInput::ApplyCleanup { project_root: project_root.clone() });

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(
            *service.command.lock().unwrap_or_else(|poisoned| poisoned.into_inner()),
            Some(DiskMaintenanceCommand::ApplyCleanup { project_root })
        );
    }

    #[test]
    fn test_query_driver_routes_plan_input_to_its_single_service() {
        let service = Arc::new(RecordingQueryService { query: Mutex::new(None) });
        let driver = MaintenanceQueryDriver::new(service.clone());
        let project_root = PathBuf::from("project");

        let outcome = driver
            .handle(MaintenanceQueryInput::PlanCleanup { project_root: project_root.clone() });

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(
            *service.query.lock().unwrap_or_else(|poisoned| poisoned.into_inner()),
            Some(CleanupPlanQuery { project_root })
        );
    }
}
