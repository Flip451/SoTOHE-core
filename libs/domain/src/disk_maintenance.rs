//! Domain values for configurable disk maintenance.

use std::path::{Component, Path, PathBuf};

use thiserror::Error;

/// Validated positive sccache capacity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheSize(String);

impl CacheSize {
    /// Validate a non-empty decimal capacity followed by a binary-size suffix.
    ///
    /// # Errors
    ///
    /// Returns [`DiskMaintenanceValidationError::InvalidCacheSize`] for an invalid value.
    pub fn try_new(value: String) -> Result<Self, DiskMaintenanceValidationError> {
        let trimmed = value.trim();
        let Some((digits, suffix)) = ['K', 'M', 'G', 'T']
            .into_iter()
            .find_map(|suffix| trimmed.strip_suffix(suffix).map(|digits| (digits, suffix)))
        else {
            return Err(DiskMaintenanceValidationError::InvalidCacheSize(
                InvalidDiskMaintenanceInput::new(value),
            ));
        };
        if digits.is_empty()
            || !digits.bytes().all(|byte| byte.is_ascii_digit())
            || digits.parse::<u64>().ok().filter(|size| *size > 0).is_none()
        {
            return Err(DiskMaintenanceValidationError::InvalidCacheSize(
                InvalidDiskMaintenanceInput::new(value),
            ));
        }
        Ok(Self(format!("{digits}{suffix}")))
    }

    /// Return the normalized capacity string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated cleanup scope relative to the repository root.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CleanupScope(PathBuf);

impl CleanupScope {
    /// Construct a safe relative cleanup scope.
    ///
    /// # Errors
    ///
    /// Returns [`DiskMaintenanceValidationError::InvalidCleanupScope`] for an empty,
    /// absolute, or parent-traversing scope.
    pub fn try_new(value: String) -> Result<Self, DiskMaintenanceValidationError> {
        let path = PathBuf::from(&value);
        if value.trim().is_empty() || path.is_absolute() {
            return Err(DiskMaintenanceValidationError::InvalidCleanupScope(
                InvalidDiskMaintenanceInput::new(value),
            ));
        }

        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                Component::CurDir => {}
                Component::Normal(segment) => {
                    if normalized.as_os_str().is_empty()
                        && segment != "target"
                        && segment != ".cache"
                    {
                        return Err(DiskMaintenanceValidationError::InvalidCleanupScope(
                            InvalidDiskMaintenanceInput::new(value),
                        ));
                    }
                    normalized.push(segment);
                }
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(DiskMaintenanceValidationError::InvalidCleanupScope(
                        InvalidDiskMaintenanceInput::new(value),
                    ));
                }
            }
        }

        if normalized.as_os_str().is_empty() {
            return Err(DiskMaintenanceValidationError::InvalidCleanupScope(
                InvalidDiskMaintenanceInput::new(value),
            ));
        }

        Ok(Self(normalized))
    }

    /// Borrow the relative scope path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// Non-empty, duplicate-free cleanup scopes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupScopeSet {
    scopes: Vec<CleanupScope>,
}

impl CleanupScopeSet {
    /// Construct a non-empty set of distinct scopes.
    ///
    /// # Errors
    ///
    /// Returns the shared validation error for empty or duplicate scopes.
    pub fn try_new(scopes: Vec<CleanupScope>) -> Result<Self, DiskMaintenanceValidationError> {
        let mut seen = std::collections::HashSet::new();
        if scopes.is_empty() {
            return Err(DiskMaintenanceValidationError::EmptyCleanupScopes);
        }
        for scope in &scopes {
            if !seen.insert(scope.clone()) {
                return Err(DiskMaintenanceValidationError::DuplicateCleanupScope(scope.clone()));
            }
        }
        Ok(Self { scopes })
    }

    /// Borrow the configured scopes.
    #[must_use]
    pub fn as_slice(&self) -> &[CleanupScope] {
        &self.scopes
    }
}

/// Immutable validated maintenance configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskMaintenanceConfig {
    max_cache_size: CacheSize,
    cleanup_scopes: CleanupScopeSet,
}

impl DiskMaintenanceConfig {
    /// Aggregate already-validated configuration values.
    #[must_use]
    pub fn new(max_cache_size: CacheSize, cleanup_scopes: CleanupScopeSet) -> Self {
        Self { max_cache_size, cleanup_scopes }
    }
    /// Borrow the configured cache capacity.
    #[must_use]
    pub fn max_cache_size(&self) -> &CacheSize {
        &self.max_cache_size
    }
    /// Borrow the cleanup scopes.
    #[must_use]
    pub fn cleanup_scopes(&self) -> &CleanupScopeSet {
        &self.cleanup_scopes
    }
}

/// Select whether cleanup is only planned or applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupExecutionMode {
    DryRun,
    Apply,
}

/// Original rejected input retained for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InvalidDiskMaintenanceInput(String);
impl InvalidDiskMaintenanceInput {
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque operational detail suitable for rendering at an outer boundary.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiskMaintenanceOperationDetail(String);
impl DiskMaintenanceOperationDetail {
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validation errors for maintenance values.
#[derive(Debug, PartialEq, Eq, Error)]
pub enum DiskMaintenanceValidationError {
    #[error("invalid sccache size: {0:?}")]
    InvalidCacheSize(InvalidDiskMaintenanceInput),
    #[error("invalid cleanup scope: {0:?}")]
    InvalidCleanupScope(InvalidDiskMaintenanceInput),
    #[error("at least one cleanup scope is required")]
    EmptyCleanupScopes,
    #[error("duplicate cleanup scope: {0:?}")]
    DuplicateCleanupScope(CleanupScope),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_size_rejects_invalid_value() -> Result<(), DiskMaintenanceValidationError> {
        assert!(CacheSize::try_new("0G".to_owned()).is_err());
        assert_eq!(CacheSize::try_new("10G".to_owned())?.as_str(), "10G");
        Ok(())
    }

    #[test]
    fn test_cache_size_non_ascii_suffix_returns_validation_error() {
        assert!(CacheSize::try_new("10Ｇ".to_owned()).is_err());
    }

    #[test]
    fn test_cleanup_scope_set_rejects_duplicates() -> Result<(), DiskMaintenanceValidationError> {
        let scope = CleanupScope::try_new("target".to_owned())?;
        assert!(CleanupScopeSet::try_new(vec![scope.clone(), scope]).is_err());
        Ok(())
    }

    #[test]
    fn test_cleanup_scope_normalizes_current_directory_aliases()
    -> Result<(), DiskMaintenanceValidationError> {
        let target = CleanupScope::try_new("target".to_owned())?;
        let target_alias = CleanupScope::try_new("./target/.".to_owned())?;

        assert_eq!(target, target_alias);
        assert!(CleanupScopeSet::try_new(vec![target, target_alias]).is_err());
        Ok(())
    }

    #[test]
    fn test_cleanup_scope_rejects_repository_root_alias() {
        assert!(CleanupScope::try_new(".".to_owned()).is_err());
    }

    #[test]
    fn test_cleanup_scope_disallowed_root_component_returns_validation_error() {
        assert!(CleanupScope::try_new("src".to_owned()).is_err());
        assert!(CleanupScope::try_new(".git".to_owned()).is_err());
    }
}
