use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::Digest;

const CONTRACT_EMBEDDING_VERSION: &str = "1";
const MAX_CONTRACT_SOURCE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_CONTRACT_SOURCE_FILES: usize = 10_000;
const MAX_CONTRACT_SOURCE_ENTRIES: usize = 20_000;
const MAX_CONTRACT_LIBRARY_ENTRIES: usize = 1_000;
const MAX_CONTRACT_SOURCE_DEPTH: usize = 32;
const MAX_CONTRACT_SOURCE_TOTAL_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Default)]
struct ContractSourceBudget {
    files: usize,
    entries: usize,
    bytes: u64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let package_root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let libs_root = package_root.parent().ok_or("infrastructure package has no libs parent")?;
    let workspace_root = libs_root.parent().ok_or("libs directory has no workspace parent")?;
    let source_roots = core_source_roots(libs_root)?;

    let evaluator = hash_contract("type-signals-evaluator", workspace_root, &source_roots)?;
    let extraction = hash_contract("rustdoc-extraction", workspace_root, &source_roots)?;

    println!("cargo:rustc-env=SOTP_EVALUATOR_CONTRACT_DIGEST={evaluator}");
    println!("cargo:rustc-env=SOTP_RUSTDOC_EXTRACTION_CONTRACT_DIGEST={extraction}");
    Ok(())
}

/// Collects every workspace-library source root, manifest, and build script.
/// This covers the complete local code/build closure of infrastructure and its
/// local path dependencies while excluding runtime caches such as `tmp/`.
fn core_source_roots(libs_root: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut roots = Vec::new();
    let mut entries = 0_usize;
    for entry in fs::read_dir(libs_root)? {
        let entry = entry?;
        entries = entries.checked_add(1).ok_or("workspace library entry count overflow")?;
        if entries > MAX_CONTRACT_LIBRARY_ENTRIES {
            return Err(format!(
                "workspace library root '{}' exceeds maximum of {MAX_CONTRACT_LIBRARY_ENTRIES} entries",
                libs_root.display()
            )
            .into());
        }
        let package_root = entry.path();
        let metadata = fs::symlink_metadata(&package_root)?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "workspace library root '{}' must not be a symlink",
                package_root.display()
            )
            .into());
        }
        if !metadata.is_dir() {
            continue;
        }
        for path in [
            package_root.join("Cargo.toml"),
            package_root.join("build.rs"),
            package_root.join("src"),
        ] {
            if path.exists() {
                roots.push(path);
            }
        }
    }
    roots.sort();
    if roots.is_empty() {
        return Err(format!(
            "workspace library root '{}' has no source roots",
            libs_root.display()
        )
        .into());
    }
    Ok(roots)
}

fn hash_contract(
    contract_name: &str,
    workspace_root: &Path,
    source_roots: &[PathBuf],
) -> Result<String, Box<dyn Error>> {
    let mut hasher = sha2::Sha256::new();
    let workspace_manifest = workspace_root.join("Cargo.toml");
    let lockfile = workspace_root.join("Cargo.lock");
    println!("cargo:rerun-if-changed={}", workspace_manifest.display());
    println!("cargo:rerun-if-changed={}", lockfile.display());
    append_component(&mut hasher, "contract-name", contract_name.as_bytes());
    append_component(
        &mut hasher,
        "contract-embedding-version",
        CONTRACT_EMBEDDING_VERSION.as_bytes(),
    );
    append_file(&mut hasher, "contract-workspace-manifest", &workspace_manifest)?;
    append_file(&mut hasher, "contract-lockfile", &lockfile)?;

    let mut budget = ContractSourceBudget::default();
    let mut files = Vec::new();
    for source_root in source_roots {
        collect_contract_files(source_root, 0, &mut budget, &mut files)?;
    }
    files.sort();
    files.dedup();

    for file in files {
        println!("cargo:rerun-if-changed={}", file.display());
        let relative = file.strip_prefix(workspace_root)?;
        append_component(&mut hasher, "contract-source-path", normalized_path(relative).as_bytes());
        append_file(&mut hasher, "contract-source-bytes", &file)?;
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn collect_contract_files(
    path: &Path,
    depth: usize,
    budget: &mut ContractSourceBudget,
    files: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    if depth > MAX_CONTRACT_SOURCE_DEPTH {
        return Err(format!(
            "contract source '{}' exceeds maximum directory depth of {MAX_CONTRACT_SOURCE_DEPTH}",
            path.display()
        )
        .into());
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(format!("contract source '{}' must not be a symlink", path.display()).into());
    }
    if metadata.is_file() {
        if metadata.len() > MAX_CONTRACT_SOURCE_BYTES {
            return Err(format!(
                "contract source '{}' exceeds maximum size of {MAX_CONTRACT_SOURCE_BYTES} bytes",
                path.display()
            )
            .into());
        }
        budget.files = budget.files.checked_add(1).ok_or("contract source file count overflow")?;
        if budget.files > MAX_CONTRACT_SOURCE_FILES {
            return Err(format!(
                "contract source closure exceeds maximum of {MAX_CONTRACT_SOURCE_FILES} files"
            )
            .into());
        }
        budget.bytes = budget
            .bytes
            .checked_add(metadata.len())
            .ok_or("contract source byte count overflow")?;
        if budget.bytes > MAX_CONTRACT_SOURCE_TOTAL_BYTES {
            return Err(format!(
                "contract source closure exceeds maximum of {MAX_CONTRACT_SOURCE_TOTAL_BYTES} bytes"
            )
            .into());
        }
        files.push(path.to_path_buf());
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(
            format!("contract source '{}' is not a file or directory", path.display()).into()
        );
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        budget.entries =
            budget.entries.checked_add(1).ok_or("contract source entry count overflow")?;
        if budget.entries > MAX_CONTRACT_SOURCE_ENTRIES {
            return Err(format!(
                "contract source closure exceeds maximum of {MAX_CONTRACT_SOURCE_ENTRIES} entries"
            )
            .into());
        }
        collect_contract_files(&entry.path(), depth + 1, budget, files)?;
    }
    Ok(())
}

fn append_file(hasher: &mut sha2::Sha256, label: &str, path: &Path) -> Result<(), Box<dyn Error>> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("contract input '{}' must be a regular file", path.display()).into());
    }
    if metadata.len() > MAX_CONTRACT_SOURCE_BYTES {
        return Err(format!(
            "contract input '{}' exceeds maximum size of {MAX_CONTRACT_SOURCE_BYTES} bytes",
            path.display()
        )
        .into());
    }
    let bytes = fs::read(path)?;
    if bytes.len() as u64 > MAX_CONTRACT_SOURCE_BYTES {
        return Err(format!(
            "contract input '{}' exceeds maximum size of {MAX_CONTRACT_SOURCE_BYTES} bytes after read",
            path.display()
        )
        .into());
    }
    append_component(hasher, label, &bytes);
    Ok(())
}

fn append_component(hasher: &mut sha2::Sha256, label: &str, bytes: &[u8]) {
    hasher.update((label.len() as u64).to_be_bytes());
    hasher.update(label.as_bytes());
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn normalized_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
