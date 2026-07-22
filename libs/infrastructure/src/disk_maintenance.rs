//! Filesystem adapter for configurable disk maintenance.

use std::fs;
use std::io::{Error, ErrorKind, Read};
use std::path::{Path, PathBuf};

use domain::disk_maintenance::{CacheSize, CleanupScope, CleanupScopeSet, DiskMaintenanceConfig};
use serde::Deserialize;
use usecase::disk_maintenance::{
    DiskMaintenanceCommandPort, DiskMaintenanceError, DiskMaintenanceQueryPort,
};

use crate::track::symlink_guard::{reject_symlinks_below, reject_symlinks_up_to_root};

const MAX_DISK_MAINTENANCE_CONFIG_BYTES: u64 = 1024 * 1024;

/// Disk-maintenance filesystem adapter.
#[derive(Debug, Default)]
pub struct FsDiskMaintenanceAdapter;

#[derive(Deserialize)]
struct DiskMaintenanceFile {
    cache: CacheConfig,
    cleanup: CleanupConfig,
}
#[derive(Deserialize)]
struct CacheConfig {
    max_size: String,
}
#[derive(Deserialize)]
struct CleanupConfig {
    scopes: Vec<String>,
}

impl FsDiskMaintenanceAdapter {
    /// Create the adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn config_path(project_root: &Path) -> Result<PathBuf, DiskMaintenanceError> {
        let metadata = project_root.symlink_metadata().map_err(|error| {
            DiskMaintenanceError::new(format!(
                "cannot inspect project root {}: {error}",
                project_root.display()
            ))
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(DiskMaintenanceError::new(format!(
                "project root is not a directory: {}",
                project_root.display()
            )));
        }
        let path = project_root.join(".harness/config/disk-maintenance.toml");
        Self::reject_symlinks(&path, project_root, "disk-maintenance configuration")?;
        Ok(path)
    }

    fn reject_symlinks(
        path: &Path,
        project_root: &Path,
        label: &str,
    ) -> Result<bool, DiskMaintenanceError> {
        reject_symlinks_up_to_root(project_root).map_err(|error| {
            DiskMaintenanceError::new(format!(
                "refusing to use {label} {}: {error}",
                path.display()
            ))
        })?;
        reject_symlinks_below(path, project_root).map_err(|error| {
            DiskMaintenanceError::new(format!(
                "refusing to use {label} {}: {error}",
                path.display()
            ))
        })
    }

    fn read_config_file(path: &Path) -> Result<String, std::io::Error> {
        let metadata = path.symlink_metadata()?;
        if !metadata.file_type().is_file() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("{} is not a regular file", path.display()),
            ));
        }
        let file = fs::File::open(path)?;
        let opened_metadata = file.metadata()?;
        if !opened_metadata.is_file() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("{} is not a regular file", path.display()),
            ));
        }
        if opened_metadata.len() > MAX_DISK_MAINTENANCE_CONFIG_BYTES {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "{} exceeds the maximum allowed size of {MAX_DISK_MAINTENANCE_CONFIG_BYTES} bytes",
                    path.display()
                ),
            ));
        }

        let mut content = String::new();
        file.take(MAX_DISK_MAINTENANCE_CONFIG_BYTES.saturating_add(1))
            .read_to_string(&mut content)?;
        if content.len() > MAX_DISK_MAINTENANCE_CONFIG_BYTES as usize {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "{} exceeds the maximum allowed size of {MAX_DISK_MAINTENANCE_CONFIG_BYTES} bytes",
                    path.display()
                ),
            ));
        }
        Ok(content)
    }

    fn load_config(project_root: &Path) -> Result<DiskMaintenanceConfig, DiskMaintenanceError> {
        let path = Self::config_path(project_root)?;
        let raw = Self::read_config_file(&path).map_err(|error| {
            DiskMaintenanceError::new(format!("cannot read {}: {error}", path.display()))
        })?;
        let file: DiskMaintenanceFile = toml::from_str(&raw).map_err(|error| {
            DiskMaintenanceError::new(format!("invalid {}: {error}", path.display()))
        })?;
        let max_cache_size = CacheSize::try_new(file.cache.max_size)?;
        let mut scopes = Vec::with_capacity(file.cleanup.scopes.len());
        for scope in file.cleanup.scopes {
            scopes.push(CleanupScope::try_new(scope)?);
        }
        Ok(DiskMaintenanceConfig::new(max_cache_size, CleanupScopeSet::try_new(scopes)?))
    }

    fn cleanup_root(
        project_root: &Path,
        scope: &CleanupScope,
    ) -> Result<PathBuf, DiskMaintenanceError> {
        let root = project_root.join(scope.as_path());
        if !root.starts_with(project_root) {
            return Err(DiskMaintenanceError::new("cleanup scope escapes project root"));
        }
        Ok(root)
    }

    fn empty_directory(project_root: &Path, root: &Path) -> Result<(), DiskMaintenanceError> {
        Self::reject_symlinks(root, project_root, "cleanup root")?;
        fs::create_dir_all(root).map_err(|error| {
            DiskMaintenanceError::new(format!("cannot create {}: {error}", root.display()))
        })?;
        Self::reject_symlinks(root, project_root, "cleanup root")?;
        for entry in fs::read_dir(root).map_err(|error| {
            DiskMaintenanceError::new(format!("cannot read {}: {error}", root.display()))
        })? {
            let entry = entry.map_err(|error| {
                DiskMaintenanceError::new(format!("cannot read cleanup entry: {error}"))
            })?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|error| {
                DiskMaintenanceError::new(format!("cannot inspect {}: {error}", path.display()))
            })?;
            if kind.is_dir() && !kind.is_symlink() {
                fs::remove_dir_all(&path).map_err(|error| {
                    DiskMaintenanceError::new(format!("cannot remove {}: {error}", path.display()))
                })?;
            } else {
                fs::remove_file(&path).map_err(|error| {
                    DiskMaintenanceError::new(format!("cannot remove {}: {error}", path.display()))
                })?;
            }
        }
        Ok(())
    }
}

impl DiskMaintenanceQueryPort for FsDiskMaintenanceAdapter {
    fn plan_cleanup(&self, project_root: &Path) -> Result<CleanupScopeSet, DiskMaintenanceError> {
        Ok(Self::load_config(project_root)?.cleanup_scopes().clone())
    }
}

impl DiskMaintenanceCommandPort for FsDiskMaintenanceAdapter {
    fn configure_sccache(&self, project_root: &Path) -> Result<CacheSize, DiskMaintenanceError> {
        let config = Self::load_config(project_root)?;
        let cache_dir = project_root.join(".cache");
        Self::reject_symlinks(&cache_dir, project_root, "sccache cache directory")?;
        fs::create_dir_all(&cache_dir).map_err(|error| {
            DiskMaintenanceError::new(format!("cannot create {}: {error}", cache_dir.display()))
        })?;
        Self::reject_symlinks(&cache_dir, project_root, "sccache cache directory")?;
        let env_path = cache_dir.join("sccache.env");
        Self::reject_symlinks(&env_path, project_root, "sccache environment file")?;
        fs::write(&env_path, format!("SCCACHE_CACHE_SIZE={}\n", config.max_cache_size().as_str()))
            .map_err(|error| {
                DiskMaintenanceError::new(format!("cannot write {}: {error}", env_path.display()))
            })?;
        Ok(config.max_cache_size().clone())
    }

    fn apply_cleanup(
        &self,
        project_root: PathBuf,
    ) -> Result<CleanupScopeSet, DiskMaintenanceError> {
        let config = Self::load_config(&project_root)?;
        for scope in config.cleanup_scopes().as_slice() {
            Self::empty_directory(&project_root, &Self::cleanup_root(&project_root, scope)?)?;
        }
        Ok(config.cleanup_scopes().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_config(root: &Path, scopes: &str) -> std::io::Result<()> {
        let config = root.join(".harness/config");
        fs::create_dir_all(&config)?;
        fs::write(
            config.join("disk-maintenance.toml"),
            format!("[cache]\nmax_size = \"2G\"\n[cleanup]\nscopes = [{scopes}]\n"),
        )?;
        Ok(())
    }

    #[test]
    fn test_configure_sccache_writes_configured_limit() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        write_config(root.path(), "\"target\"")?;
        let size = FsDiskMaintenanceAdapter::new().configure_sccache(root.path())?;
        assert_eq!(size.as_str(), "2G");
        assert_eq!(
            fs::read_to_string(root.path().join(".cache/sccache.env"))?,
            "SCCACHE_CACHE_SIZE=2G\n"
        );
        Ok(())
    }

    #[test]
    fn test_cleanup_removes_contents_but_preserves_configured_roots()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        write_config(root.path(), "\"target\", \".cache\"")?;
        fs::create_dir_all(root.path().join("target/nested"))?;
        fs::write(root.path().join("target/nested/file"), "x")?;
        fs::create_dir_all(root.path().join(".cache"))?;
        fs::write(root.path().join(".cache/file"), "x")?;
        FsDiskMaintenanceAdapter::new().apply_cleanup(root.path().to_path_buf())?;
        assert!(root.path().join("target").is_dir());
        assert!(root.path().join(".cache").is_dir());
        assert!(fs::read_dir(root.path().join("target"))?.next().is_none());
        assert!(fs::read_dir(root.path().join(".cache"))?.next().is_none());
        Ok(())
    }

    #[test]
    fn test_plan_cleanup_rejects_invalid_config() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        write_config(root.path(), "\"../outside\"")?;
        assert!(FsDiskMaintenanceAdapter::new().plan_cleanup(root.path()).is_err());
        Ok(())
    }

    #[test]
    fn test_plan_cleanup_oversized_config_returns_error() -> Result<(), Box<dyn std::error::Error>>
    {
        let root = tempfile::tempdir()?;
        let config_dir = root.path().join(".harness/config");
        fs::create_dir_all(&config_dir)?;
        fs::File::create(config_dir.join("disk-maintenance.toml"))?
            .set_len(MAX_DISK_MAINTENANCE_CONFIG_BYTES.saturating_add(1))?;

        assert!(FsDiskMaintenanceAdapter::new().plan_cleanup(root.path()).is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_plan_cleanup_symlinked_config_ancestor_returns_error()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        fs::create_dir_all(outside.path().join("config"))?;
        fs::write(
            outside.path().join("config/disk-maintenance.toml"),
            "[cache]\nmax_size = \"2G\"\n[cleanup]\nscopes = [\"target\"]\n",
        )?;
        std::os::unix::fs::symlink(outside.path(), root.path().join(".harness"))?;

        assert!(FsDiskMaintenanceAdapter::new().plan_cleanup(root.path()).is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_plan_cleanup_symlinked_project_root_ancestor_returns_error()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let real_parent = root.path().join("real-parent");
        let project_root = real_parent.join("project");
        fs::create_dir_all(&project_root)?;
        write_config(&project_root, "\"target\"")?;
        let linked_parent = root.path().join("linked-parent");
        std::os::unix::fs::symlink(&real_parent, &linked_parent)?;

        assert!(
            FsDiskMaintenanceAdapter::new().plan_cleanup(&linked_parent.join("project")).is_err()
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_configure_sccache_symlinked_cache_returns_error()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        write_config(root.path(), "\"target\"")?;
        let sentinel = outside.path().join("sccache.env");
        fs::write(&sentinel, "unchanged")?;
        std::os::unix::fs::symlink(outside.path(), root.path().join(".cache"))?;

        assert!(FsDiskMaintenanceAdapter::new().configure_sccache(root.path()).is_err());
        assert_eq!(fs::read_to_string(sentinel)?, "unchanged");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_configure_sccache_symlinked_environment_file_returns_error()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        write_config(root.path(), "\"target\"")?;
        let cache_dir = root.path().join(".cache");
        fs::create_dir_all(&cache_dir)?;
        let sentinel = outside.path().join("sccache.env");
        fs::write(&sentinel, "unchanged")?;
        std::os::unix::fs::symlink(&sentinel, cache_dir.join("sccache.env"))?;

        assert!(FsDiskMaintenanceAdapter::new().configure_sccache(root.path()).is_err());
        assert_eq!(fs::read_to_string(sentinel)?, "unchanged");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_apply_cleanup_symlinked_root_returns_error() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        write_config(root.path(), "\"target\"")?;
        let sentinel = outside.path().join("sentinel");
        fs::write(&sentinel, "unchanged")?;
        std::os::unix::fs::symlink(outside.path(), root.path().join("target"))?;

        assert!(FsDiskMaintenanceAdapter::new().apply_cleanup(root.path().to_path_buf()).is_err());
        assert_eq!(fs::read_to_string(sentinel)?, "unchanged");
        Ok(())
    }
}
