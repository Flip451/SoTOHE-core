//! Application services for disk-maintenance commands and queries.

use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use domain::disk_maintenance::CleanupExecutionMode;
use domain::disk_maintenance::{
    CacheSize, CleanupScopeSet, DiskMaintenanceOperationDetail, DiskMaintenanceValidationError,
};

/// Mutating maintenance intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiskMaintenanceCommand {
    ConfigureSccache { project_root: PathBuf },
    ApplyCleanup { project_root: PathBuf },
}

/// Result of a maintenance command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiskMaintenanceCommandResponse {
    SccacheConfigured(CacheSize),
    CleanupApplied(CleanupScopeSet),
}

/// Read-only cleanup-plan query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupPlanQuery {
    pub project_root: PathBuf,
}

/// Read-only cleanup-plan response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupPlanResponse {
    pub scopes: CleanupScopeSet,
}

/// User-safe maintenance failure.
#[derive(Debug)]
pub enum DiskMaintenanceError {
    Validation(DiskMaintenanceValidationError),
    Operation(DiskMaintenanceOperationDetail),
}

impl DiskMaintenanceError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self::Operation(DiskMaintenanceOperationDetail::new(message.into()))
    }
}

impl From<DiskMaintenanceValidationError> for DiskMaintenanceError {
    fn from(value: DiskMaintenanceValidationError) -> Self {
        Self::Validation(value)
    }
}

impl std::fmt::Display for DiskMaintenanceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(error) => error.fmt(formatter),
            Self::Operation(detail) => formatter.write_str(detail.as_str()),
        }
    }
}
impl std::error::Error for DiskMaintenanceError {}

/// Synchronous mutating disk-maintenance boundary.
pub trait DiskMaintenanceCommandPort: Send + Sync {
    fn configure_sccache(&self, project_root: &Path) -> Result<CacheSize, DiskMaintenanceError>;
    fn apply_cleanup(&self, project_root: PathBuf)
    -> Result<CleanupScopeSet, DiskMaintenanceError>;
}

/// Synchronous read-only disk-maintenance boundary.
pub trait DiskMaintenanceQueryPort: Send + Sync {
    fn plan_cleanup(&self, project_root: &Path) -> Result<CleanupScopeSet, DiskMaintenanceError>;
}

/// Mutating maintenance application service.
pub trait DiskMaintenanceCommandService: Send + Sync {
    fn execute_command(
        &self,
        command: DiskMaintenanceCommand,
    ) -> Result<DiskMaintenanceCommandResponse, DiskMaintenanceError>;
}

/// Read-only maintenance application service.
pub trait DiskMaintenanceQueryService: Send + Sync {
    fn plan_cleanup(
        &self,
        query: CleanupPlanQuery,
    ) -> Result<CleanupPlanResponse, DiskMaintenanceError>;
}

/// Command interactor.
pub struct DiskMaintenanceCommandInteractor {
    port: Arc<dyn DiskMaintenanceCommandPort>,
}
impl DiskMaintenanceCommandInteractor {
    #[must_use]
    pub fn new(port: Arc<dyn DiskMaintenanceCommandPort>) -> Self {
        Self { port }
    }
}
impl DiskMaintenanceCommandService for DiskMaintenanceCommandInteractor {
    fn execute_command(
        &self,
        command: DiskMaintenanceCommand,
    ) -> Result<DiskMaintenanceCommandResponse, DiskMaintenanceError> {
        match command {
            DiskMaintenanceCommand::ConfigureSccache { project_root } => self
                .port
                .configure_sccache(&project_root)
                .map(DiskMaintenanceCommandResponse::SccacheConfigured),
            DiskMaintenanceCommand::ApplyCleanup { project_root } => self
                .port
                .apply_cleanup(project_root)
                .map(DiskMaintenanceCommandResponse::CleanupApplied),
        }
    }
}

/// Query interactor.
pub struct DiskMaintenanceQueryInteractor {
    port: Arc<dyn DiskMaintenanceQueryPort>,
}
impl DiskMaintenanceQueryInteractor {
    #[must_use]
    pub fn new(port: Arc<dyn DiskMaintenanceQueryPort>) -> Self {
        Self { port }
    }
}
impl DiskMaintenanceQueryService for DiskMaintenanceQueryInteractor {
    fn plan_cleanup(
        &self,
        query: CleanupPlanQuery,
    ) -> Result<CleanupPlanResponse, DiskMaintenanceError> {
        self.port.plan_cleanup(&query.project_root).map(|scopes| CleanupPlanResponse { scopes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::disk_maintenance::{CleanupScope, DiskMaintenanceConfig};

    struct FakePort {
        config: DiskMaintenanceConfig,
    }
    impl DiskMaintenanceCommandPort for FakePort {
        fn configure_sccache(&self, _: &Path) -> Result<CacheSize, DiskMaintenanceError> {
            Ok(self.config.max_cache_size().clone())
        }
        fn apply_cleanup(&self, _: PathBuf) -> Result<CleanupScopeSet, DiskMaintenanceError> {
            Ok(self.config.cleanup_scopes().clone())
        }
    }
    impl DiskMaintenanceQueryPort for FakePort {
        fn plan_cleanup(&self, _: &Path) -> Result<CleanupScopeSet, DiskMaintenanceError> {
            Ok(self.config.cleanup_scopes().clone())
        }
    }
    fn port() -> Result<Arc<FakePort>, DiskMaintenanceError> {
        let scopes = CleanupScopeSet::try_new(vec![CleanupScope::try_new("target".to_owned())?])?;
        Ok(Arc::new(FakePort {
            config: DiskMaintenanceConfig::new(CacheSize::try_new("1G".to_owned())?, scopes),
        }))
    }
    #[test]
    fn test_command_interactor_configures_sccache() -> Result<(), DiskMaintenanceError> {
        let service = DiskMaintenanceCommandInteractor::new(port()?);
        assert!(matches!(
            service.execute_command(DiskMaintenanceCommand::ConfigureSccache {
                project_root: PathBuf::from(".")
            }),
            Ok(DiskMaintenanceCommandResponse::SccacheConfigured(_))
        ));
        Ok(())
    }
    #[test]
    fn test_query_interactor_returns_plan_without_command() -> Result<(), DiskMaintenanceError> {
        let service = DiskMaintenanceQueryInteractor::new(port()?);
        assert_eq!(
            service
                .plan_cleanup(CleanupPlanQuery { project_root: PathBuf::from(".") })?
                .scopes
                .as_slice()
                .len(),
            1
        );
        Ok(())
    }
}
