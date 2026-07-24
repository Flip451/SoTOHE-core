use std::fs::{self, OpenOptions};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use domain::adr_baseline::{
    AdrBaselineLedgerEntry, AdrBaselineRecordedCopyStatus, AdrSourceFileName,
};
use domain::tddd::test_obligation::ids::DiagnosticMessage;
use domain::{ContentHash, Timestamp, TrackId};
use fs4::fs_std::FileExt as _;
use sha2::{Digest as _, Sha256};
use usecase::adr_baseline::{
    AdrBaselineSnapshotKind, AdrBaselineStoreError, AdrBaselineStorePort,
    AdrBaselineStoreReadError, AdrBaselineStoreReadPort,
};

use super::{decode_ledger_line, diagnostic, encode_ledger_entry, make_entry};

const TRACK_ITEMS: &str = "track/items";
const BASELINE_DIR: &str = "adr-baseline";
const LEDGER_FILE: &str = "ledger.jsonl";
const LEDGER_LOCK_FILE: &str = "ledger.jsonl.lock";
const MAX_SNAPSHOT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_LEDGER_BYTES: u64 = 8 * 1024 * 1024;
const MAX_LEDGER_LINE_BYTES: usize = 64 * 1024;
const MAX_LEDGER_ENTRIES: usize = 10_000;
const MAX_BASELINE_DIR_ENTRIES: usize = MAX_LEDGER_ENTRIES + 2;

enum CopyLookup {
    Missing,
    Found { path: PathBuf, actual: ContentHash },
}

/// Filesystem persistence adapter rooted at the repository workspace.
#[derive(Debug, Clone)]
pub struct FsAdrBaselineStore {
    root: PathBuf,
}

impl From<PathBuf> for FsAdrBaselineStore {
    fn from(root: PathBuf) -> Self {
        Self { root }
    }
}

impl FsAdrBaselineStore {
    fn reject_symlinks(&self, path: &Path) -> Result<bool, std::io::Error> {
        reject_leaf_symlink(path)?;
        crate::track::symlink_guard::reject_symlinks_below(path, &self.root)
    }

    fn ensure_baseline_dir(&self, track_id: &TrackId) -> Result<PathBuf, AdrBaselineStoreError> {
        let dir = self.baseline_dir(track_id);
        self.reject_symlinks(&dir).map_err(io_write_error)?;
        fs::create_dir_all(&dir).map_err(io_write_error)?;
        self.reject_symlinks(&dir).map_err(io_write_error)?;
        Ok(dir)
    }

    fn baseline_dir(&self, track_id: &TrackId) -> PathBuf {
        self.root.join(TRACK_ITEMS).join(track_id.as_ref()).join(BASELINE_DIR)
    }

    fn ledger_path(&self, track_id: &TrackId) -> PathBuf {
        self.baseline_dir(track_id).join(LEDGER_FILE)
    }

    fn lock_ledger(&self, track_id: &TrackId) -> Result<fs::File, AdrBaselineStoreError> {
        let dir = self.baseline_dir(track_id);
        let path = dir.join(LEDGER_LOCK_FILE);
        self.reject_symlinks(&path).map_err(io_write_error)?;
        ensure_resolved_below(&path, &dir).map_err(io_write_error)?;
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)
            .map_err(io_write_error)?;
        file.lock_exclusive().map_err(io_write_error)?;
        Ok(file)
    }

    fn adr_source_path(
        &self,
        source: &AdrSourceFileName,
    ) -> Result<PathBuf, AdrBaselineStoreError> {
        trusted_adr_child(&self.root.join("knowledge/adr"), source).map_err(io_write_error)
    }

    fn read_entries_for_write(
        &self,
        track_id: &TrackId,
    ) -> Result<Vec<AdrBaselineLedgerEntry>, AdrBaselineStoreError> {
        self.reject_symlinks(&self.ledger_path(track_id)).map_err(io_read_error)?;
        read_ledger(&self.ledger_path(track_id)).map_err(AdrBaselineStoreError::Read)
    }

    fn copy_path(
        &self,
        track_id: &TrackId,
        source: &AdrSourceFileName,
        hash: &ContentHash,
        bytes: &[u8],
    ) -> Result<Option<PathBuf>, AdrBaselineStoreError> {
        let dir = self.ensure_baseline_dir(track_id)?;
        validate_adr_source_file_name(source).map_err(io_write_error)?;
        let slug = source.as_str().strip_suffix(".md").unwrap_or(source.as_str());
        let full_hash = hash.to_hex();
        for prefix_len in 8..=full_hash.len() {
            let Some(prefix) = full_hash.get(..prefix_len) else {
                return Err(AdrBaselineStoreError::Write(diagnostic(
                    "invalid snapshot hash prefix",
                )));
            };
            let path =
                trusted_child(&dir, &format!("{slug}.{prefix}.md")).map_err(io_write_error)?;
            self.reject_symlinks(&path).map_err(io_read_error)?;
            ensure_resolved_below(&path, &dir).map_err(io_write_error)?;
            match read_file_limited(&path, MAX_SNAPSHOT_BYTES) {
                Ok(existing) if existing == bytes => return Ok(None),
                Ok(_) => continue,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(Some(path));
                }
                Err(error) => return Err(io_read_error(error)),
            }
        }
        Err(AdrBaselineStoreError::Write(diagnostic(
            "unable to allocate unique ADR baseline filename",
        )))
    }

    fn find_copy(
        &self,
        track_id: &TrackId,
        entry: &AdrBaselineLedgerEntry,
    ) -> Result<CopyLookup, AdrBaselineStoreReadError> {
        let dir = self.baseline_dir(track_id);
        match self.reject_symlinks(&dir) {
            Ok(true) => {}
            Ok(false) => return Ok(CopyLookup::Missing),
            Err(error) => {
                return Err(AdrBaselineStoreReadError::Read(diagnostic(&error.to_string())));
            }
        }
        validate_adr_source_file_name(entry.source())
            .map_err(|error| AdrBaselineStoreReadError::Read(diagnostic(&error.to_string())))?;
        let slug = entry.source().as_str().strip_suffix(".md").unwrap_or(entry.source().as_str());
        let prefix = format!("{slug}.");
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(CopyLookup::Missing);
            }
            Err(error) => {
                return Err(AdrBaselineStoreReadError::Read(diagnostic(&error.to_string())));
            }
        };
        let mut mismatch = None;
        for (entry_count, entry_path) in entries.enumerate() {
            if entry_count >= MAX_BASELINE_DIR_ENTRIES {
                return Err(AdrBaselineStoreReadError::Read(diagnostic(
                    "ADR baseline directory entry count exceeds the configured limit",
                )));
            }
            let entry_path = entry_path
                .map_err(|error| AdrBaselineStoreReadError::Read(diagnostic(&error.to_string())))?;
            let name = entry_path.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with(&prefix) || !name.ends_with(".md") {
                continue;
            }
            let hash_part = name.trim_start_matches(&prefix).trim_end_matches(".md");
            if !hash_part.is_empty() && entry.hash().to_hex().starts_with(hash_part) {
                let path = entry_path.path();
                self.reject_symlinks(&path).map_err(|error| {
                    AdrBaselineStoreReadError::Read(diagnostic(&error.to_string()))
                })?;
                ensure_resolved_below(&path, &dir).map_err(|error| {
                    AdrBaselineStoreReadError::Read(diagnostic(&error.to_string()))
                })?;
                let bytes = read_file_limited(&path, MAX_SNAPSHOT_BYTES).map_err(|error| {
                    AdrBaselineStoreReadError::Read(diagnostic(&error.to_string()))
                })?;
                let actual = content_hash(&bytes);
                if actual == *entry.hash() {
                    return Ok(CopyLookup::Found { path, actual });
                }
                if !actual.to_hex().starts_with(hash_part) {
                    mismatch.get_or_insert((path, actual));
                }
            }
        }
        Ok(match mismatch {
            Some((path, actual)) => CopyLookup::Found { path, actual },
            None => CopyLookup::Missing,
        })
    }
}

impl AdrBaselineStorePort for FsAdrBaselineStore {
    fn snapshot(
        &self,
        track_id: &TrackId,
        source: &AdrSourceFileName,
        bytes: Vec<u8>,
        kind: AdrBaselineSnapshotKind,
        timestamp: Timestamp,
    ) -> Result<AdrBaselineLedgerEntry, AdrBaselineStoreError> {
        validate_adr_source_file_name(source).map_err(io_write_error)?;
        if bytes.len() as u64 > MAX_SNAPSHOT_BYTES {
            return Err(AdrBaselineStoreError::Write(diagnostic(
                "ADR baseline snapshot exceeds the configured byte limit",
            )));
        }
        let hash = content_hash(&bytes);
        let (kind, reason) = kind.into_ledger_parts();
        let entry = make_entry(source.clone(), hash, kind, reason, timestamp)
            .map_err(AdrBaselineStoreError::Write)?;
        let encoded = encode_ledger_entry(&entry)
            .map_err(|error| AdrBaselineStoreError::Write(diagnostic(&error.to_string())))?;
        self.ensure_baseline_dir(track_id)?;
        let _ledger_lock = self.lock_ledger(track_id)?;
        validate_ledger_append(&self.ledger_path(track_id), &self.root, &encoded)
            .map_err(io_write_error)?;
        let created_copy = self.copy_path(track_id, source, entry.hash(), &bytes)?;
        if let Some(path) = &created_copy {
            create_snapshot_copy(path, &bytes).map_err(io_write_error)?;
        }
        if let Err(error) = append_ledger_line(&self.ledger_path(track_id), &self.root, &encoded) {
            return Err(io_write_error(error));
        }
        Ok(entry)
    }

    fn restore(
        &self,
        track_id: &TrackId,
        source: &AdrSourceFileName,
    ) -> Result<(), AdrBaselineStoreError> {
        let entries = self.read_entries_for_write(track_id)?;
        let latest =
            entries.iter().rev().find(|entry| entry.source() == source).ok_or_else(|| {
                AdrBaselineStoreError::Read(diagnostic("no recorded ADR baseline for source"))
            })?;
        let path = match self
            .find_copy(track_id, latest)
            .map_err(|error| AdrBaselineStoreError::Read(diagnostic(&error.to_string())))?
        {
            CopyLookup::Missing => {
                return Err(AdrBaselineStoreError::Read(diagnostic(
                    "latest ADR baseline copy is missing",
                )));
            }
            CopyLookup::Found { path, actual } if actual == *latest.hash() => path,
            CopyLookup::Found { .. } => {
                return Err(AdrBaselineStoreError::Read(diagnostic(
                    "latest ADR baseline copy hash does not match its ledger entry",
                )));
            }
        };
        self.reject_symlinks(&path).map_err(io_read_error)?;
        let bytes = read_file_limited(&path, MAX_SNAPSHOT_BYTES).map_err(io_read_error)?;
        let target = self.adr_source_path(source)?;
        ensure_resolved_below(&target, &self.root).map_err(io_write_error)?;
        atomic_overwrite(&target, &self.root, &bytes).map_err(io_write_error)
    }
}

impl AdrBaselineStoreReadPort for FsAdrBaselineStore {
    fn read_entries(
        &self,
        track_id: &TrackId,
    ) -> Result<Vec<AdrBaselineLedgerEntry>, AdrBaselineStoreReadError> {
        self.reject_symlinks(&self.ledger_path(track_id))
            .map_err(|error| AdrBaselineStoreReadError::Read(diagnostic(&error.to_string())))?;
        read_ledger(&self.ledger_path(track_id)).map_err(AdrBaselineStoreReadError::Read)
    }

    fn verify_recorded_copy(
        &self,
        track_id: &TrackId,
        entry: &AdrBaselineLedgerEntry,
    ) -> Result<AdrBaselineRecordedCopyStatus, AdrBaselineStoreReadError> {
        match self.find_copy(track_id, entry)? {
            CopyLookup::Missing => Ok(AdrBaselineRecordedCopyStatus::Missing),
            CopyLookup::Found { actual, .. } if actual == *entry.hash() => {
                Ok(AdrBaselineRecordedCopyStatus::Matches)
            }
            CopyLookup::Found { actual, .. } => {
                Ok(AdrBaselineRecordedCopyStatus::HashMismatch { actual })
            }
        }
    }
}

fn read_ledger(path: &Path) -> Result<Vec<AdrBaselineLedgerEntry>, DiagnosticMessage> {
    reject_leaf_symlink(path).map_err(|error| diagnostic(&error.to_string()))?;
    let content = match read_utf8_limited(path, MAX_LEDGER_BYTES) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(diagnostic(&error.to_string())),
    };
    let mut entries = Vec::new();
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        if line.len() > MAX_LEDGER_LINE_BYTES {
            return Err(diagnostic("ADR baseline ledger line exceeds the configured byte limit"));
        }
        if entries.len() == MAX_LEDGER_ENTRIES {
            return Err(diagnostic("ADR baseline ledger entry count exceeds the configured limit"));
        }
        entries.push(decode_ledger_line(line).map_err(|error| diagnostic(&error.to_string()))?);
    }
    Ok(entries)
}

fn validate_adr_source_file_name(source: &AdrSourceFileName) -> std::io::Result<()> {
    if has_windows_drive_prefix(source.as_str()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "ADR source filename must not have a Windows drive prefix",
        ));
    }
    Ok(())
}

fn trusted_adr_child(root: &Path, source: &AdrSourceFileName) -> std::io::Result<PathBuf> {
    validate_adr_source_file_name(source)?;
    trusted_child(root, source.as_str())
}

fn trusted_child(root: &Path, child: &str) -> std::io::Result<PathBuf> {
    let path = root.join(child);
    if !path.starts_with(root) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "ADR baseline path escapes its trusted root",
        ));
    }
    Ok(path)
}

fn has_windows_drive_prefix(value: &str) -> bool {
    matches!(
        (value.as_bytes().first(), value.as_bytes().get(1)),
        (Some(first), Some(b':')) if first.is_ascii_alphabetic()
    )
}

fn ensure_resolved_below(path: &Path, trusted_root: &Path) -> std::io::Result<()> {
    let canonical_root = trusted_root.canonicalize()?;
    let resolved = match path.canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => path
            .parent()
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "ADR baseline path has no parent",
                )
            })?
            .canonicalize()?,
        Err(error) => return Err(error),
    };
    if resolved.starts_with(&canonical_root) {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "resolved ADR baseline path escapes its trusted root",
        ))
    }
}

fn read_utf8_limited(path: &Path, limit: u64) -> std::io::Result<String> {
    let bytes = read_file_limited(path, limit)?;
    String::from_utf8(bytes)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

fn read_file_limited(path: &Path, limit: u64) -> std::io::Result<Vec<u8>> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "ADR baseline input exceeds the configured byte limit",
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "ADR baseline input exceeds the configured byte limit",
        ));
    }
    Ok(bytes)
}

fn create_snapshot_copy(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "ADR baseline copy has no parent")
    })?;
    let temporary = tempfile_path(parent, "snapshot")?;
    let result = (|| {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::hard_link(&temporary, path)?;
        fs::remove_file(&temporary)?;
        fs::File::open(parent)?.sync_all()
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn append_ledger_line(path: &Path, trusted_root: &Path, line: &str) -> std::io::Result<()> {
    reject_leaf_symlink(path)?;
    crate::track::symlink_guard::reject_symlinks_below(path, trusted_root)?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let mut record = line.as_bytes().to_vec();
    record.push(b'\n');
    write_ledger_record(&mut file, &record)?;
    file.sync_all()
}

pub(super) fn write_ledger_record(
    writer: &mut impl std::io::Write,
    record: &[u8],
) -> std::io::Result<()> {
    writer.write_all(record)
}

fn validate_ledger_append(path: &Path, trusted_root: &Path, line: &str) -> std::io::Result<()> {
    let record_len = line.len().checked_add(1).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "ADR baseline ledger record length overflows",
        )
    })?;
    if line.len() > MAX_LEDGER_LINE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "ADR baseline ledger record exceeds the configured line limit",
        ));
    }
    reject_leaf_symlink(path)?;
    crate::track::symlink_guard::reject_symlinks_below(path, trusted_root)?;
    let existing_len = match fs::metadata(path) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => return Err(error),
    };
    if existing_len > MAX_LEDGER_BYTES.saturating_sub(record_len as u64) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "ADR baseline ledger exceeds the configured byte limit",
        ));
    }
    let entry_count = read_ledger(path)
        .map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid ADR baseline ledger")
        })?
        .len();
    if entry_count >= MAX_LEDGER_ENTRIES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "ADR baseline ledger entry count exceeds the configured limit",
        ));
    }
    Ok(())
}

fn atomic_overwrite(path: &Path, trusted_root: &Path, bytes: &[u8]) -> std::io::Result<()> {
    reject_leaf_symlink(path)?;
    crate::track::symlink_guard::reject_symlinks_below(path, trusted_root)?;
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "ADR source path has no parent")
    })?;
    let temporary = tempfile_path(parent, "restore")?;
    let result = (|| {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        fs::File::open(parent)?.sync_all()
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn tempfile_path(parent: &Path, operation: &str) -> std::io::Result<PathBuf> {
    for attempt in 0..1024_u16 {
        let path =
            parent.join(format!(".adr-baseline-{operation}-{}-{attempt}", std::process::id()));
        if !path.try_exists()? {
            return Ok(path);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "unable to allocate ADR baseline temporary path",
    ))
}

fn reject_leaf_symlink(path: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("refusing to follow symlink: {}", path.display()),
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn content_hash(bytes: &[u8]) -> ContentHash {
    ContentHash::from_bytes(Sha256::digest(bytes).into())
}

fn io_read_error(error: std::io::Error) -> AdrBaselineStoreError {
    AdrBaselineStoreError::Read(diagnostic(&error.to_string()))
}

fn io_write_error(error: std::io::Error) -> AdrBaselineStoreError {
    AdrBaselineStoreError::Write(diagnostic(&error.to_string()))
}
