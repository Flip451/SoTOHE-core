//! Filesystem adapter for configurable disk maintenance.

use std::ffi::{CString, OsStr};
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Write};
use std::mem::MaybeUninit;
use std::path::{Path, PathBuf};

use domain::disk_maintenance::{CacheSize, CleanupScope, CleanupScopeSet, DiskMaintenanceConfig};
use serde::Deserialize;
use usecase::disk_maintenance::{
    DiskMaintenanceCommandPort, DiskMaintenanceError, DiskMaintenanceQueryPort,
};

const MAX_DISK_MAINTENANCE_CONFIG_BYTES: u64 = 1024 * 1024;
const DIRECTORY_ENTRY_BUFFER_BYTES: usize = 8192;

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

    fn config_path(project_root: &Path) -> PathBuf {
        project_root.join(".harness/config/disk-maintenance.toml")
    }

    fn read_config_file(file: File, path: &Path) -> Result<String, std::io::Error> {
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

    fn load_config(
        project_root: &Path,
        project_directory: &File,
    ) -> Result<DiskMaintenanceConfig, DiskMaintenanceError> {
        let path = Self::config_path(project_root);
        let raw = Self::open_config_file(project_directory, &path).and_then(|file| {
            Self::read_config_file(file, &path).map_err(|error| {
                DiskMaintenanceError::new(format!("cannot read {}: {error}", path.display()))
            })
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

    fn open_config_file(
        project_directory: &File,
        path: &Path,
    ) -> Result<File, DiskMaintenanceError> {
        let harness = Self::open_directory_at_nofollow(project_directory, OsStr::new(".harness"))
            .map_err(|error| {
            DiskMaintenanceError::new(format!("cannot open {}: {error}", path.display()))
        })?;
        let config =
            Self::open_directory_at_nofollow(&harness, OsStr::new("config")).map_err(|error| {
                DiskMaintenanceError::new(format!("cannot open {}: {error}", path.display()))
            })?;
        rustix::fs::openat(
            &config,
            OsStr::new("disk-maintenance.toml"),
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map(File::from)
        .map_err(|error| {
            DiskMaintenanceError::new(format!("cannot read {}: {error}", path.display()))
        })
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

    fn open_directory_nofollow(path: &Path) -> Result<File, std::io::Error> {
        rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map(File::from)
        .map_err(Into::into)
    }

    fn open_project_directory_nofollow(project_root: &Path) -> Result<File, std::io::Error> {
        let directory = if project_root.is_absolute() {
            Self::open_directory_nofollow(Path::new("/"))?
        } else {
            Self::open_directory_nofollow(Path::new("."))?
        };
        Self::open_directory_components_nofollow(directory, project_root.components())
    }

    fn open_directory_components_nofollow(
        mut directory: File,
        components: std::path::Components<'_>,
    ) -> Result<File, std::io::Error> {
        for component in components {
            let name = match component {
                std::path::Component::RootDir | std::path::Component::CurDir => continue,
                std::path::Component::Normal(name) => name,
                std::path::Component::ParentDir => OsStr::new(".."),
                std::path::Component::Prefix(_) => {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
                        "unsupported project root path prefix",
                    ));
                }
            };
            directory = Self::open_directory_at_nofollow(&directory, name)?;
        }
        Ok(directory)
    }

    fn open_directory_at_nofollow(parent: &File, name: &OsStr) -> Result<File, std::io::Error> {
        rustix::fs::openat(
            parent,
            name,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map(File::from)
        .map_err(Into::into)
    }

    fn open_or_create_directory_at(parent: &File, name: &OsStr) -> Result<File, std::io::Error> {
        match Self::open_directory_at_nofollow(parent, name) {
            Ok(directory) => Ok(directory),
            Err(error) if error.kind() == ErrorKind::NotFound => {
                match rustix::fs::mkdirat(parent, name, rustix::fs::Mode::from_raw_mode(0o700)) {
                    Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                    Err(error) => return Err(error.into()),
                }
                Self::open_directory_at_nofollow(parent, name)
            }
            Err(error) => Err(error),
        }
    }

    fn open_cleanup_root(
        project_directory: &File,
        scope: &CleanupScope,
        root: &Path,
    ) -> Result<File, DiskMaintenanceError> {
        let mut directory = project_directory.try_clone().map_err(|error| {
            DiskMaintenanceError::new(format!(
                "cannot retain cleanup root {}: {error}",
                root.display()
            ))
        })?;

        for component in scope.as_path().components() {
            let std::path::Component::Normal(name) = component else {
                return Err(DiskMaintenanceError::new("cleanup scope escapes project root"));
            };
            directory = Self::open_or_create_directory_at(&directory, name).map_err(|error| {
                DiskMaintenanceError::new(format!(
                    "cannot open cleanup root {}: {error}",
                    root.display()
                ))
            })?;
        }
        Ok(directory)
    }

    fn read_directory_entry_batch(
        directory: &File,
        root: &Path,
    ) -> Result<Option<Vec<CString>>, DiskMaintenanceError> {
        let mut buffer = [MaybeUninit::uninit(); DIRECTORY_ENTRY_BUFFER_BYTES];
        let mut entries = rustix::fs::RawDir::new(directory, &mut buffer);
        let mut names = Vec::new();
        let Some(entry) = entries.next() else {
            return Ok(None);
        };
        let mut entry = entry.map_err(|error| {
            DiskMaintenanceError::new(format!(
                "cannot read cleanup root {}: {error}",
                root.display()
            ))
        })?;
        loop {
            let name = entry.file_name();
            if matches!(name.to_bytes(), b"." | b"..") {
                // Skip directory self and parent entries.
            } else {
                names.push(name.to_owned());
            }
            if entries.is_buffer_empty() {
                return Ok(Some(names));
            }
            entry = entries
                .next()
                .ok_or_else(|| {
                    DiskMaintenanceError::new(format!(
                        "cannot read cleanup root {}: incomplete directory entry batch",
                        root.display()
                    ))
                })?
                .map_err(|error| {
                    DiskMaintenanceError::new(format!(
                        "cannot read cleanup root {}: {error}",
                        root.display()
                    ))
                })?;
        }
    }

    fn empty_open_directory(directory: File, root: &Path) -> Result<(), DiskMaintenanceError> {
        struct DirectoryFrame {
            directory: File,
            entries: Vec<CString>,
            next_entry: usize,
            parent_entry: Option<CString>,
        }

        let mut frames = vec![DirectoryFrame {
            entries: Vec::new(),
            directory,
            next_entry: 0,
            parent_entry: None,
        }];
        while let Some(frame) = frames.last_mut() {
            if frame.next_entry == frame.entries.len() {
                if let Some(entries) = Self::read_directory_entry_batch(&frame.directory, root)? {
                    frame.entries = entries;
                    frame.next_entry = 0;
                    continue;
                }
                let completed = frames.pop().ok_or_else(|| {
                    DiskMaintenanceError::new("cleanup traversal lost its current directory")
                })?;
                if let Some(parent_entry) = completed.parent_entry {
                    let parent = frames.last().ok_or_else(|| {
                        DiskMaintenanceError::new("cleanup traversal lost a parent directory")
                    })?;
                    rustix::fs::unlinkat(
                        &parent.directory,
                        parent_entry.as_c_str(),
                        rustix::fs::AtFlags::REMOVEDIR,
                    )
                    .map_err(|error| {
                        DiskMaintenanceError::new(format!(
                            "cannot remove cleanup directory below {}: {error}",
                            root.display()
                        ))
                    })?;
                }
                continue;
            }
            let name = frame.entries.get(frame.next_entry).cloned().ok_or_else(|| {
                DiskMaintenanceError::new("cleanup traversal lost its current directory entry")
            })?;
            frame.next_entry = frame.next_entry.saturating_add(1);

            let entry_metadata = rustix::fs::statat(
                &frame.directory,
                name.as_c_str(),
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|error| {
                DiskMaintenanceError::new(format!(
                    "cannot inspect cleanup entry below {}: {error}",
                    root.display()
                ))
            })?;
            if rustix::fs::FileType::from_raw_mode(entry_metadata.st_mode).is_dir() {
                let child = rustix::fs::openat(
                    &frame.directory,
                    name.as_c_str(),
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::DIRECTORY
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                )
                .map(File::from)
                .map_err(|error| {
                    DiskMaintenanceError::new(format!(
                        "cannot open cleanup directory below {}: {error}",
                        root.display()
                    ))
                })?;
                frames.push(DirectoryFrame {
                    entries: Vec::new(),
                    directory: child,
                    next_entry: 0,
                    parent_entry: Some(name),
                });
            } else {
                rustix::fs::unlinkat(
                    &frame.directory,
                    name.as_c_str(),
                    rustix::fs::AtFlags::empty(),
                )
                .map_err(|error| {
                    DiskMaintenanceError::new(format!(
                        "cannot remove cleanup entry below {}: {error}",
                        root.display()
                    ))
                })?;
            }
        }
        Ok(())
    }

    fn empty_directory(
        project_directory: &File,
        scope: &CleanupScope,
        root: &Path,
    ) -> Result<(), DiskMaintenanceError> {
        let directory = Self::open_cleanup_root(project_directory, scope, root)?;
        Self::empty_open_directory(directory, root)
    }

    fn open_sccache_environment_file(
        cache_directory: &File,
        environment_path: &Path,
    ) -> Result<File, DiskMaintenanceError> {
        let file = rustix::fs::openat(
            cache_directory,
            OsStr::new("sccache.env"),
            rustix::fs::OFlags::WRONLY
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::from_raw_mode(0o600),
        )
        .map(File::from)
        .map_err(|error| {
            DiskMaintenanceError::new(format!(
                "cannot open {}: {error}",
                environment_path.display()
            ))
        })?;
        if !file
            .metadata()
            .map_err(|error| {
                DiskMaintenanceError::new(format!(
                    "cannot inspect {}: {error}",
                    environment_path.display()
                ))
            })?
            .is_file()
        {
            return Err(DiskMaintenanceError::new(format!(
                "{} is not a regular file",
                environment_path.display()
            )));
        }
        Ok(file)
    }
}

impl DiskMaintenanceQueryPort for FsDiskMaintenanceAdapter {
    fn plan_cleanup(&self, project_root: &Path) -> Result<CleanupScopeSet, DiskMaintenanceError> {
        let project_directory =
            Self::open_project_directory_nofollow(project_root).map_err(|error| {
                DiskMaintenanceError::new(format!(
                    "cannot open project root {}: {error}",
                    project_root.display()
                ))
            })?;
        Ok(Self::load_config(project_root, &project_directory)?.cleanup_scopes().clone())
    }
}

impl DiskMaintenanceCommandPort for FsDiskMaintenanceAdapter {
    fn configure_sccache(&self, project_root: &Path) -> Result<CacheSize, DiskMaintenanceError> {
        let cache_dir = project_root.join(".cache");
        let project_directory =
            Self::open_project_directory_nofollow(project_root).map_err(|error| {
                DiskMaintenanceError::new(format!(
                    "cannot open sccache project root {}: {error}",
                    project_root.display()
                ))
            })?;
        let config = Self::load_config(project_root, &project_directory)?;
        let cache_directory = Self::open_or_create_directory_at(
            &project_directory,
            OsStr::new(".cache"),
        )
        .map_err(|error| {
            DiskMaintenanceError::new(format!("cannot open {}: {error}", cache_dir.display()))
        })?;
        let env_path = cache_dir.join("sccache.env");
        let mut environment_file =
            Self::open_sccache_environment_file(&cache_directory, &env_path)?;
        environment_file.set_len(0).map_err(|error| {
            DiskMaintenanceError::new(format!("cannot truncate {}: {error}", env_path.display()))
        })?;
        environment_file
            .write_all(
                format!("SCCACHE_CACHE_SIZE={}\n", config.max_cache_size().as_str()).as_bytes(),
            )
            .map_err(|error| {
                DiskMaintenanceError::new(format!("cannot write {}: {error}", env_path.display()))
            })?;
        Ok(config.max_cache_size().clone())
    }

    fn apply_cleanup(
        &self,
        project_root: PathBuf,
    ) -> Result<CleanupScopeSet, DiskMaintenanceError> {
        let project_directory =
            Self::open_project_directory_nofollow(&project_root).map_err(|error| {
                DiskMaintenanceError::new(format!(
                    "cannot open cleanup project root {}: {error}",
                    project_root.display()
                ))
            })?;
        let config = Self::load_config(&project_root, &project_directory)?;
        for scope in config.cleanup_scopes().as_slice() {
            let root = Self::cleanup_root(&project_root, scope)?;
            Self::empty_directory(&project_directory, scope, &root)?;
        }
        Ok(config.cleanup_scopes().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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

    #[cfg(unix)]
    #[test]
    fn test_empty_open_directory_path_swap_preserves_external_contents()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        let cleanup_root = root.path().join("target");
        fs::create_dir_all(&cleanup_root)?;
        fs::write(cleanup_root.join("inside"), "remove")?;
        let sentinel = outside.path().join("sentinel");
        fs::write(&sentinel, "unchanged")?;

        let opened_root = FsDiskMaintenanceAdapter::open_directory_nofollow(&cleanup_root)?;
        let original_root = root.path().join("target-before-swap");
        fs::rename(&cleanup_root, &original_root)?;
        std::os::unix::fs::symlink(outside.path(), &cleanup_root)?;

        FsDiskMaintenanceAdapter::empty_open_directory(opened_root, &cleanup_root)?;

        assert!(fs::read_dir(&original_root)?.next().is_none());
        assert_eq!(fs::read_to_string(sentinel)?, "unchanged");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_opened_sccache_environment_file_path_swap_preserves_external_file()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        let cache_root = root.path().join(".cache");
        fs::create_dir_all(&cache_root)?;
        let environment_path = cache_root.join("sccache.env");
        fs::write(&environment_path, "old")?;
        let sentinel = outside.path().join("sccache.env");
        fs::write(&sentinel, "unchanged")?;

        let cache_directory = FsDiskMaintenanceAdapter::open_directory_nofollow(&cache_root)?;
        let mut environment_file = FsDiskMaintenanceAdapter::open_sccache_environment_file(
            &cache_directory,
            &environment_path,
        )?;
        let original_environment_path = cache_root.join("sccache-before-swap.env");
        fs::rename(&environment_path, &original_environment_path)?;
        std::os::unix::fs::symlink(&sentinel, &environment_path)?;

        environment_file.set_len(0)?;
        environment_file.write_all(b"updated")?;

        assert_eq!(fs::read_to_string(&original_environment_path)?, "updated");
        assert_eq!(fs::read_to_string(sentinel)?, "unchanged");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_descriptor_root_ancestor_swap_preserves_external_sccache_file()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        let ancestor = root.path().join("ancestor");
        let project = ancestor.join("project");
        fs::create_dir_all(&project)?;
        let sentinel = outside.path().join(".cache/sccache.env");
        fs::create_dir_all(sentinel.parent().ok_or("sentinel has no parent")?)?;
        fs::write(&sentinel, "unchanged")?;

        let retained_ancestor = FsDiskMaintenanceAdapter::open_directory_nofollow(&ancestor)?;
        let original_ancestor = root.path().join("ancestor-before-swap");
        fs::rename(&ancestor, &original_ancestor)?;
        std::os::unix::fs::symlink(outside.path(), &ancestor)?;

        let project_directory = FsDiskMaintenanceAdapter::open_directory_components_nofollow(
            retained_ancestor,
            Path::new("project").components(),
        )?;
        let cache_directory = FsDiskMaintenanceAdapter::open_or_create_directory_at(
            &project_directory,
            OsStr::new(".cache"),
        )?;
        let mut environment_file = FsDiskMaintenanceAdapter::open_sccache_environment_file(
            &cache_directory,
            &project.join(".cache/sccache.env"),
        )?;
        environment_file.write_all(b"updated")?;

        assert_eq!(
            fs::read_to_string(original_ancestor.join("project/.cache/sccache.env"))?,
            "updated"
        );
        assert_eq!(fs::read_to_string(sentinel)?, "unchanged");
        Ok(())
    }

    #[test]
    fn test_apply_cleanup_deep_tree_completes_without_stack_overflow()
    -> Result<(), Box<dyn std::error::Error>> {
        const DEPTH: usize = 1024;

        let root = tempfile::tempdir()?;
        write_config(root.path(), "\"target\"")?;
        let target = root.path().join("target");
        let mut nested = target.clone();
        fs::create_dir_all(&nested)?;
        for _ in 0..DEPTH {
            nested.push("d");
            fs::create_dir(&nested)?;
        }
        fs::write(nested.join("artifact"), "remove")?;

        FsDiskMaintenanceAdapter::new().apply_cleanup(root.path().to_path_buf())?;

        assert!(target.is_dir());
        assert!(fs::read_dir(target)?.next().is_none());
        Ok(())
    }
}
