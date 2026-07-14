//! Resolved Cargo build-input closure hashing for type-signal freshness.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::time::Duration;

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use sha2::Digest;

use domain::tddd::type_signals_doc::Sha256Digest;

use super::EvaluateSignalsError;

const MAX_BUILD_INPUT_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_BUILD_INPUT_COMMAND_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_BUILD_INPUT_COMMAND_DURATION: Duration = Duration::from_secs(120);
const MAX_LOCAL_SOURCE_FILES: usize = 10_000;
const MAX_LOCAL_SOURCE_ENTRIES: usize = 20_000;
const MAX_LOCAL_SOURCE_DEPTH: usize = 32;
const MAX_LOCAL_SOURCE_BYTES: u64 = 256 * 1024 * 1024;
const EXECUTABLE_OVERRIDE_ENVIRONMENT_KEYS: [&str; 8] = [
    "RUSTDOC",
    "CARGO_BUILD_RUSTDOC",
    "RUSTC",
    "CARGO_BUILD_RUSTC",
    "RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
];
const EXECUTABLE_OVERRIDE_CONFIG_KEYS: [&str; 4] =
    ["rustdoc", "rustc", "rustc-wrapper", "rustc-workspace-wrapper"];

#[derive(Default)]
struct LocalSourceBudget {
    files: usize,
    entries: usize,
    bytes: u64,
}

/// Hashes the complete resolved Cargo input closure for one rustdoc target.
///
/// The hash is deliberately conservative: unresolved metadata, config, source
/// files, toolchain identity, or environment values return an error so the
/// caller cannot take a snapshot-reuse path.
///
/// # Errors
///
/// Returns [`EvaluateSignalsError`] when Cargo cannot resolve the package graph
/// or when any declared build input cannot be read and normalized.
pub(super) fn hash_resolved_build_inputs(
    workspace_root: &Path,
    target_crate: &str,
) -> Result<Sha256Digest, EvaluateSignalsError> {
    let configs = cargo_configs(workspace_root)?;
    let environment = normalized_environment()?;
    reject_compiler_overrides(&environment, &configs)?;
    let toolchain = nightly_toolchain_identity()?;
    let target_triple = resolved_target_triple(&configs, &toolchain)?;
    let target_specification = resolved_target_specification(workspace_root, &target_triple)?;
    let metadata = cargo_metadata(workspace_root)?;
    let closure = resolved_package_closure(&metadata, target_crate)?;

    let mut hasher = sha2::Sha256::new();
    append_component(&mut hasher, "target-crate", target_crate.as_bytes());
    append_component(&mut hasher, "target-triple", target_triple.as_bytes());
    append_component(&mut hasher, "target-specification", &target_specification);
    append_component(&mut hasher, "nightly-rustc", &toolchain);
    append_file(&mut hasher, "workspace-manifest", &workspace_root.join("Cargo.toml"))?;
    append_file(&mut hasher, "lockfile", &workspace_root.join("Cargo.lock"))?;
    for (path, bytes) in configs {
        append_component(&mut hasher, "cargo-config-path", normalized_path(&path).as_bytes());
        append_component(&mut hasher, "cargo-config", &bytes);
    }
    append_environment(&mut hasher, environment);

    let mut local_source_budget = LocalSourceBudget::default();
    for package in closure {
        let package_id = required_str(&package, "id")?;
        append_component(&mut hasher, "package-id", package_id.as_bytes());
        append_component(
            &mut hasher,
            "package-source",
            package.get("source").and_then(serde_json::Value::as_str).unwrap_or("path").as_bytes(),
        );
        let features = canonical_json(package.get("resolved_features"))?;
        append_component(&mut hasher, "package-features", &features);
        let is_local_package = package.get("source").is_none_or(serde_json::Value::is_null);
        if is_local_package {
            let manifest = required_path(&package, "manifest_path")?;
            let package_root = validate_local_package_root(workspace_root, &manifest)?;
            append_file(&mut hasher, "package-manifest", &manifest)?;
            reject_target_sources_outside_package(&package, &package_root)?;
            append_local_package_sources(&mut hasher, &manifest, &mut local_source_budget)?;
        }
        reject_unresolved_build_execution(&package)?;
    }

    Sha256Digest::try_new(format!("{:x}", hasher.finalize())).map_err(|error| {
        EvaluateSignalsError(format!("failed to construct build-input digest: {error}"))
    })
}

/// Validates a local Cargo package root before it contributes any files to the
/// freshness closure. Path dependencies outside the workspace are not trusted
/// inputs for snapshot reuse, even when Cargo metadata resolves them.
fn validate_local_package_root(
    workspace_root: &Path,
    manifest: &Path,
) -> Result<PathBuf, EvaluateSignalsError> {
    let package_root = manifest.parent().ok_or_else(|| {
        EvaluateSignalsError(format!("package manifest '{}' has no parent", manifest.display()))
    })?;
    crate::track::symlink_guard::reject_symlinks_up_to_root(package_root).map_err(|error| {
        EvaluateSignalsError(format!(
            "cannot authorize local package root '{}': symlink guard rejected it: {error}",
            package_root.display()
        ))
    })?;
    let canonical_workspace = workspace_root.canonicalize().map_err(|error| {
        EvaluateSignalsError(format!(
            "cannot authorize workspace root '{}': {error}",
            workspace_root.display()
        ))
    })?;
    let canonical_package = package_root.canonicalize().map_err(|error| {
        EvaluateSignalsError(format!(
            "cannot authorize local package root '{}': {error}",
            package_root.display()
        ))
    })?;
    if canonical_package.starts_with(&canonical_workspace) {
        Ok(canonical_package)
    } else {
        Err(EvaluateSignalsError(format!(
            "cannot authorize local package root '{}': it resolves outside workspace root '{}'; refusing stale snapshot reuse",
            package_root.display(),
            workspace_root.display()
        )))
    }
}

/// Cargo metadata can declare a target source outside the manifest directory.
/// The local-package walker only has a complete closure when every such source
/// is lexically contained by that directory; otherwise snapshot reuse must
/// fail closed rather than omit the external target from its identity.
fn reject_target_sources_outside_package(
    package: &serde_json::Value,
    package_root: &Path,
) -> Result<(), EvaluateSignalsError> {
    let targets =
        package.get("targets").and_then(serde_json::Value::as_array).ok_or_else(|| {
            EvaluateSignalsError("Cargo metadata package has no targets array".to_owned())
        })?;
    for target in targets {
        let source_path = required_path(target, "src_path")?;
        crate::track::symlink_guard::reject_symlinks_up_to_root(&source_path).map_err(|error| {
            EvaluateSignalsError(format!(
                "cannot authorize Cargo target source '{}': symlink guard rejected it: {error}",
                source_path.display()
            ))
        })?;
        let canonical_source = source_path.canonicalize().map_err(|error| {
            EvaluateSignalsError(format!(
                "cannot authorize Cargo target source '{}': {error}",
                source_path.display()
            ))
        })?;
        if !canonical_source.starts_with(package_root) {
            return Err(EvaluateSignalsError(format!(
                "cannot resolve complete build-input closure: Cargo target source '{}' for package '{}' escapes package root '{}'; refusing stale snapshot reuse",
                source_path.display(),
                required_str(package, "id")?,
                package_root.display()
            )));
        }
    }
    Ok(())
}

fn reject_unresolved_build_execution(
    package: &serde_json::Value,
) -> Result<(), EvaluateSignalsError> {
    let targets =
        package.get("targets").and_then(serde_json::Value::as_array).ok_or_else(|| {
            EvaluateSignalsError("Cargo metadata package has no targets array".to_owned())
        })?;
    if targets.iter().any(|target| target_has_kind(target, "proc-macro")) {
        return Err(EvaluateSignalsError(format!(
            "cannot resolve proc-macro inputs for '{}'; refusing stale snapshot reuse",
            required_str(package, "id")?
        )));
    }
    let has_custom_build_target =
        targets.iter().any(|target| target_has_kind(target, "custom-build"));
    if has_custom_build_target && !uses_hermetic_contract_digest_build_script(package)? {
        return Err(EvaluateSignalsError(format!(
            "cannot resolve build-script inputs for '{}'; refusing stale snapshot reuse",
            required_str(package, "id")?
        )));
    }
    Ok(())
}

fn target_has_kind(target: &serde_json::Value, expected_kind: &str) -> bool {
    target
        .get("kind")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some(expected_kind)))
}

/// The infrastructure build script embeds digests computed solely from inputs
/// already covered by this closure: its source, package manifest, workspace
/// manifest, lockfile, and tracked source files. It declares every source
/// dependency with `rerun-if-changed`, so this single script needs no separate
/// runtime input model. All other build scripts remain fail-closed.
fn uses_hermetic_contract_digest_build_script(
    package: &serde_json::Value,
) -> Result<bool, EvaluateSignalsError> {
    if !package.get("source").is_none_or(serde_json::Value::is_null)
        || required_str(package, "name")? != "infrastructure"
    {
        return Ok(false);
    }
    let manifest = required_path(package, "manifest_path")?;
    let package_root = manifest.parent().ok_or_else(|| {
        EvaluateSignalsError(format!("package manifest '{}' has no parent", manifest.display()))
    })?;
    let targets =
        package.get("targets").and_then(serde_json::Value::as_array).ok_or_else(|| {
            EvaluateSignalsError("Cargo metadata package has no targets array".to_owned())
        })?;
    Ok(targets.iter().any(|target| {
        target
            .get("kind")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some("custom-build")))
            && target.get("src_path").and_then(serde_json::Value::as_str)
                == package_root.join("build.rs").to_str()
    }))
}

#[path = "build_inputs/metadata.rs"]
mod metadata;

use metadata::{
    cargo_configs, cargo_metadata, nightly_toolchain_identity, resolved_package_closure,
};

fn resolved_target_triple(
    configs: &[(PathBuf, Vec<u8>)],
    toolchain: &[u8],
) -> Result<String, EvaluateSignalsError> {
    if let Some(target) = std::env::var_os("CARGO_BUILD_TARGET") {
        return target.into_string().map_err(|_| {
            EvaluateSignalsError(
                "CARGO_BUILD_TARGET is not valid Unicode; cannot normalize build inputs".to_owned(),
            )
        });
    }
    let mut configured_target = None;
    for (path, bytes) in configs {
        let value = parse_cargo_config(path, bytes)?;
        if let Some(target) = value.get("build").and_then(|build| build.get("target")) {
            configured_target = Some(
                target.as_str().map(str::to_owned).ok_or_else(|| {
                    EvaluateSignalsError(format!(
                        "Cargo config '{}' has a non-string build.target; cannot normalize build inputs",
                        path.display()
                    ))
                })?,
            );
        }
    }
    configured_target
        .or_else(|| {
            std::str::from_utf8(toolchain).ok().and_then(|identity| {
                identity.lines().find_map(|line| line.strip_prefix("host: ").map(str::to_owned))
            })
        })
        .ok_or_else(|| EvaluateSignalsError("nightly rustc identity has no host target".to_owned()))
}

/// Resolves the effective rustc target specification, including custom target
/// JSON content. Hashing the target argument alone would permit snapshot reuse
/// after a custom target file changes at the same path.
fn resolved_target_specification(
    workspace_root: &Path,
    target: &str,
) -> Result<Vec<u8>, EvaluateSignalsError> {
    let mut command = Command::new("rustup");
    command
        .args([
            "run",
            "nightly",
            "rustc",
            "-Z",
            "unstable-options",
            "--print",
            "target-spec-json",
            "--target",
            target,
        ])
        .current_dir(workspace_root);
    let output = crate::capability_exec::process::run_command_with_bounded_output(
        &mut command,
        MAX_BUILD_INPUT_COMMAND_OUTPUT_BYTES,
        MAX_BUILD_INPUT_COMMAND_DURATION,
        "nightly rustc target specification",
    )
    .map_err(|error| {
        EvaluateSignalsError(format!("cannot resolve target specification for '{target}': {error}"))
    })?;
    if !output.status.success() {
        return Err(EvaluateSignalsError(format!(
            "cannot resolve target specification for '{target}': rustup run nightly rustc exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    if output.stdout.is_empty() {
        return Err(EvaluateSignalsError(format!(
            "cannot resolve target specification for '{target}': rustc returned no specification"
        )));
    }
    Ok(output.stdout)
}

fn normalized_environment() -> Result<BTreeMap<String, String>, EvaluateSignalsError> {
    let mut environment = BTreeMap::new();
    for (key, value) in std::env::vars_os() {
        let key = key.into_string().map_err(|_| {
            EvaluateSignalsError(
                "build environment key is not valid Unicode; cannot normalize build inputs"
                    .to_owned(),
            )
        })?;
        let value = value.into_string().map_err(|_| {
            EvaluateSignalsError(format!(
                "build environment value for '{key}' is not valid Unicode"
            ))
        })?;
        environment.insert(key, value);
    }
    Ok(environment)
}

fn reject_compiler_overrides(
    environment: &BTreeMap<String, String>,
    configs: &[(PathBuf, Vec<u8>)],
) -> Result<(), EvaluateSignalsError> {
    for key in EXECUTABLE_OVERRIDE_ENVIRONMENT_KEYS {
        if environment.contains_key(key) {
            return Err(EvaluateSignalsError(format!(
                "cannot normalize build inputs: {key} overrides the compiler or rustdoc executable; refusing snapshot reuse"
            )));
        }
    }
    for (path, bytes) in configs {
        let value = parse_cargo_config(path, bytes)?;
        let Some(build) = value.get("build").and_then(toml::Value::as_table) else {
            continue;
        };
        for key in EXECUTABLE_OVERRIDE_CONFIG_KEYS {
            if build.contains_key(key) {
                return Err(EvaluateSignalsError(format!(
                    "cannot normalize build inputs: Cargo config '{}' overrides build.{key}; refusing snapshot reuse",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

fn append_environment(hasher: &mut sha2::Sha256, environment: BTreeMap<String, String>) {
    for (key, value) in environment {
        append_component(hasher, "environment-key", key.as_bytes());
        append_component(hasher, "environment-value", value.as_bytes());
    }
}

fn parse_cargo_config(path: &Path, bytes: &[u8]) -> Result<toml::Value, EvaluateSignalsError> {
    let text = std::str::from_utf8(bytes).map_err(|error| {
        EvaluateSignalsError(format!("Cargo config '{}' is not UTF-8: {error}", path.display()))
    })?;
    toml::from_str(text).map_err(|error| {
        EvaluateSignalsError(format!("cannot parse Cargo config '{}': {error}", path.display()))
    })
}

fn append_local_package_sources(
    hasher: &mut sha2::Sha256,
    manifest: &Path,
    budget: &mut LocalSourceBudget,
) -> Result<(), EvaluateSignalsError> {
    let root = manifest.parent().ok_or_else(|| {
        EvaluateSignalsError(format!("package manifest '{}' has no parent", manifest.display()))
    })?;
    let mut files = Vec::new();
    collect_package_files(root, 0, budget, &mut files)?;
    for file in files {
        let relative = file.strip_prefix(root).map_err(|error| {
            EvaluateSignalsError(format!(
                "cannot normalize package source '{}': {error}",
                file.display()
            ))
        })?;
        let bytes = read_regular_file(&file)?;
        reject_untracked_rust_source_inputs(&file, &bytes)?;
        append_component(hasher, "package-source-path", normalized_path(relative).as_bytes());
        append_component(hasher, "package-source-bytes", &bytes);
    }
    Ok(())
}

/// Rejects local Rust sources whose compiler input closure cannot be derived
/// solely from package-tree traversal. These directives may read a source or
/// data file outside the package directory, so snapshot reuse must fail closed.
fn reject_untracked_rust_source_inputs(
    path: &Path,
    bytes: &[u8],
) -> Result<(), EvaluateSignalsError> {
    if path.extension().is_none_or(|extension| extension != "rs") {
        return Ok(());
    }
    let source = std::str::from_utf8(bytes).map_err(|error| {
        EvaluateSignalsError(format!(
            "cannot parse Rust source '{}': source is not UTF-8 ({error}); refusing stale snapshot reuse",
            path.display()
        ))
    })?;
    syn::parse_file(source).map_err(|error| {
        EvaluateSignalsError(format!(
            "cannot parse Rust source '{}': {error}; refusing stale snapshot reuse",
            path.display()
        ))
    })?;
    if contains_external_rust_input_directive(source)? {
        return Err(EvaluateSignalsError(format!(
            "cannot resolve complete build-input closure: Rust source '{}' contains include!/include_str!/include_bytes!, #[path], or cfg_attr(..., path = ...) and may read outside its package; refusing stale snapshot reuse",
            path.display()
        )));
    }
    Ok(())
}

/// Detects source directives with a token stream instead of byte matching so
/// whitespace and comments cannot conceal compiler-visible inputs.
fn contains_external_rust_input_directive(source: &str) -> Result<bool, EvaluateSignalsError> {
    let tokens = TokenStream::from_str(source).map_err(|error| {
        EvaluateSignalsError(format!(
            "cannot tokenize Rust source while resolving build inputs: {error}; refusing stale snapshot reuse"
        ))
    })?;
    Ok(contains_external_input_macro(&tokens) || contains_path_attribute(&tokens))
}

fn contains_external_input_macro(tokens: &TokenStream) -> bool {
    let trees: Vec<TokenTree> = tokens.clone().into_iter().collect();
    trees.iter().zip(trees.iter().skip(1)).any(|(first, second)| {
        matches!(
            (first, second),
            (TokenTree::Ident(name), TokenTree::Punct(punctuation))
                if is_external_input_macro(name.to_string().as_str())
                    && punctuation.as_char() == '!'
        )
    }) || trees.iter().any(|tree| {
        matches!(tree, TokenTree::Group(group) if contains_external_input_macro(&group.stream()))
    })
}

fn is_external_input_macro(name: &str) -> bool {
    matches!(name, "include" | "include_str" | "include_bytes")
}

fn contains_path_attribute(tokens: &TokenStream) -> bool {
    let trees: Vec<TokenTree> = tokens.clone().into_iter().collect();
    trees.iter().zip(trees.iter().skip(1)).any(|(first, second)| {
        matches!(
            (first, second),
            (TokenTree::Punct(hash), TokenTree::Group(group))
                if hash.as_char() == '#' && group.delimiter() == Delimiter::Bracket
                    && contains_path_assignment(&group.stream())
        )
    }) || trees.iter().any(
        |tree| matches!(tree, TokenTree::Group(group) if contains_path_attribute(&group.stream())),
    )
}

fn contains_path_assignment(tokens: &TokenStream) -> bool {
    let trees: Vec<TokenTree> = tokens.clone().into_iter().collect();
    trees.iter().zip(trees.iter().skip(1)).any(|(first, second)| {
        matches!(
            (first, second),
            (TokenTree::Ident(name), TokenTree::Punct(equals))
                if name == "path" && equals.as_char() == '='
        )
    }) || trees.iter().any(
        |tree| matches!(tree, TokenTree::Group(group) if contains_path_assignment(&group.stream())),
    )
}

fn collect_package_files(
    directory: &Path,
    depth: usize,
    budget: &mut LocalSourceBudget,
    files: &mut Vec<PathBuf>,
) -> Result<(), EvaluateSignalsError> {
    if depth > MAX_LOCAL_SOURCE_DEPTH {
        return Err(EvaluateSignalsError(format!(
            "cannot normalize build input: package source '{}' exceeds maximum directory depth of {MAX_LOCAL_SOURCE_DEPTH}",
            directory.display()
        )));
    }
    for entry in std::fs::read_dir(directory).map_err(|error| {
        EvaluateSignalsError(format!(
            "cannot read package source directory '{}': {error}",
            directory.display()
        ))
    })? {
        let entry = entry.map_err(|error| {
            EvaluateSignalsError(format!(
                "cannot read package source entry '{}': {error}",
                directory.display()
            ))
        })?;
        budget.entries = budget.entries.checked_add(1).ok_or_else(|| {
            EvaluateSignalsError(
                "cannot normalize build input: package source entry count overflow".to_owned(),
            )
        })?;
        if budget.entries > MAX_LOCAL_SOURCE_ENTRIES {
            return Err(EvaluateSignalsError(format!(
                "cannot normalize build input: local package closure exceeds maximum of {MAX_LOCAL_SOURCE_ENTRIES} entries"
            )));
        }
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
            EvaluateSignalsError(format!(
                "cannot stat package source '{}': {error}",
                path.display()
            ))
        })?;
        if metadata.file_type().is_symlink() {
            return Err(EvaluateSignalsError(format!(
                "cannot normalize build input: package source '{}' is a symlink",
                path.display()
            )));
        }
        if metadata.is_dir() {
            collect_package_files(&path, depth + 1, budget, files)?;
        } else if metadata.is_file() {
            if metadata.len() > MAX_BUILD_INPUT_FILE_BYTES {
                return Err(EvaluateSignalsError(format!(
                    "cannot normalize build input: package source '{}' exceeds maximum file size of {MAX_BUILD_INPUT_FILE_BYTES} bytes",
                    path.display()
                )));
            }
            budget.files = budget.files.checked_add(1).ok_or_else(|| {
                EvaluateSignalsError(
                    "cannot normalize build input: package source file count overflow".to_owned(),
                )
            })?;
            if budget.files > MAX_LOCAL_SOURCE_FILES {
                return Err(EvaluateSignalsError(format!(
                    "cannot normalize build input: local package closure exceeds maximum of {MAX_LOCAL_SOURCE_FILES} files"
                )));
            }
            budget.bytes = budget.bytes.checked_add(metadata.len()).ok_or_else(|| {
                EvaluateSignalsError(
                    "cannot normalize build input: package source byte count overflow".to_owned(),
                )
            })?;
            if budget.bytes > MAX_LOCAL_SOURCE_BYTES {
                return Err(EvaluateSignalsError(format!(
                    "cannot normalize build input: local package closure exceeds maximum of {MAX_LOCAL_SOURCE_BYTES} bytes"
                )));
            }
            files.push(path);
        } else {
            return Err(EvaluateSignalsError(format!(
                "cannot normalize build input: package source '{}' is not a regular file",
                path.display()
            )));
        }
    }
    files.sort();
    Ok(())
}

fn append_file(
    hasher: &mut sha2::Sha256,
    label: &str,
    path: &Path,
) -> Result<(), EvaluateSignalsError> {
    let bytes = read_regular_file(path)?;
    append_component(hasher, label, &bytes);
    Ok(())
}

fn read_optional_regular_file(path: &Path) -> Result<Option<Vec<u8>>, EvaluateSignalsError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(EvaluateSignalsError(format!(
            "cannot read build input '{}': symlinks are unsupported",
            path.display()
        ))),
        Ok(metadata) if !metadata.is_file() => Err(EvaluateSignalsError(format!(
            "cannot read build input '{}': not a regular file",
            path.display()
        ))),
        Ok(_) => read_regular_file(path).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(EvaluateSignalsError(format!(
            "cannot stat build input '{}': {error}",
            path.display()
        ))),
    }
}

fn read_regular_file(path: &Path) -> Result<Vec<u8>, EvaluateSignalsError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        EvaluateSignalsError(format!("cannot stat build input '{}': {error}", path.display()))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(EvaluateSignalsError(format!(
            "cannot read build input '{}': symlinks are unsupported",
            path.display()
        )));
    }
    if !metadata.is_file() {
        return Err(EvaluateSignalsError(format!(
            "cannot read build input '{}': not a regular file",
            path.display()
        )));
    }
    if metadata.len() > MAX_BUILD_INPUT_FILE_BYTES {
        return Err(EvaluateSignalsError(format!(
            "cannot read build input '{}': exceeds maximum size of {MAX_BUILD_INPUT_FILE_BYTES} bytes",
            path.display()
        )));
    }
    let bytes = std::fs::read(path).map_err(|error| {
        EvaluateSignalsError(format!("cannot read build input '{}': {error}", path.display()))
    })?;
    if bytes.len() as u64 > MAX_BUILD_INPUT_FILE_BYTES {
        return Err(EvaluateSignalsError(format!(
            "cannot read build input '{}': exceeds maximum size of {MAX_BUILD_INPUT_FILE_BYTES} bytes after read",
            path.display()
        )));
    }
    Ok(bytes)
}

fn append_component(hasher: &mut sha2::Sha256, label: &str, bytes: &[u8]) {
    hasher.update((label.len() as u64).to_be_bytes());
    hasher.update(label.as_bytes());
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn canonical_json(value: Option<&serde_json::Value>) -> Result<Vec<u8>, EvaluateSignalsError> {
    value.map_or_else(
        || Ok(Vec::new()),
        |value| {
            serde_json::to_vec(value).map_err(|error| {
                EvaluateSignalsError(format!("cannot normalize Cargo metadata value: {error}"))
            })
        },
    )
}

fn required_str<'a>(
    value: &'a serde_json::Value,
    field: &str,
) -> Result<&'a str, EvaluateSignalsError> {
    value.get(field).and_then(serde_json::Value::as_str).ok_or_else(|| {
        EvaluateSignalsError(format!("Cargo metadata field '{field}' is missing or not a string"))
    })
}

fn required_path(value: &serde_json::Value, field: &str) -> Result<PathBuf, EvaluateSignalsError> {
    required_str(value, field).map(PathBuf::from)
}

fn normalized_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::{
        LocalSourceBudget, MAX_BUILD_INPUT_FILE_BYTES, MAX_LOCAL_SOURCE_ENTRIES,
        collect_package_files, contains_external_rust_input_directive, hash_resolved_build_inputs,
        nightly_toolchain_identity, read_regular_file, reject_compiler_overrides,
        reject_target_sources_outside_package, reject_unresolved_build_execution,
        reject_untracked_rust_source_inputs, resolved_target_specification, resolved_target_triple,
        validate_local_package_root,
    };

    fn nightly_toolchain_available() -> bool {
        Command::new("rustup")
            .args(["run", "nightly", "rustc", "-Vv"])
            .status()
            .is_ok_and(|status| status.success())
    }

    fn write_workspace(root: &Path) {
        fs::create_dir_all(root.join("crates/alpha/src")).unwrap();
        fs::create_dir_all(root.join("crates/beta/src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/alpha\", \"crates/beta\"]\nresolver = \"2\"\n",
        )
        .unwrap();
        fs::write(
            root.join("Cargo.lock"),
            "version = 4\n\n[[package]]\nname = \"alpha\"\nversion = \"0.1.0\"\ndependencies = [\"beta\"]\n\n[[package]]\nname = \"beta\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(
            root.join("crates/alpha/Cargo.toml"),
            "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nbeta = { path = \"../beta\", features = [\"first\"] }\n",
        )
        .unwrap();
        fs::write(root.join("crates/alpha/src/lib.rs"), "pub fn alpha() {}\n").unwrap();
        fs::write(
            root.join("crates/beta/Cargo.toml"),
            "[package]\nname = \"beta\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[features]\nfirst = []\nsecond = []\n",
        )
        .unwrap();
        fs::write(root.join("crates/beta/src/lib.rs"), "pub fn beta() {}\n").unwrap();
    }

    #[test]
    fn test_hash_resolved_build_inputs_covers_dependency_sources_manifests_and_features() {
        if !nightly_toolchain_available() {
            eprintln!("skipping build-input closure lane: nightly toolchain is unavailable");
            return;
        }

        let workspace = tempfile::tempdir().unwrap();
        write_workspace(workspace.path());

        let initial = hash_resolved_build_inputs(workspace.path(), "alpha").unwrap();
        fs::write(workspace.path().join("crates/beta/src/lib.rs"), "pub fn beta_changed() {}\n")
            .unwrap();
        let dependency_source_changed =
            hash_resolved_build_inputs(workspace.path(), "alpha").unwrap();
        assert_ne!(
            initial, dependency_source_changed,
            "dependency source belongs to alpha's closure"
        );

        fs::write(
            workspace.path().join("crates/beta/Cargo.toml"),
            "[package]\nname = \"beta\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\ndescription = \"changed manifest\"\n\n[features]\nfirst = []\nsecond = []\n",
        )
        .unwrap();
        let dependency_manifest_changed =
            hash_resolved_build_inputs(workspace.path(), "alpha").unwrap();
        assert_ne!(
            dependency_source_changed, dependency_manifest_changed,
            "dependency manifest belongs to alpha's resolved closure"
        );

        fs::write(
            workspace.path().join("crates/alpha/Cargo.toml"),
            "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nbeta = { path = \"../beta\", features = [\"second\"] }\n",
        )
        .unwrap();
        let feature_selection_changed =
            hash_resolved_build_inputs(workspace.path(), "alpha").unwrap();
        assert_ne!(
            dependency_manifest_changed, feature_selection_changed,
            "resolved dependency features belong to alpha's closure"
        );
    }

    #[test]
    fn test_hash_resolved_build_inputs_covers_custom_target_specification_contents() {
        if !nightly_toolchain_available() {
            eprintln!("skipping custom target lane: nightly toolchain is unavailable");
            return;
        }

        let workspace = tempfile::tempdir().unwrap();
        write_workspace(workspace.path());
        let toolchain = nightly_toolchain_identity().unwrap();
        let host_target = resolved_target_triple(&[], &toolchain).unwrap();
        let target_path = workspace.path().join("custom-target.json");
        let target_specification =
            resolved_target_specification(workspace.path(), &host_target).unwrap();
        fs::write(&target_path, &target_specification).unwrap();
        fs::create_dir_all(workspace.path().join(".cargo")).unwrap();
        fs::write(
            workspace.path().join(".cargo/config.toml"),
            "[build]\ntarget = \"custom-target.json\"\n",
        )
        .unwrap();

        let initial = hash_resolved_build_inputs(workspace.path(), "alpha").unwrap();
        let mut changed_target: serde_json::Value =
            serde_json::from_slice(&target_specification).unwrap();
        changed_target
            .get_mut("metadata")
            .and_then(serde_json::Value::as_object_mut)
            .unwrap()
            .insert(
                "description".to_owned(),
                serde_json::Value::String("changed custom target description".to_owned()),
            );
        fs::write(&target_path, serde_json::to_vec_pretty(&changed_target).unwrap()).unwrap();

        let changed = hash_resolved_build_inputs(workspace.path(), "alpha").unwrap();

        assert_ne!(
            initial, changed,
            "custom target specification content belongs to the build-input closure"
        );
    }

    #[test]
    fn test_hash_resolved_build_inputs_local_build_script_returns_error() {
        if !nightly_toolchain_available() {
            eprintln!("skipping build-input closure lane: nightly toolchain is unavailable");
            return;
        }

        let workspace = tempfile::tempdir().unwrap();
        write_workspace(workspace.path());
        fs::write(
            workspace.path().join("crates/alpha/Cargo.toml"),
            "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\nedition = \"2024\"\nbuild = \"build.rs\"\n\n[dependencies]\nbeta = { path = \"../beta\", features = [\"first\"] }\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("crates/alpha/build.rs"),
            "fn main() { println!(\"cargo:rerun-if-changed=../../shared-schema.json\"); }\n",
        )
        .unwrap();

        let error = hash_resolved_build_inputs(workspace.path(), "alpha").unwrap_err();

        assert!(error.to_string().contains("build-script inputs"));
    }

    #[test]
    fn test_reject_unresolved_build_execution_hermetic_build_script_returns_ok() {
        let directory = tempfile::tempdir().unwrap();
        let manifest = directory.path().join("Cargo.toml");
        let build_script = directory.path().join("build.rs");
        let package = serde_json::json!({
            "id": "path+file:///workspace/libs/infrastructure#0.1.0",
            "name": "infrastructure",
            "manifest_path": manifest,
            "targets": [{"kind": ["custom-build"], "src_path": build_script}],
        });

        assert!(reject_unresolved_build_execution(&package).is_ok());
    }

    #[test]
    fn test_reject_unresolved_build_execution_registry_build_script_returns_error() {
        let directory = tempfile::tempdir().unwrap();
        let manifest = directory.path().join("Cargo.toml");
        let build_script = directory.path().join("build.rs");
        let package = serde_json::json!({
            "id": "registry+https://example.invalid/registry#fixture@1.0.0",
            "name": "fixture",
            "source": "registry+https://example.invalid/registry",
            "manifest_path": manifest,
            "targets": [{"kind": ["custom-build"], "src_path": build_script}],
        });

        let error = reject_unresolved_build_execution(&package).unwrap_err();

        assert!(error.to_string().contains("build-script inputs"));
    }

    #[test]
    fn test_reject_unresolved_build_execution_proc_macro_returns_error() {
        let directory = tempfile::tempdir().unwrap();
        let manifest = directory.path().join("Cargo.toml");
        let source = directory.path().join("src/lib.rs");
        let package = serde_json::json!({
            "id": "registry+https://example.invalid/registry#fixture@1.0.0",
            "name": "fixture",
            "source": "registry+https://example.invalid/registry",
            "manifest_path": manifest,
            "targets": [{"kind": ["proc-macro"], "src_path": source}],
        });

        let error = reject_unresolved_build_execution(&package).unwrap_err();

        assert!(error.to_string().contains("proc-macro inputs"));
    }

    #[test]
    fn test_reject_target_sources_outside_package_returns_error() {
        let directory = tempfile::tempdir().unwrap();
        let package_root = directory.path().join("crate");
        let source_path = directory.path().join("shared/lib.rs");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::write(&source_path, "pub struct Shared;").unwrap();
        let package = serde_json::json!({
            "id": "path+file:///workspace/crate#0.1.0",
            "targets": [{"src_path": source_path}],
        });

        let error = reject_target_sources_outside_package(&package, &package_root).unwrap_err();

        assert!(error.to_string().contains("escapes package root"));
    }

    #[test]
    fn test_validate_local_package_root_outside_workspace_returns_error() {
        let workspace = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let manifest = external.path().join("Cargo.toml");
        fs::write(&manifest, "[package]\nname = \"external\"\nversion = \"0.1.0\"\n").unwrap();

        let error = validate_local_package_root(workspace.path(), &manifest).unwrap_err();

        assert!(error.to_string().contains("outside workspace root"));
    }

    #[test]
    fn test_reject_compiler_overrides_rejects_rustdoc_environment_override() {
        let environment = BTreeMap::from([("RUSTDOC".to_owned(), "/tmp/rustdoc".to_owned())]);

        let error = reject_compiler_overrides(&environment, &[]).unwrap_err();

        assert!(error.to_string().contains("RUSTDOC overrides"));
    }

    #[test]
    fn test_reject_compiler_overrides_rejects_configured_rustdoc_override() {
        let config_path = PathBuf::from(".cargo/config.toml");
        let configs = vec![(config_path, b"[build]\nrustdoc = 'custom-rustdoc'\n".to_vec())];

        let error = reject_compiler_overrides(&BTreeMap::new(), &configs).unwrap_err();

        assert!(error.to_string().contains("build.rustdoc"));
    }

    #[test]
    fn test_collect_package_files_includes_unusual_source_directories() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("tmp")).unwrap();
        fs::create_dir(directory.path().join("target")).unwrap();
        fs::write(directory.path().join("tmp/lib.rs"), "pub fn tmp() {}\n").unwrap();
        fs::write(directory.path().join("target/lib.rs"), "pub fn target() {}\n").unwrap();
        fs::write(directory.path().join("lib.rs"), "pub fn alpha() {}\n").unwrap();
        let mut budget = LocalSourceBudget::default();
        let mut files = Vec::new();

        collect_package_files(directory.path(), 0, &mut budget, &mut files).unwrap();

        assert_eq!(
            files,
            vec![
                directory.path().join("lib.rs"),
                directory.path().join("target/lib.rs"),
                directory.path().join("tmp/lib.rs"),
            ]
        );
    }

    #[test]
    fn test_collect_package_files_directory_entry_limit_returns_error() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("nested")).unwrap();
        let mut budget =
            LocalSourceBudget { entries: MAX_LOCAL_SOURCE_ENTRIES, ..LocalSourceBudget::default() };
        let mut files = Vec::new();

        let error =
            collect_package_files(directory.path(), 0, &mut budget, &mut files).unwrap_err();

        assert!(error.to_string().contains("maximum of"));
        assert!(error.to_string().contains("entries"));
    }

    #[test]
    fn test_reject_untracked_rust_source_inputs_external_directive_returns_error() {
        for source in [
            b"include!(\"../../shared.rs\");".as_slice(),
            b"include /* comment */ !(\"../../shared.rs\");".as_slice(),
            b"include_str!(\"../../shared.txt\");".as_slice(),
            b"include_bytes!(\"../../shared.bin\");".as_slice(),
            b"#[path = \"../../shared.rs\"] mod shared;".as_slice(),
            b"#[cfg_attr(feature = \"x\", path = \"../../shared.rs\")] mod shared;".as_slice(),
        ] {
            assert!(
                contains_external_rust_input_directive(std::str::from_utf8(source).unwrap())
                    .unwrap()
            );
            let error =
                reject_untracked_rust_source_inputs(Path::new("src/lib.rs"), source).unwrap_err();
            assert!(error.to_string().contains("complete build-input closure"));
        }
    }

    #[test]
    fn test_reject_untracked_rust_source_inputs_normal_rust_source_is_allowed() {
        assert!(
            reject_untracked_rust_source_inputs(
                Path::new("src/lib.rs"),
                b"// include /* comment */ !\nconst PATH: &str = \"#[path = \\\"shared.rs\\\"]\";"
            )
            .is_ok()
        );
    }

    #[test]
    fn test_reject_untracked_rust_source_inputs_invalid_rust_returns_error() {
        let error =
            reject_untracked_rust_source_inputs(Path::new("src/lib.rs"), b"pub fn {").unwrap_err();

        assert!(error.to_string().contains("cannot parse Rust source"));
    }

    #[cfg(unix)]
    #[test]
    fn test_read_regular_file_symlinked_input_returns_error() {
        let directory = tempfile::tempdir().unwrap();
        let outside = directory.path().join("outside.toml");
        let input = directory.path().join("Cargo.toml");
        fs::write(&outside, "[workspace]\n").unwrap();
        std::os::unix::fs::symlink(&outside, &input).unwrap();

        let error = read_regular_file(&input).unwrap_err();

        assert!(error.to_string().contains("symlinks are unsupported"));
    }

    #[test]
    fn test_read_regular_file_oversized_input_returns_error() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("Cargo.lock");
        fs::File::create(&input).unwrap().set_len(MAX_BUILD_INPUT_FILE_BYTES + 1).unwrap();

        let error = read_regular_file(&input).unwrap_err();

        assert!(error.to_string().contains("exceeds maximum size"));
    }

    #[test]
    fn test_collect_package_files_oversized_source_returns_error() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("generated.rs");
        fs::File::create(&input).unwrap().set_len(MAX_BUILD_INPUT_FILE_BYTES + 1).unwrap();
        let mut budget = LocalSourceBudget::default();
        let mut files = Vec::new();

        let error =
            collect_package_files(directory.path(), 0, &mut budget, &mut files).unwrap_err();

        assert!(error.to_string().contains("exceeds maximum file size"));
    }
}
