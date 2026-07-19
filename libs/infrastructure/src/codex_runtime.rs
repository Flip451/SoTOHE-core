//! Filesystem-backed provisioning of the repository-local Codex runtime link.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use domain::tddd::test_obligation::ids::DiagnosticMessage;
use usecase::codex_runtime::{
    CodexRuntimeProjectRootDiscoveryError, CodexRuntimeProjectRootDiscoveryPort,
    CodexRuntimeProvisionError, CodexRuntimeProvisionPort,
};

const CODEX_NAME: &str = "codex";
const NPM_NAME: &str = "npm";
const RUNTIME_LINK: &str = ".harness/tools/bin/codex";
const RUNTIME_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const RUNTIME_PROBE_MAX_OUTPUT_BYTES: usize = 64 * 1024;
static NEXT_TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

/// Git-backed discovery adapter for implicit Codex runtime provisioning roots.
pub struct GitCodexRuntimeProjectRootDiscoveryAdapter;

impl GitCodexRuntimeProjectRootDiscoveryAdapter {
    /// Create the Git-backed project-root discovery adapter.
    #[must_use]
    pub fn new() -> GitCodexRuntimeProjectRootDiscoveryAdapter {
        GitCodexRuntimeProjectRootDiscoveryAdapter
    }
}

impl Default for GitCodexRuntimeProjectRootDiscoveryAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CodexRuntimeProjectRootDiscoveryPort for GitCodexRuntimeProjectRootDiscoveryAdapter {
    fn discover_from(
        &self,
        start_directory: &Path,
    ) -> Result<PathBuf, CodexRuntimeProjectRootDiscoveryError> {
        let mut command = Command::new("git");
        command.args(["rev-parse", "--show-toplevel"]);
        command.current_dir(start_directory);
        let output = crate::capability_exec::process::run_command_with_bounded_output(
            &mut command,
            RUNTIME_PROBE_MAX_OUTPUT_BYTES,
            RUNTIME_PROBE_TIMEOUT,
            "git rev-parse --show-toplevel",
        )
        .map_err(|error| {
            CodexRuntimeProjectRootDiscoveryError::GitRootDiscoveryFailed(diagnostic(format!(
                "failed to discover Git project root: {error}"
            )))
        })?;
        if !output.status.success() {
            return Err(CodexRuntimeProjectRootDiscoveryError::GitRootDiscoveryFailed(diagnostic(
                format!(
                    "Git root discovery exited with {}{}",
                    output.status,
                    stderr_detail(&output.stderr)
                ),
            )));
        }

        let root = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if root.is_empty() {
            return Err(CodexRuntimeProjectRootDiscoveryError::GitRootDiscoveryFailed(diagnostic(
                "Git root discovery returned an empty project root".to_owned(),
            )));
        }
        Ok(PathBuf::from(root))
    }
}

/// Filesystem adapter that verifies and links the Codex runtime for one repository.
pub struct FsCodexRuntimeProvisioner;

impl FsCodexRuntimeProvisioner {
    /// Create the filesystem-backed Codex runtime provisioner.
    #[must_use]
    pub fn new() -> FsCodexRuntimeProvisioner {
        FsCodexRuntimeProvisioner
    }

    fn provision_with_path(
        &self,
        project_root: &Path,
        path: Option<OsString>,
    ) -> Result<(), CodexRuntimeProvisionError> {
        if !project_root.is_dir() {
            return Err(CodexRuntimeProvisionError::ProjectRootInvalid(diagnostic(format!(
                "project root is not a directory: {}",
                project_root.display()
            ))));
        }

        let mut attempts = Vec::new();
        if let Some(candidate) = find_codex_on_path(project_root, path.as_deref()) {
            match probe(&candidate, path.as_deref(), None) {
                Ok(()) => return refresh_link(project_root, &candidate),
                Err(reason) => attempts.push(format!(
                    "PATH candidate {} failed sanitized --version probe: {reason}",
                    candidate.display()
                )),
            }
        } else {
            attempts.push("PATH candidate codex was not found".to_owned());
        }

        let npm_candidate = match npm_global_entry(path.as_deref()) {
            Ok(candidate) => candidate,
            Err(reason) => {
                attempts.push(format!("public npm fallback failed: {reason}"));
                return Err(CodexRuntimeProvisionError::NpmQueryFailed(diagnostic(format!(
                    "{}; install or repair a Codex entry, then rerun `cargo make bootstrap`",
                    attempts.join("; ")
                ))));
            }
        };

        match probe(&npm_candidate, path.as_deref(), npm_candidate.parent()) {
            Ok(()) => refresh_link(project_root, &npm_candidate),
            Err(reason) => {
                attempts.push(format!(
                    "public npm fallback {} failed sanitized --version probe: {reason}",
                    npm_candidate.display()
                ));
                Err(CodexRuntimeProvisionError::NoUsableCandidate(diagnostic(format!(
                    "{}; install or repair a Codex entry, then rerun `cargo make bootstrap`",
                    attempts.join("; ")
                ))))
            }
        }
    }
}

impl Default for FsCodexRuntimeProvisioner {
    fn default() -> Self {
        Self::new()
    }
}

impl CodexRuntimeProvisionPort for FsCodexRuntimeProvisioner {
    fn provision(&self, project_root: &Path) -> Result<(), CodexRuntimeProvisionError> {
        self.provision_with_path(project_root, std::env::var_os("PATH"))
    }
}

fn npm_global_entry(path: Option<&std::ffi::OsStr>) -> Result<PathBuf, String> {
    let npm = find_on_path(NPM_NAME, path)
        .ok_or_else(|| "`npm` was not found on PATH for `npm prefix -g`".to_owned())?;
    let mut command = Command::new(npm);
    command.args(["prefix", "-g"]);
    let output = crate::capability_exec::process::run_command_with_bounded_output(
        &mut command,
        RUNTIME_PROBE_MAX_OUTPUT_BYTES,
        RUNTIME_PROBE_TIMEOUT,
        "npm prefix -g",
    )
    .map_err(|error| format!("`npm prefix -g` could not complete: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "`npm prefix -g` exited with {}{}",
            output.status,
            stderr_detail(&output.stderr)
        ));
    }
    let prefix = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if prefix.is_empty() {
        return Err("`npm prefix -g` returned an empty prefix".to_owned());
    }
    let candidate = PathBuf::from(prefix).join("bin").join(CODEX_NAME);
    if is_executable(&candidate) {
        Ok(candidate)
    } else {
        Err(format!(
            "`npm prefix -g` public entry is absent or not executable: {}",
            candidate.display()
        ))
    }
}

fn probe(
    candidate: &Path,
    path: Option<&std::ffi::OsStr>,
    prefix: Option<&Path>,
) -> Result<(), String> {
    let home = SanitizedHome::new()?;
    let mut command = Command::new(candidate);
    command.arg("--version").env_clear().env("HOME", home.path());
    if let Some(probe_path) = probe_path(path, prefix)? {
        command.env("PATH", probe_path);
    }
    let output = crate::capability_exec::process::run_command_with_bounded_output(
        &mut command,
        RUNTIME_PROBE_MAX_OUTPUT_BYTES,
        RUNTIME_PROBE_TIMEOUT,
        "sanitized Codex runtime probe",
    )
    .map_err(|error| format!("could not complete: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("exited with {}{}", output.status, stderr_detail(&output.stderr)))
    }
}

fn probe_path(
    path: Option<&std::ffi::OsStr>,
    prefix: Option<&Path>,
) -> Result<Option<OsString>, String> {
    let Some(path) = path else {
        return Ok(prefix.map(PathBuf::from).map(Into::into));
    };
    match prefix {
        Some(prefix) => std::env::join_paths(
            std::iter::once(prefix.to_path_buf()).chain(std::env::split_paths(path)),
        )
        .map(Some)
        .map_err(|error| format!("cannot prepare sanitized PATH: {error}")),
        None => Ok(Some(path.to_owned())),
    }
}

#[cfg(unix)]
fn refresh_link(project_root: &Path, candidate: &Path) -> Result<(), CodexRuntimeProvisionError> {
    let link_target = absolute_link_target(candidate).map_err(|error| {
        CodexRuntimeProvisionError::LinkUpdateFailed(diagnostic(format!(
            "cannot make verified Codex entry absolute {}: {error}",
            candidate.display()
        )))
    })?;
    let pinned_link = pinned_runtime_link(project_root)?;

    match rustix::fs::statat(
        &pinned_link.parent_dir,
        &pinned_link.name,
        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Ok(metadata) if !rustix::fs::FileType::from_raw_mode(metadata.st_mode).is_symlink() => {
            return Err(CodexRuntimeProvisionError::LinkUpdateFailed(diagnostic(format!(
                "refusing to replace non-symlink runtime path: {}",
                pinned_link.path.display()
            ))));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(CodexRuntimeProvisionError::LinkUpdateFailed(diagnostic(format!(
                "cannot inspect runtime link {}: {error}",
                pinned_link.path.display()
            ))));
        }
    }

    let temporary =
        create_temporary_link(&link_target, &pinned_link.parent_dir).map_err(|error| {
            CodexRuntimeProvisionError::LinkUpdateFailed(diagnostic(format!(
                "cannot prepare runtime link in {}: {error}",
                pinned_link.parent.display()
            )))
        })?;
    rustix::fs::renameat(
        &pinned_link.parent_dir,
        &temporary,
        &pinned_link.parent_dir,
        &pinned_link.name,
    )
    .map_err(|error| {
        let _ =
            rustix::fs::unlinkat(&pinned_link.parent_dir, &temporary, rustix::fs::AtFlags::empty());
        CodexRuntimeProvisionError::LinkUpdateFailed(diagnostic(format!(
            "cannot refresh runtime symlink {}: {error}",
            pinned_link.path.display()
        )))
    })
}

#[cfg(not(unix))]
fn refresh_link(_project_root: &Path, _candidate: &Path) -> Result<(), CodexRuntimeProvisionError> {
    Err(CodexRuntimeProvisionError::LinkUpdateFailed(diagnostic(
        "Codex runtime provisioning requires descriptor-pinned filesystem operations on this platform"
            .to_owned(),
    )))
}

#[cfg(unix)]
struct PinnedRuntimeLink {
    path: PathBuf,
    parent: PathBuf,
    parent_dir: std::fs::File,
    name: OsString,
}

#[cfg(unix)]
fn pinned_runtime_link(
    project_root: &Path,
) -> Result<PinnedRuntimeLink, CodexRuntimeProvisionError> {
    let trusted_root = project_root.canonicalize().map_err(|error| {
        CodexRuntimeProvisionError::LinkUpdateFailed(diagnostic(format!(
            "cannot resolve project root {}: {error}",
            project_root.display()
        )))
    })?;
    let path = trusted_root.join(RUNTIME_LINK);
    let parent = path
        .parent()
        .ok_or_else(|| {
            CodexRuntimeProvisionError::LinkUpdateFailed(diagnostic(format!(
                "runtime link has no parent: {}",
                path.display()
            )))
        })?
        .to_path_buf();
    let name = OsString::from(CODEX_NAME);
    let parent_dir = open_runtime_link_directory(&trusted_root).map_err(|error| {
        CodexRuntimeProvisionError::LinkUpdateFailed(diagnostic(format!(
            "cannot open trusted runtime link directory {}: {error}",
            parent.display()
        )))
    })?;

    Ok(PinnedRuntimeLink { path, parent, parent_dir, name })
}

#[cfg(unix)]
fn open_runtime_link_directory(trusted_root: &Path) -> Result<std::fs::File, std::io::Error> {
    let mut directory = open_directory_nofollow(trusted_root)?;
    for component in [".harness", "tools", "bin"] {
        directory = open_or_create_directory(&directory, component)?;
    }
    Ok(directory)
}

#[cfg(unix)]
fn open_or_create_directory(
    parent_dir: &std::fs::File,
    name: &str,
) -> Result<std::fs::File, std::io::Error> {
    match open_directory_at_nofollow(parent_dir, name) {
        Ok(directory) => Ok(directory),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match rustix::fs::mkdirat(parent_dir, name, rustix::fs::Mode::from_raw_mode(0o777)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
            open_directory_at_nofollow(parent_dir, name)
        }
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn open_directory_at_nofollow(
    parent_dir: &std::fs::File,
    name: &str,
) -> Result<std::fs::File, std::io::Error> {
    rustix::fs::openat(
        parent_dir,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(std::fs::File::from)
    .map_err(Into::into)
}

#[cfg(unix)]
fn open_directory_nofollow(path: &Path) -> Result<std::fs::File, std::io::Error> {
    use std::os::unix::fs::OpenOptionsExt as _;

    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
}

fn absolute_link_target(candidate: &Path) -> Result<PathBuf, std::io::Error> {
    if candidate.is_absolute() {
        Ok(candidate.to_path_buf())
    } else {
        std::env::current_dir().map(|current_dir| current_dir.join(candidate))
    }
}

#[cfg(unix)]
fn create_temporary_link(
    candidate: &Path,
    parent_dir: &std::fs::File,
) -> Result<OsString, std::io::Error> {
    for _ in 0..128 {
        let temporary = OsString::from(format!(
            ".codex-link-{}-{}",
            std::process::id(),
            NEXT_TEMPORARY_ID.fetch_add(1, Ordering::Relaxed)
        ));
        match rustix::fs::symlinkat(candidate, parent_dir, &temporary) {
            Ok(()) => return Ok(temporary),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate a temporary Codex runtime link",
    ))
}

struct SanitizedHome {
    path: PathBuf,
}

impl SanitizedHome {
    fn new() -> Result<SanitizedHome, String> {
        let temporary_root = std::env::temp_dir();
        for _ in 0..128 {
            let path = temporary_root.join(format!(
                "sotp-codex-probe-{}-{}",
                std::process::id(),
                NEXT_TEMPORARY_ID.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(SanitizedHome { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(format!("cannot create sanitized HOME: {error}")),
            }
        }
        Err("cannot allocate a sanitized HOME directory".to_owned())
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for SanitizedHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn find_on_path(name: &str, path: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    path.and_then(|path| {
        std::env::split_paths(path)
            .map(|directory| directory.join(name))
            .find(|candidate| is_executable(candidate))
    })
}

fn find_codex_on_path(project_root: &Path, path: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    path.and_then(|path| {
        std::env::split_paths(path).map(|directory| directory.join(CODEX_NAME)).find(|candidate| {
            is_executable(candidate) && !is_repo_local_runtime_link(project_root, candidate)
        })
    })
}

fn is_repo_local_runtime_link(project_root: &Path, candidate: &Path) -> bool {
    let expected = absolute_lexical_path(&project_root.join(RUNTIME_LINK));
    let candidate = absolute_lexical_path(candidate);
    matches!((expected, candidate), (Ok(expected), Ok(candidate)) if expected == candidate)
}

fn absolute_lexical_path(path: &Path) -> Result<PathBuf, std::io::Error> {
    let absolute = absolute_link_target(path)?;
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            std::path::Component::RootDir => normalized.push(component.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::Normal(name) => normalized.push(name),
        }
    }
    Ok(normalized)
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;

    path.is_file()
        && std::fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

fn stderr_detail(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr).trim().to_owned();
    if text.is_empty() { String::new() } else { format!(" (stderr: {text})") }
}

fn diagnostic(message: String) -> DiagnosticMessage {
    let mut candidate = if message.trim().is_empty() {
        "Codex runtime provisioning failed".to_owned()
    } else {
        message
    };
    loop {
        match DiagnosticMessage::try_new(candidate) {
            Ok(diagnostic) => return diagnostic,
            Err(_) => candidate = "Codex runtime provisioning failed".to_owned(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use usecase::codex_runtime::{
        CodexRuntimeProjectRootDiscoveryPort, CodexRuntimeProvisionError, CodexRuntimeProvisionPort,
    };

    use super::{FsCodexRuntimeProvisioner, GitCodexRuntimeProjectRootDiscoveryAdapter};

    fn executable(path: &Path, body: &str) {
        fs::write(path, body).expect("test executable must be written");
        let mut permissions =
            fs::metadata(path).expect("test executable must be readable").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("test executable must be executable");
    }

    fn path_of(paths: &[PathBuf]) -> OsString {
        std::env::join_paths(paths).expect("test PATH must be valid")
    }

    #[test]
    fn test_git_project_root_discovery_returns_root_from_subdirectory() {
        let fixture = tempfile::tempdir().expect("fixture must be created");
        let repository_root = fixture.path().join("repository");
        let git_init = Command::new("git")
            .args(["init", "--quiet"])
            .arg(&repository_root)
            .output()
            .expect("git init must run");
        assert!(git_init.status.success(), "git init must create fixture repository");
        let subdirectory = repository_root.join("nested/command");
        fs::create_dir_all(&subdirectory).expect("fixture subdirectory must be created");
        let root = GitCodexRuntimeProjectRootDiscoveryAdapter::new()
            .discover_from(&subdirectory)
            .expect("discovery must return the fixture repository root");

        assert_eq!(root, repository_root);
    }

    #[test]
    fn test_provision_path_candidate_creates_and_refreshes_symlink() {
        let fixture = tempfile::tempdir().expect("fixture must be created");
        let project = fixture.path().join("project");
        let bin = fixture.path().join("bin");
        fs::create_dir_all(&project).expect("project must be created");
        fs::create_dir_all(&bin).expect("bin must be created");
        let first = bin.join("codex");
        executable(&first, "#!/bin/sh\necho codex-first\n");
        let provisioner = FsCodexRuntimeProvisioner::new();

        provisioner
            .provision_with_path(&project, Some(path_of(std::slice::from_ref(&bin))))
            .expect("PATH candidate must provision");
        let link = project.join(".harness/tools/bin/codex");
        assert_eq!(fs::read_link(&link).expect("link must exist"), first);

        let second = bin.join("codex-new");
        executable(&second, "#!/bin/sh\necho codex-second\n");
        fs::rename(&second, &first).expect("candidate may be replaced");
        provisioner
            .provision_with_path(&project, Some(path_of(&[bin])))
            .expect("repeat provisioning must refresh link");
        assert_eq!(fs::read_link(&link).expect("refreshed link must exist"), first);
    }

    #[test]
    fn test_provision_repo_local_path_entry_skips_link_and_remains_idempotent() {
        let fixture = tempfile::tempdir().expect("fixture must be created");
        let project = fixture.path().join("project");
        let external_bin = fixture.path().join("external-bin");
        fs::create_dir_all(&project).expect("project must be created");
        fs::create_dir_all(&external_bin).expect("external bin must be created");
        let external_codex = external_bin.join("codex");
        executable(&external_codex, "#!/bin/sh\necho codex-external\n");
        let provisioner = FsCodexRuntimeProvisioner::new();

        provisioner
            .provision_with_path(&project, Some(path_of(std::slice::from_ref(&external_bin))))
            .expect("external PATH candidate must provision");
        let runtime_bin = project.join(".harness/tools/bin");
        let link = runtime_bin.join("codex");

        provisioner
            .provision_with_path(&project, Some(path_of(&[runtime_bin, external_bin])))
            .expect("repo-local PATH entry must be skipped in favor of the external candidate");

        assert_eq!(fs::read_link(&link).expect("runtime link must remain healthy"), external_codex);
        assert_eq!(
            link.canonicalize().expect("runtime link must resolve"),
            external_codex.canonicalize().expect("external candidate must resolve")
        );
    }

    #[test]
    fn test_provision_uses_public_npm_entry_with_colocated_runtime() {
        let fixture = tempfile::tempdir().expect("fixture must be created");
        let project = fixture.path().join("project");
        let path_bin = fixture.path().join("path-bin");
        let npm_bin = fixture.path().join("npm-bin");
        let global_prefix = fixture.path().join("npm-prefix");
        let global_bin = global_prefix.join("bin");
        fs::create_dir_all(&project).expect("project must be created");
        fs::create_dir_all(&path_bin).expect("PATH bin must be created");
        fs::create_dir_all(&npm_bin).expect("npm bin must be created");
        fs::create_dir_all(&global_bin).expect("global bin must be created");
        executable(&path_bin.join("codex"), "#!/bin/sh\necho path-shim >&2\nexit 1\n");
        executable(
            &npm_bin.join("npm"),
            &format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", global_prefix.display()),
        );
        executable(&global_bin.join("node"), "#!/bin/sh\necho node\n");
        executable(&global_bin.join("codex"), "#!/bin/sh\nnode --version\n");

        FsCodexRuntimeProvisioner::new()
            .provision_with_path(&project, Some(path_of(&[path_bin, npm_bin])))
            .expect("verified public npm entry must provision");
        assert_eq!(
            fs::read_link(project.join(".harness/tools/bin/codex")).expect("link must exist"),
            global_bin.join("codex")
        );
    }

    #[test]
    fn test_provision_failed_candidates_reports_attempts_and_creates_no_link() {
        let fixture = tempfile::tempdir().expect("fixture must be created");
        let project = fixture.path().join("project");
        let path_bin = fixture.path().join("path-bin");
        let npm_bin = fixture.path().join("npm-bin");
        let global_prefix = fixture.path().join("npm-prefix");
        let global_bin = global_prefix.join("bin");
        fs::create_dir_all(&project).expect("project must be created");
        fs::create_dir_all(&path_bin).expect("PATH bin must be created");
        fs::create_dir_all(&npm_bin).expect("npm bin must be created");
        fs::create_dir_all(&global_bin).expect("global bin must be created");
        executable(&path_bin.join("codex"), "#!/bin/sh\necho path failure >&2\nexit 1\n");
        executable(
            &npm_bin.join("npm"),
            &format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", global_prefix.display()),
        );
        executable(&global_bin.join("codex"), "#!/bin/sh\necho npm failure >&2\nexit 2\n");

        let error = FsCodexRuntimeProvisioner::new()
            .provision_with_path(&project, Some(path_of(&[path_bin, npm_bin])))
            .expect_err("unverified candidates must fail");
        assert!(matches!(error, CodexRuntimeProvisionError::NoUsableCandidate(_)));
        let diagnostic = error.to_string();
        assert!(diagnostic.contains("PATH candidate"));
        assert!(diagnostic.contains("public npm fallback"));
        assert!(diagnostic.contains("rerun `cargo make bootstrap`"));
        assert!(!project.join(".harness/tools/bin/codex").exists());
    }

    #[test]
    fn test_provision_port_rejects_missing_project_root() {
        let fixture = tempfile::tempdir().expect("fixture must be created");
        let error = FsCodexRuntimeProvisioner::new()
            .provision(&fixture.path().join("missing"))
            .expect_err("missing project root must fail");
        assert!(matches!(error, CodexRuntimeProvisionError::ProjectRootInvalid(_)));
    }

    #[test]
    fn test_provision_rejects_symlinked_runtime_link_parent() {
        let fixture = tempfile::tempdir().expect("fixture must be created");
        let project = fixture.path().join("project");
        let bin = fixture.path().join("bin");
        let outside = fixture.path().join("outside");
        fs::create_dir_all(&project).expect("project must be created");
        fs::create_dir_all(&bin).expect("bin must be created");
        fs::create_dir_all(&outside).expect("outside directory must be created");
        std::os::unix::fs::symlink(&outside, project.join(".harness"))
            .expect("runtime directory symlink must be created");
        executable(&bin.join("codex"), "#!/bin/sh\necho codex\n");

        let error = FsCodexRuntimeProvisioner::new()
            .provision_with_path(&project, Some(path_of(&[bin])))
            .expect_err("symlinked runtime directory must be rejected");

        assert!(matches!(error, CodexRuntimeProvisionError::LinkUpdateFailed(_)));
        assert!(!outside.join("tools/bin/codex").exists());
    }
}
