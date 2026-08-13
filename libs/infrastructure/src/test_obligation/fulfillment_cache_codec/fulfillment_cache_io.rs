//! Guarded, bounded I/O for the fulfillment-cache JSON artifact.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use domain::tddd::test_obligation::errors::VerifyCacheError;
use domain::tddd::test_obligation::ids::DiagnosticMessage;
use serde::Serialize;

use crate::test_obligation::diagnostic;

const MAX_FULFILLMENT_CACHE_BYTES: u64 = 4 * 1024 * 1024;

/// Fixed-capacity JSON sink that rejects an oversized cache before it is written.
struct BoundedFulfillmentCacheBuffer {
    bytes: Vec<u8>,
    exceeded_limit: bool,
}

impl BoundedFulfillmentCacheBuffer {
    fn new() -> Self {
        Self {
            bytes: Vec::with_capacity(MAX_FULFILLMENT_CACHE_BYTES as usize),
            exceeded_limit: false,
        }
    }

    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }

    fn clear(&mut self) {
        self.bytes.clear();
        self.exceeded_limit = false;
    }
}

impl Write for BoundedFulfillmentCacheBuffer {
    fn write(&mut self, bytes: &[u8]) -> Result<usize, std::io::Error> {
        let remaining = (MAX_FULFILLMENT_CACHE_BYTES as usize).saturating_sub(self.bytes.len());
        if bytes.len() > remaining {
            self.exceeded_limit = true;
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "fulfillment cache serialized output exceeds the byte limit",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}

pub(super) fn serialize_bounded_fulfillment_cache<T: Serialize>(
    dto: &T,
) -> Result<Vec<u8>, DiagnosticMessage> {
    let mut writer = BoundedFulfillmentCacheBuffer::new();
    // First serialize through the bounded sink. This makes the JSON input to
    // canonicalization no larger than the artifact limit, rather than
    // materializing an unbounded `Value` before output is checked.
    serde_json::to_writer_pretty(&mut writer, dto).map_err(|error| {
        if writer.exceeded_limit {
            diagnostic(&format!("fulfillment cache exceeds {MAX_FULFILLMENT_CACHE_BYTES} bytes"))
        } else {
            diagnostic(&format!("failed to encode fulfillment cache: {error}"))
        }
    })?;

    // `serde_json::Value` uses its canonical map-key ordering recursively.
    // Its source is already limited by the bounded serialization above, and
    // the same bounded buffer is reused for the final artifact bytes.
    let value: serde_json::Value = serde_json::from_slice(&writer.bytes)
        .map_err(|error| diagnostic(&format!("failed to encode fulfillment cache: {error}")))?;
    writer.clear();
    serde_json::to_writer_pretty(&mut writer, &value).map_err(|error| {
        if writer.exceeded_limit {
            diagnostic(&format!("fulfillment cache exceeds {MAX_FULFILLMENT_CACHE_BYTES} bytes"))
        } else {
            diagnostic(&format!("failed to encode fulfillment cache: {error}"))
        }
    })?;
    Ok(writer.into_inner())
}

/// Reads a regular cache file through a descriptor-pinned, no-follow
/// handle with a hard byte cap.
pub(super) fn read_bounded_fulfillment_cache(
    path: &Path,
    trusted_root: &Path,
) -> Result<String, VerifyCacheError> {
    let file = open_fulfillment_cache_guarded(path, trusted_root).map_err(|error| {
        VerifyCacheError::Io(diagnostic(&format!(
            "failed to open fulfillment cache {}: {error}",
            path.display()
        )))
    })?;
    let metadata = file.metadata().map_err(|error| {
        VerifyCacheError::Io(diagnostic(&format!(
            "failed to inspect fulfillment cache {}: {error}",
            path.display()
        )))
    })?;
    if !metadata.is_file() || metadata.len() > MAX_FULFILLMENT_CACHE_BYTES {
        return Err(VerifyCacheError::Io(diagnostic(&format!(
            "fulfillment cache {} exceeds {MAX_FULFILLMENT_CACHE_BYTES} bytes or is not a regular file",
            path.display()
        ))));
    }
    let mut reader = file.take(MAX_FULFILLMENT_CACHE_BYTES.saturating_add(1));
    let mut content = String::new();
    reader.read_to_string(&mut content).map_err(|error| {
        VerifyCacheError::Io(diagnostic(&format!(
            "failed to read fulfillment cache {}: {error}",
            path.display()
        )))
    })?;
    if content.len() > MAX_FULFILLMENT_CACHE_BYTES as usize {
        return Err(VerifyCacheError::Io(diagnostic(&format!(
            "fulfillment cache {} exceeds {MAX_FULFILLMENT_CACHE_BYTES} bytes",
            path.display()
        ))));
    }
    Ok(content)
}

fn open_fulfillment_cache_guarded(
    path: &Path,
    trusted_root: &Path,
) -> Result<File, std::io::Error> {
    #[cfg(unix)]
    {
        let relative_path = path.strip_prefix(trusted_root).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "fulfillment cache path escapes its trusted root",
            )
        })?;
        let mut components = relative_path.components().peekable();
        let mut directory = open_trusted_root_for_read(trusted_root)?;

        while let Some(component) = components.next() {
            let std::path::Component::Normal(name) = component else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "fulfillment cache path contains an invalid component",
                ));
            };
            let terminal = components.peek().is_none();
            let flags = if terminal {
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC
                    | rustix::fs::OFlags::NONBLOCK
            } else {
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC
            };
            let opened = rustix::fs::openat(&directory, name, flags, rustix::fs::Mode::empty())
                .map(File::from)?;
            if terminal {
                return Ok(opened);
            }
            directory = opened;
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "fulfillment cache path has no leaf component",
        ))
    }
    #[cfg(not(unix))]
    {
        unsupported_descriptor_pinned_cache_io(path, trusted_root)
    }
}

/// Opens the cache leaf for overwrite through a descriptor-pinned path
/// beneath `trusted_root`. Missing parent directories are created through the
/// same pinned descriptor chain.
pub(super) fn open_fulfillment_cache_for_write_guarded(
    path: &Path,
    trusted_root: &Path,
) -> Result<File, std::io::Error> {
    #[cfg(unix)]
    {
        let relative_path = path.strip_prefix(trusted_root).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "fulfillment cache path escapes its trusted root",
            )
        })?;
        let mut components = relative_path.components().peekable();
        let mut directory = open_trusted_root_for_write(trusted_root)?;

        while let Some(component) = components.next() {
            let std::path::Component::Normal(name) = component else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "fulfillment cache path contains an invalid component",
                ));
            };
            if components.peek().is_none() {
                return Ok(rustix::fs::openat(
                    &directory,
                    name,
                    rustix::fs::OFlags::WRONLY
                        | rustix::fs::OFlags::CREATE
                        | rustix::fs::OFlags::TRUNC
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::NONBLOCK
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::from_raw_mode(0o600),
                )
                .map(File::from)?);
            }
            let flags = rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC;
            let opened =
                match rustix::fs::openat(&directory, name, flags, rustix::fs::Mode::empty()) {
                    Ok(opened) => opened,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        match rustix::fs::mkdirat(
                            &directory,
                            name,
                            rustix::fs::Mode::from_raw_mode(0o700),
                        ) {
                            Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                            Err(error) => return Err(error.into()),
                        }
                        rustix::fs::openat(&directory, name, flags, rustix::fs::Mode::empty())?
                    }
                    Err(error) => return Err(error.into()),
                };
            directory = File::from(opened);
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "fulfillment cache path has no leaf component",
        ))
    }
    #[cfg(not(unix))]
    {
        unsupported_descriptor_pinned_cache_io(path, trusted_root)
    }
}

#[cfg(unix)]
fn open_trusted_root_for_write(trusted_root: &Path) -> Result<File, std::io::Error> {
    open_trusted_root_guarded(trusted_root, true)
}

#[cfg(unix)]
fn open_trusted_root_for_read(trusted_root: &Path) -> Result<File, std::io::Error> {
    open_trusted_root_guarded(trusted_root, false)
}

#[cfg(unix)]
fn open_trusted_root_guarded(
    trusted_root: &Path,
    create_missing: bool,
) -> Result<File, std::io::Error> {
    let directory_flags = rustix::fs::OFlags::RDONLY
        | rustix::fs::OFlags::DIRECTORY
        | rustix::fs::OFlags::NOFOLLOW
        | rustix::fs::OFlags::CLOEXEC;
    let mut components = trusted_root.components().peekable();
    let mut directory = if matches!(components.peek(), Some(std::path::Component::RootDir)) {
        components.next();
        rustix::fs::open("/", directory_flags, rustix::fs::Mode::empty()).map(File::from)?
    } else {
        if matches!(components.peek(), Some(std::path::Component::CurDir)) {
            components.next();
        }
        rustix::fs::open(".", directory_flags, rustix::fs::Mode::empty()).map(File::from)?
    };

    for component in components {
        let std::path::Component::Normal(name) = component else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "trusted root path contains an invalid component",
            ));
        };
        let opened = match rustix::fs::openat(
            &directory,
            name,
            directory_flags,
            rustix::fs::Mode::empty(),
        ) {
            Ok(opened) => opened,
            Err(error) if create_missing && error.kind() == std::io::ErrorKind::NotFound => {
                match rustix::fs::mkdirat(&directory, name, rustix::fs::Mode::from_raw_mode(0o700))
                {
                    Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                    Err(error) => return Err(error.into()),
                }
                rustix::fs::openat(&directory, name, directory_flags, rustix::fs::Mode::empty())?
            }
            Err(error) => return Err(error.into()),
        };
        directory = File::from(opened);
    }
    Ok(directory)
}

#[cfg(not(unix))]
fn unsupported_descriptor_pinned_cache_io(
    path: &Path,
    trusted_root: &Path,
) -> Result<File, std::io::Error> {
    let _ = (path, trusted_root);
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "fulfillment-cache I/O requires Unix descriptor-pinned opening",
    ))
}

#[cfg(all(test, not(unix)))]
mod tests {
    use std::io::ErrorKind;

    use super::{open_fulfillment_cache_for_write_guarded, read_bounded_fulfillment_cache};

    #[test]
    fn test_cache_io_on_non_unix_returns_unsupported() {
        let temporary_directory = tempfile::tempdir().expect("temporary directory must exist");
        let trusted_root = temporary_directory.path().join("items");
        let cache_path = trusted_root.join("track").join("cache.json");

        let write_error = open_fulfillment_cache_for_write_guarded(&cache_path, &trusted_root)
            .expect_err("non-Unix cache writes must fail closed");
        assert_eq!(write_error.kind(), ErrorKind::Unsupported);
        assert!(
            read_bounded_fulfillment_cache(&cache_path, &trusted_root).is_err(),
            "non-Unix cache reads must fail closed"
        );
    }
}

#[cfg(all(test, unix))]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod unix_tests {
    use std::io::Write as _;
    use std::path::PathBuf;

    use super::{open_fulfillment_cache_for_write_guarded, read_bounded_fulfillment_cache};

    #[derive(serde::Serialize)]
    struct FulfillmentCacheFixture {
        track_id: String,
        entries: Vec<FulfillmentCacheEntryFixture>,
    }

    #[derive(serde::Serialize)]
    struct FulfillmentCacheEntryFixture {
        verdict: FulfillmentVerdictFixture,
        key: FulfillmentCacheKeyFixture,
    }

    #[derive(serde::Serialize)]
    struct FulfillmentVerdictFixture {
        reason: String,
        kind: String,
        category: String,
    }

    #[derive(serde::Serialize)]
    struct FulfillmentCacheKeyFixture {
        declaration_hash: String,
        anchor_text_hash: String,
        bound_tests_set_hash: String,
    }

    #[test]
    fn test_guarded_cache_write_with_missing_trusted_root_creates_cache() {
        let temporary_directory = tempfile::tempdir().unwrap();
        let trusted_root = temporary_directory.path().join("items");
        let cache_path = trusted_root.join("track").join("cache.json");

        let mut cache = open_fulfillment_cache_for_write_guarded(&cache_path, &trusted_root)
            .expect("a missing trusted root must be created for first cache save");
        cache.write_all(b"{}").unwrap();
        drop(cache);

        assert_eq!(std::fs::read(cache_path).unwrap(), b"{}");
    }

    #[test]
    fn test_fulfillment_cache_writer_nested_verdict_returns_canonical_deterministic_bytes() {
        let fixture = FulfillmentCacheFixture {
            track_id: "my-track".to_owned(),
            entries: vec![FulfillmentCacheEntryFixture {
                verdict: FulfillmentVerdictFixture {
                    reason: "missing assertion".to_owned(),
                    kind: "fail".to_owned(),
                    category: "substitution".to_owned(),
                },
                key: FulfillmentCacheKeyFixture {
                    declaration_hash: "declaration".to_owned(),
                    anchor_text_hash: "anchor".to_owned(),
                    bound_tests_set_hash: "bound-tests".to_owned(),
                },
            }],
        };

        let first = super::serialize_bounded_fulfillment_cache(&fixture).unwrap();
        let second = super::serialize_bounded_fulfillment_cache(&fixture).unwrap();
        let json = std::str::from_utf8(&first).unwrap();

        assert_eq!(first, second);
        assert!(json.starts_with("{\n  \"entries\":"), "root keys must be canonical: {json}");
        assert!(
            json.contains(
                "\"verdict\": {\n        \"category\": \"substitution\",\n        \"kind\": \"fail\",\n        \"reason\": \"missing assertion\""
            ),
            "nested verdict keys must be canonical: {json}"
        );
    }

    #[test]
    fn test_guarded_cache_write_with_symlinked_root_ancestor_fails_closed() {
        let temporary_directory = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let symlinked_ancestor = temporary_directory.path().join("redirect");
        std::os::unix::fs::symlink(outside.path(), &symlinked_ancestor).unwrap();
        let trusted_root = symlinked_ancestor.join("items");
        let cache_path = trusted_root.join("track").join("cache.json");

        assert!(open_fulfillment_cache_for_write_guarded(&cache_path, &trusted_root).is_err());
        assert!(!outside.path().join("items").exists());
    }

    #[test]
    fn test_guarded_cache_read_with_symlinked_root_ancestor_fails_closed() {
        let temporary_directory = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_cache = outside.path().join("items").join("track").join("cache.json");
        std::fs::create_dir_all(outside_cache.parent().unwrap()).unwrap();
        std::fs::write(&outside_cache, "{}").unwrap();
        let symlinked_ancestor = temporary_directory.path().join("redirect");
        std::os::unix::fs::symlink(outside.path(), &symlinked_ancestor).unwrap();
        let trusted_root = symlinked_ancestor.join("items");
        let cache_path = trusted_root.join("track").join("cache.json");

        assert!(read_bounded_fulfillment_cache(&cache_path, &trusted_root).is_err());
    }

    #[test]
    fn test_guarded_cache_write_with_relative_current_directory_root_creates_cache() {
        let temporary_directory =
            tempfile::Builder::new().prefix("fulfillment-cache-").tempdir_in(".").unwrap();
        let directory_name = temporary_directory.path().file_name().unwrap();
        let trusted_root = PathBuf::from(".").join(directory_name).join("items");
        let cache_path = trusted_root.join("track").join("cache.json");

        let mut cache = open_fulfillment_cache_for_write_guarded(&cache_path, &trusted_root)
            .expect("a relative current-directory root must support first cache save");
        cache.write_all(b"{}").unwrap();
        drop(cache);

        assert_eq!(
            std::fs::read(temporary_directory.path().join("items/track/cache.json")).unwrap(),
            b"{}"
        );
    }
}
