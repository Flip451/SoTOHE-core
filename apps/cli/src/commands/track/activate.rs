use crate::CliError;

use super::*;

pub(super) fn execute_branch(action: BranchAction) -> Result<ExitCode, CliError> {
    match action {
        BranchAction::Create(args) => execute_activate(
            ActivateArgs { items_dir: args.items_dir, track_id: args.track_id },
            BranchMode::Create,
        ),
        BranchAction::Switch(args) => execute_activate(
            ActivateArgs { items_dir: args.items_dir, track_id: args.track_id },
            BranchMode::Switch,
        ),
    }
}

pub(super) fn execute_activate(args: ActivateArgs, mode: BranchMode) -> Result<ExitCode, CliError> {
    let ActivateArgs { items_dir, track_id } = args;

    let track_id = TrackId::new(&track_id)
        .map_err(|err| CliError::Message(format!("invalid track id: {err}")))?;

    let branch_name = format!("track/{track_id}");

    let branch = TrackBranch::new(&branch_name)
        .map_err(|err| CliError::Message(format!("invalid track branch: {err}")))?;

    let project_root = resolve_project_root(&items_dir).map_err(CliError::Message)?;

    let repo = SystemGitRepo::discover()
        .map_err(|err| CliError::Message(format!("failed to discover git repository: {err}")))?;

    let store = Arc::new(FsTrackStore::new(items_dir.clone()));
    let activation = ActivateTrackUseCase::new(Arc::clone(&store));

    let track_record = load_track_branch_record(&project_root, &items_dir, &track_id)
        .map_err(|err| CliError::Message(format!("activation failed: {err}")))?;

    if uses_legacy_branch_mode(mode, track_record.schema_version) {
        return execute_legacy_branch_mode(&repo, &branch_name, mode);
    }

    let already_materialized = track_record.branch.is_some();
    let current_branch = repo.current_branch().map_err(|err| {
        CliError::Message(format!("failed to determine current branch before activation: {err}"))
    })?;
    if !already_materialized && current_branch.as_deref() == Some(branch_name.as_str()) {
        return Err(CliError::Message(format!(
            "activation preflight failed: branch '{branch_name}' is already checked out; rerun /track:activate from a non-track branch so materialized metadata is committed before switching"
        )));
    }
    if activation_rejects_invalid_source_branch(
        mode,
        already_materialized,
        current_branch.as_deref(),
    ) {
        return Err(CliError::Message(
            "activation preflight failed: activation must start from a non-track source branch; switch to 'main' or another non-track branch and rerun".to_owned()
        ));
    }
    if activation_create_requires_main_branch(mode, already_materialized, current_branch.as_deref())
    {
        return Err(CliError::Message(format!(
            "activation preflight failed: track branch creation must start from 'main'; switch to main or use /track:activate {track_id} instead"
        )));
    }

    let resume_allowed = if already_materialized && mode == BranchMode::Auto {
        activation_resume_allowed(
            &repo,
            &project_root,
            &items_dir,
            &track_id,
            &branch_name,
            track_record.status.as_deref().unwrap_or("planned"),
            current_branch.as_deref(),
        )
        .map_err(|err| CliError::Message(format!("activation preflight failed: {err}")))?
    } else {
        false
    };

    if already_materialized && !allow_materialized_activation(mode, resume_allowed) {
        return Err(CliError::Message(format!(
            "activation failed: track '{track_id}' is already materialized on branch '{branch_name}'; use that branch directly instead of rerunning /track:activate"
        )));
    }

    let should_persist_side_effects =
        should_persist_activation_side_effects(mode, already_materialized, resume_allowed);

    let allowed_dirty_paths = allowed_activation_dirty_paths(
        &project_root,
        &items_dir,
        &track_id,
        mode,
        already_materialized,
        resume_allowed,
    );
    if activation_requires_clean_worktree(mode, already_materialized, resume_allowed) {
        ensure_clean_worktree(&repo, &allowed_dirty_paths)
            .map_err(|err| CliError::Message(format!("activation preflight failed: {err}")))?;
    }

    let branch_exists =
        preflight_branch_operation(&repo, &branch_name, mode, !already_materialized)
            .map_err(|err| CliError::Message(format!("activation preflight failed: {err}")))?;

    let materialized_now = if already_materialized {
        false
    } else {
        match activation.execute(&track_id, &branch, track_record.schema_version) {
            Ok(ActivateTrackOutcome::Materialized(_)) => true,
            Err(err) => {
                return Err(CliError::Message(format!("activation failed: {err}")));
            }
        }
    };

    let created_activation_commit = if should_persist_side_effects {
        let rendered_paths = render::sync_rendered_views(&project_root, Some(track_id.as_str()))
            .map_err(|err| {
                CliError::Message(format!("activation persisted but sync-views failed: {err}"))
            })?;
        for path in &rendered_paths {
            match path.strip_prefix(&project_root) {
                Ok(relative) => println!("[OK] Rendered: {}", relative.display()),
                Err(_) => println!("[OK] Rendered: {}", path.display()),
            }
        }

        persist_activation_commit(&repo, &project_root, &items_dir, &track_id, &rendered_paths)
            .map_err(|err| {
                CliError::Message(format!(
                    "activation persisted but activation commit failed: {err}"
                ))
            })?
    } else {
        false
    };
    let resume_marker_present = activation_resume_marker_exists(&project_root, &track_id);
    let resume_marker_armed =
        if mode == BranchMode::Auto && (materialized_now || created_activation_commit) {
            write_activation_resume_marker(&project_root, &track_id)
                .map_err(|err| CliError::Message(format!("activation failed: {err}")))?;
            true
        } else {
            false
        };

    let create_from = activation_branch_create_base(
        &repo,
        &track_id,
        &branch_name,
        mode,
        branch_exists,
        materialized_now,
    )
    .map_err(|err| CliError::Message(format!("activation preflight failed: {err}")))?;
    let git_commands = activation_git_commands(
        mode,
        &branch_name,
        branch_exists,
        materialized_now,
        current_branch.as_deref(),
        create_from.as_deref(),
    );
    for command in &git_commands {
        let args = command.iter().map(String::as_str).collect::<Vec<_>>();
        match repo.status(&args) {
            Ok(0) => {}
            Ok(_) => {
                return Err(CliError::Message(format!(
                    "git {} failed after metadata materialization; rerun `cargo run --quiet -p cli -- track activate {track_id}` to resume",
                    args.join(" ")
                )));
            }
            Err(err) => {
                return Err(CliError::Message(format!(
                    "failed to run git {} after metadata materialization: {err}. rerun `cargo run --quiet -p cli -- track activate {track_id}` to resume",
                    args.join(" ")
                )));
            }
        }
    }
    if mode == BranchMode::Auto && (resume_marker_present || resume_marker_armed) {
        clear_activation_resume_marker(&project_root, &track_id).map_err(|err| {
            CliError::Message(format!("activation succeeded but cleanup failed: {err}"))
        })?;
    }

    if materialized_now {
        println!("[OK] Materialized branch metadata: {branch_name}");
    } else {
        println!("[OK] Branch metadata already materialized: {branch_name}");
    }
    if created_activation_commit {
        println!("[OK] Created activation commit");
    }
    println!(
        "[OK] {} branch: {}",
        activation_switch_label(
            mode,
            branch_exists,
            current_branch.as_deref() == Some(branch_name.as_str())
        ),
        branch_name
    );
    Ok(ExitCode::SUCCESS)
}

pub(super) fn execute_legacy_branch_mode(
    repo: &impl GitRepository,
    branch_name: &str,
    mode: BranchMode,
) -> Result<ExitCode, CliError> {
    let branch_exists = preflight_branch_operation(repo, branch_name, mode, false)
        .map_err(|err| CliError::Message(format!("legacy branch preflight failed: {err}")))?;
    let current_branch = repo
        .current_branch()
        .map_err(|err| CliError::Message(format!("failed to determine current branch: {err}")))?;
    let create_from = matches!(mode, BranchMode::Create).then_some("main");
    let git_commands = activation_git_commands(
        mode,
        branch_name,
        branch_exists,
        false,
        current_branch.as_deref(),
        create_from,
    );
    for command in &git_commands {
        let args = command.iter().map(String::as_str).collect::<Vec<_>>();
        match repo.status(&args) {
            Ok(0) => {}
            Ok(_) => {
                return Err(CliError::Message(format!("git {} failed", args.join(" "))));
            }
            Err(err) => {
                return Err(CliError::Message(format!(
                    "failed to run git {}: {err}",
                    args.join(" ")
                )));
            }
        }
    }

    println!("[OK] Legacy track left branch metadata unchanged");
    println!(
        "[OK] {} branch: {}",
        activation_switch_label(
            mode,
            branch_exists,
            current_branch.as_deref() == Some(branch_name)
        ),
        branch_name
    );
    Ok(ExitCode::SUCCESS)
}

pub(super) fn load_track_branch_record(
    project_root: &std::path::Path,
    items_dir: &std::path::Path,
    track_id: &TrackId,
) -> Result<TrackBranchRecord, String> {
    let track_dir = items_dir.join(track_id.as_str());
    load_explicit_track_branch_from_items_dir(project_root, items_dir, &track_dir)
}

fn persist_activation_commit(
    repo: &impl GitRepository,
    project_root: &std::path::Path,
    items_dir: &std::path::Path,
    track_id: &TrackId,
    rendered_paths: &[PathBuf],
) -> Result<bool, String> {
    let metadata_path = items_dir
        .join(track_id.as_str())
        .join("metadata.json")
        .strip_prefix(project_root)
        .unwrap_or(&items_dir.join(track_id.as_str()).join("metadata.json"))
        .display()
        .to_string();
    let mut staged = std::collections::BTreeSet::from([metadata_path]);
    for path in rendered_paths {
        let relative = path.strip_prefix(project_root).unwrap_or(path);
        staged.insert(relative.display().to_string());
    }
    let staged_paths = staged.into_iter().collect::<Vec<_>>();

    let mut status_args = vec!["status".to_owned(), "--porcelain".to_owned(), "--".to_owned()];
    status_args.extend(staged_paths.iter().cloned());
    let status_refs = status_args.iter().map(String::as_str).collect::<Vec<_>>();
    let status_output = repo.output(&status_refs).map_err(|e| e.to_string())?;
    if !status_output.status.success() {
        return Err("git status failed while checking activation artifacts".to_owned());
    }
    if String::from_utf8_lossy(&status_output.stdout).trim().is_empty() {
        return Ok(false);
    }

    let mut add_args = vec!["add".to_owned(), "--".to_owned()];
    add_args.extend(staged_paths.iter().cloned());
    let add_refs = add_args.iter().map(String::as_str).collect::<Vec<_>>();
    if repo.status(&add_refs).map_err(|e| e.to_string())? != 0 {
        return Err("git add failed while preparing activation commit".to_owned());
    }

    let message = format!("track: activate {track_id}");
    let mut commit_args =
        vec!["commit".to_owned(), "-m".to_owned(), message, "--only".to_owned(), "--".to_owned()];
    commit_args.extend(staged_paths);
    let commit_refs = commit_args.iter().map(String::as_str).collect::<Vec<_>>();
    if repo.status(&commit_refs).map_err(|e| e.to_string())? != 0 {
        return Err("git commit failed while persisting activation materialization".to_owned());
    }

    Ok(true)
}

fn activation_resume_marker_path(
    project_root: &std::path::Path,
    track_id: &TrackId,
) -> std::path::PathBuf {
    project_root.join("tmp/track-activate").join(format!("{}.pending", track_id.as_str()))
}

fn activation_resume_marker_exists(project_root: &std::path::Path, track_id: &TrackId) -> bool {
    activation_resume_marker_path(project_root, track_id).is_file()
}

fn write_activation_resume_marker(
    project_root: &std::path::Path,
    track_id: &TrackId,
) -> Result<(), String> {
    let marker_path = activation_resume_marker_path(project_root, track_id);
    if let Some(parent) = marker_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!("failed to prepare activation resume marker directory: {err}")
        })?;
    }
    std::fs::write(&marker_path, b"pending\n")
        .map_err(|err| format!("failed to write activation resume marker: {err}"))
}

fn clear_activation_resume_marker(
    project_root: &std::path::Path,
    track_id: &TrackId,
) -> Result<(), String> {
    let marker_path = activation_resume_marker_path(project_root, track_id);
    match std::fs::remove_file(&marker_path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!("failed to clear activation resume marker: {err}")),
    }
}

fn activation_git_commands(
    mode: BranchMode,
    branch_name: &str,
    branch_exists: bool,
    materialized_now: bool,
    current_branch: Option<&str>,
    create_from: Option<&str>,
) -> Vec<Vec<String>> {
    if current_branch == Some(branch_name) {
        return Vec::new();
    }

    match (mode, branch_exists, materialized_now) {
        (BranchMode::Create, _, _) => vec![vec![
            "switch".to_owned(),
            "-c".to_owned(),
            branch_name.to_owned(),
            create_from.unwrap_or("main").to_owned(),
        ]],
        (BranchMode::Switch, _, true) => vec![
            vec!["branch".to_owned(), "-f".to_owned(), branch_name.to_owned(), "HEAD".to_owned()],
            vec!["switch".to_owned(), branch_name.to_owned()],
        ],
        (BranchMode::Auto, true, true) => vec![
            vec!["branch".to_owned(), "-f".to_owned(), branch_name.to_owned(), "HEAD".to_owned()],
            vec!["switch".to_owned(), branch_name.to_owned()],
        ],
        (BranchMode::Auto, true, false) if create_from.is_some() => vec![
            vec![
                "branch".to_owned(),
                "-f".to_owned(),
                branch_name.to_owned(),
                create_from.unwrap_or("HEAD").to_owned(),
            ],
            vec!["switch".to_owned(), branch_name.to_owned()],
        ],
        (BranchMode::Switch, _, false) | (BranchMode::Auto, true, false) => {
            vec![vec!["switch".to_owned(), branch_name.to_owned()]]
        }
        (BranchMode::Auto, false, _) => vec![vec![
            "switch".to_owned(),
            "-c".to_owned(),
            branch_name.to_owned(),
            create_from.unwrap_or("HEAD").to_owned(),
        ]],
    }
}

fn activation_branch_create_base(
    repo: &impl GitRepository,
    track_id: &TrackId,
    branch_name: &str,
    mode: BranchMode,
    branch_exists: bool,
    materialized_now: bool,
) -> Result<Option<String>, String> {
    match mode {
        BranchMode::Create => Ok((!branch_exists).then_some("main".to_owned())),
        BranchMode::Switch => Ok(None),
        BranchMode::Auto if materialized_now => {
            if branch_exists {
                Ok(None)
            } else {
                Ok(Some("HEAD".to_owned()))
            }
        }
        BranchMode::Auto => {
            let Some(commit) = find_latest_activation_commit(repo, track_id)? else {
                return if branch_exists {
                    Ok(None)
                } else {
                    Err(format!(
                        "branch 'track/{track_id}' is missing after activation; cannot resume safely without an activation commit"
                    ))
                };
            };

            if !branch_exists {
                return Ok(Some(commit));
            }

            let branch_head = rev_parse_oid(repo, branch_name)?
                .ok_or_else(|| format!("cannot resolve existing branch '{branch_name}'"))?;
            if branch_head == commit {
                return Ok(None);
            }
            if is_ancestor(repo, &branch_head, &commit)? {
                return Ok(Some(commit));
            }
            if is_ancestor(repo, &commit, &branch_head)? {
                return Ok(None);
            }

            Err(format!(
                "branch '{branch_name}' diverged from activation commit; cannot resume safely"
            ))
        }
    }
}

fn activation_resume_allowed(
    repo: &impl GitRepository,
    project_root: &std::path::Path,
    items_dir: &std::path::Path,
    track_id: &TrackId,
    branch_name: &str,
    status: &str,
    current_branch: Option<&str>,
) -> Result<bool, String> {
    if current_branch == Some(branch_name) || status != "planned" {
        return Ok(false);
    }

    if activation_resume_marker_exists(project_root, track_id) {
        return Ok(true);
    }

    if find_latest_activation_commit(repo, track_id)?.is_some() {
        return Ok(false);
    }

    activation_artifacts_dirty(repo, project_root, items_dir, track_id)
}

fn allow_materialized_activation(mode: BranchMode, resume_allowed: bool) -> bool {
    match mode {
        BranchMode::Auto => resume_allowed,
        BranchMode::Switch => true,
        BranchMode::Create => true,
    }
}

fn should_persist_activation_side_effects(
    mode: BranchMode,
    already_materialized: bool,
    resume_allowed: bool,
) -> bool {
    if !already_materialized {
        return true;
    }
    matches!(mode, BranchMode::Auto) && resume_allowed
}

fn activation_requires_clean_worktree(
    mode: BranchMode,
    already_materialized: bool,
    resume_allowed: bool,
) -> bool {
    if !already_materialized {
        return true;
    }
    matches!(mode, BranchMode::Auto) && resume_allowed
}

fn allowed_activation_dirty_paths(
    project_root: &std::path::Path,
    items_dir: &std::path::Path,
    track_id: &TrackId,
    mode: BranchMode,
    already_materialized: bool,
    resume_allowed: bool,
) -> std::collections::BTreeSet<String> {
    if matches!(mode, BranchMode::Auto) && already_materialized && resume_allowed {
        activation_artifact_paths(project_root, items_dir, track_id)
    } else {
        std::collections::BTreeSet::new()
    }
}

fn activation_artifact_paths(
    project_root: &std::path::Path,
    items_dir: &std::path::Path,
    track_id: &TrackId,
) -> std::collections::BTreeSet<String> {
    let metadata_path = items_dir
        .join(track_id.as_str())
        .join("metadata.json")
        .strip_prefix(project_root)
        .unwrap_or(&items_dir.join(track_id.as_str()).join("metadata.json"))
        .display()
        .to_string();
    let plan_path = items_dir
        .join(track_id.as_str())
        .join("plan.md")
        .strip_prefix(project_root)
        .unwrap_or(&items_dir.join(track_id.as_str()).join("plan.md"))
        .display()
        .to_string();
    std::collections::BTreeSet::from([metadata_path, plan_path, "track/registry.md".to_owned()])
}

pub(super) fn ensure_clean_worktree(
    repo: &(impl GitRepository + domain::WorktreeReader),
    allowed_dirty_paths: &std::collections::BTreeSet<String>,
) -> Result<(), String> {
    usecase::worktree_guard::ensure_clean_worktree(repo, allowed_dirty_paths)
        .map_err(|e| e.to_string())
}

fn activation_create_requires_main_branch(
    mode: BranchMode,
    already_materialized: bool,
    current_branch: Option<&str>,
) -> bool {
    mode == BranchMode::Create && !already_materialized && current_branch != Some("main")
}

fn activation_rejects_invalid_source_branch(
    mode: BranchMode,
    already_materialized: bool,
    current_branch: Option<&str>,
) -> bool {
    let invalid = current_branch.is_none()
        || current_branch == Some("HEAD")
        || current_branch.is_some_and(|branch| branch.starts_with("track/"));

    match mode {
        BranchMode::Auto => invalid,
        BranchMode::Switch => !already_materialized && invalid,
        BranchMode::Create => false,
    }
}

pub(super) fn uses_legacy_branch_mode(mode: BranchMode, schema_version: u32) -> bool {
    schema_version != 3 && !matches!(mode, BranchMode::Auto)
}

/// Fetches dirty worktree paths via git, delegating parsing to the usecase layer.
fn git_dirty_worktree_paths(repo: &impl GitRepository) -> Result<Vec<String>, String> {
    let output = repo.output(&["status", "--porcelain"]).map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("git status --porcelain failed".to_owned());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(usecase::worktree_guard::parse_dirty_worktree_paths(&stdout))
}

fn activation_artifacts_dirty(
    repo: &impl GitRepository,
    project_root: &std::path::Path,
    items_dir: &std::path::Path,
    track_id: &TrackId,
) -> Result<bool, String> {
    let artifact_paths = activation_artifact_paths(project_root, items_dir, track_id);
    let dirty_paths = git_dirty_worktree_paths(repo)?;
    Ok(dirty_paths.iter().any(|path| artifact_paths.contains(path)))
}

fn find_latest_activation_commit(
    repo: &impl GitRepository,
    track_id: &TrackId,
) -> Result<Option<String>, String> {
    let message = format!("^track: activate {track_id}$");
    let output = repo
        .output(&["log", "-n", "1", "--format=%H", "--grep", message.as_str()])
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("git log failed while locating activation commit".to_owned());
    }

    let commit = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if commit.is_empty() { Ok(None) } else { Ok(Some(commit)) }
}

fn is_ancestor(
    repo: &impl GitRepository,
    ancestor: &str,
    descendant: &str,
) -> Result<bool, String> {
    let output = repo
        .output(&["merge-base", "--is-ancestor", ancestor, descendant])
        .map_err(|e| e.to_string())?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        Some(code) => Err(format!(
            "git merge-base --is-ancestor failed while comparing '{ancestor}' and '{descendant}' (exit {code})"
        )),
        None => Err("git merge-base --is-ancestor terminated by signal".to_owned()),
    }
}

fn activation_switch_label(
    mode: BranchMode,
    branch_exists: bool,
    already_on_branch: bool,
) -> &'static str {
    if already_on_branch {
        return "Already on";
    }

    match mode {
        BranchMode::Create => "Created and switched to",
        BranchMode::Switch => "Switched to",
        BranchMode::Auto if branch_exists => "Switched to",
        BranchMode::Auto => "Created and switched to",
    }
}

pub(super) fn branch_exists(repo: &impl GitRepository, branch_name: &str) -> Result<bool, String> {
    let output = repo
        .output(&["rev-parse", "--verify", "--quiet", branch_name])
        .map_err(|e| e.to_string())?;
    Ok(output.status.success())
}

fn rev_parse_oid(repo: &impl GitRepository, rev: &str) -> Result<Option<String>, String> {
    let spec = format!("{rev}^{{commit}}");
    let output = repo
        .output(&["rev-parse", "--verify", "--quiet", spec.as_str()])
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&output.stdout).trim().to_owned()))
}

fn reject_stale_or_divergent_branch(
    repo: &impl GitRepository,
    branch_name: &str,
    exists: bool,
) -> Result<(), String> {
    if !exists {
        return Ok(());
    }

    if repo.current_branch().map_err(|e| e.to_string())?.as_deref() == Some(branch_name) {
        return Ok(());
    }

    let current_head = rev_parse_oid(repo, "HEAD")?
        .ok_or_else(|| "cannot resolve current HEAD for activation preflight".to_owned())?;
    let branch_head = rev_parse_oid(repo, branch_name)?
        .ok_or_else(|| format!("cannot resolve existing branch '{branch_name}'"))?;

    if current_head != branch_head {
        return Err(format!(
            "branch '{branch_name}' exists but does not point at the current HEAD; refuse to activate onto a stale/divergent branch"
        ));
    }

    Ok(())
}

pub(super) fn preflight_branch_operation(
    repo: &impl GitRepository,
    branch_name: &str,
    mode: BranchMode,
    require_alignment: bool,
) -> Result<bool, String> {
    let exists = branch_exists(repo, branch_name)?;
    if require_alignment {
        reject_stale_or_divergent_branch(repo, branch_name, exists)?;
    }
    match mode {
        BranchMode::Create if exists => Err(format!("branch '{branch_name}' already exists")),
        BranchMode::Switch if !exists => Err(format!("branch '{branch_name}' does not exist")),
        _ => Ok(exists),
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Output;
    use std::sync::Mutex;

    use domain::TrackId;
    use infrastructure::git_cli::{GitError, GitRepository};
    use rstest::rstest;

    use super::super::{BranchMode, resolve_project_root};
    use super::load_track_branch_record;
    use super::{
        activation_branch_create_base, activation_create_requires_main_branch,
        activation_git_commands, activation_rejects_invalid_source_branch,
        activation_requires_clean_worktree, activation_resume_allowed,
        activation_resume_marker_path, allow_materialized_activation,
        allowed_activation_dirty_paths, clear_activation_resume_marker, ensure_clean_worktree,
        persist_activation_commit, preflight_branch_operation,
        should_persist_activation_side_effects, uses_legacy_branch_mode,
        write_activation_resume_marker,
    };
    use std::path::Path;

    struct StubRepo {
        current_branch: Option<String>,
        outputs: HashMap<Vec<String>, Output>,
    }

    impl GitRepository for StubRepo {
        fn root(&self) -> &Path {
            Path::new(".")
        }

        fn status(&self, _args: &[&str]) -> Result<i32, GitError> {
            Ok(0)
        }

        fn output(&self, args: &[&str]) -> Result<Output, GitError> {
            self.outputs
                .get(&args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>())
                .cloned()
                .ok_or_else(|| GitError::CommandFailed {
                    command: args.join(" "),
                    code: -1,
                    stderr: format!("unexpected git args: {}", args.join(" ")),
                })
        }

        fn current_branch(&self) -> Result<Option<String>, GitError> {
            Ok(self.current_branch.clone())
        }
    }

    impl domain::WorktreeReader for StubRepo {
        fn porcelain_status(&self) -> Result<String, domain::WorktreeError> {
            let key = vec!["status".to_owned(), "--porcelain".to_owned()];
            match self.outputs.get(&key) {
                Some(output) => Ok(String::from_utf8_lossy(&output.stdout).into_owned()),
                None => Ok(String::new()),
            }
        }
    }

    struct RecordingRepo {
        current_branch: Option<String>,
        outputs: HashMap<Vec<String>, Output>,
        status_calls: Mutex<Vec<Vec<String>>>,
    }

    impl GitRepository for RecordingRepo {
        fn root(&self) -> &Path {
            Path::new(".")
        }

        fn status(&self, args: &[&str]) -> Result<i32, GitError> {
            self.status_calls
                .lock()
                .unwrap()
                .push(args.iter().map(|arg| (*arg).to_owned()).collect());
            Ok(0)
        }

        fn output(&self, args: &[&str]) -> Result<Output, GitError> {
            self.outputs
                .get(&args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>())
                .cloned()
                .ok_or_else(|| GitError::CommandFailed {
                    command: args.join(" "),
                    code: -1,
                    stderr: format!("unexpected git args: {}", args.join(" ")),
                })
        }

        fn current_branch(&self) -> Result<Option<String>, GitError> {
            Ok(self.current_branch.clone())
        }
    }

    impl domain::WorktreeReader for RecordingRepo {
        fn porcelain_status(&self) -> Result<String, domain::WorktreeError> {
            let key = vec!["status".to_owned(), "--porcelain".to_owned()];
            match self.outputs.get(&key) {
                Some(output) => Ok(String::from_utf8_lossy(&output.stdout).into_owned()),
                None => Ok(String::new()),
            }
        }
    }

    fn write_track_metadata(
        root: &Path,
        schema_version: u32,
        branch: Option<&str>,
    ) -> std::path::PathBuf {
        let track_dir = root.join("track/items/demo");
        fs::create_dir_all(&track_dir).unwrap();
        let branch_json = match branch {
            Some(branch) => format!(r#""{branch}""#),
            None => "null".to_owned(),
        };
        fs::write(
            track_dir.join("metadata.json"),
            format!(
                r#"{{
  "schema_version": {schema_version},
  "id": "demo",
  "branch": {branch_json},
  "title": "Demo",
  "status": "planned",
  "created_at": "2026-03-14T00:00:00Z",
  "updated_at": "2026-03-14T00:00:00Z",
  "tasks": [
    {{
      "id": "T1",
      "description": "Implement activation guard",
      "status": "todo"
    }}
  ],
  "plan": {{
    "summary": [],
    "sections": [
      {{
        "id": "S1",
        "title": "Build",
        "description": [],
        "task_ids": ["T1"]
      }}
    ]
  }}
}}
"#
            ),
        )
        .unwrap();
        track_dir
    }

    fn success_output(stdout: &str) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    fn exit_output(code: i32, stdout: &str) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    #[test]
    fn resolve_project_root_accepts_standard_track_items_layout() {
        assert_eq!(
            resolve_project_root(Path::new("repo/track/items")),
            Ok(std::path::PathBuf::from("repo"))
        );
    }

    #[test]
    fn resolve_project_root_rejects_non_standard_layout() {
        assert!(matches!(
            resolve_project_root(Path::new("repo/custom-items")),
            Err(err) if err.contains("track/items")
        ));
    }

    #[test]
    fn reject_branchless_guard_blocks_planning_only_tracks_via_fs_store() {
        let dir = tempfile::tempdir().unwrap();
        write_track_metadata(dir.path(), 3, None);

        let items_dir = dir.path().join("track/items");
        let store = infrastructure::track::fs_store::FsTrackStore::new(items_dir);
        let (_, meta) = store.find_with_meta(&TrackId::new("demo").unwrap()).unwrap().unwrap();

        let err = usecase::track_resolution::reject_branchless_guard(
            &store,
            &TrackId::new("demo").unwrap(),
            "in_progress",
            meta.schema_version,
        )
        .unwrap_err();

        assert!(err.to_string().contains("/track:activate demo"));
    }

    #[test]
    fn reject_branchless_guard_allows_materialized_tracks_via_fs_store() {
        let dir = tempfile::tempdir().unwrap();
        write_track_metadata(dir.path(), 3, Some("track/demo"));

        let items_dir = dir.path().join("track/items");
        let store = infrastructure::track::fs_store::FsTrackStore::new(items_dir);
        let (_, meta) = store.find_with_meta(&TrackId::new("demo").unwrap()).unwrap().unwrap();

        let result = usecase::track_resolution::reject_branchless_guard(
            &store,
            &TrackId::new("demo").unwrap(),
            "in_progress",
            meta.schema_version,
        );

        assert!(result.is_ok());
    }

    #[test]
    fn reject_branchless_guard_allows_legacy_v2_branchless_tracks_via_fs_store() {
        let dir = tempfile::tempdir().unwrap();
        write_track_metadata(dir.path(), 2, None);

        let items_dir = dir.path().join("track/items");
        let store = infrastructure::track::fs_store::FsTrackStore::new(items_dir);

        let result = usecase::track_resolution::reject_branchless_guard(
            &store,
            &TrackId::new("demo").unwrap(),
            "in_progress",
            2,
        );

        assert!(result.is_ok());
    }

    #[rstest]
    #[case::create_v2(BranchMode::Create, 2, true)]
    #[case::switch_v2(BranchMode::Switch, 2, true)]
    #[case::auto_v2(BranchMode::Auto, 2, false)]
    #[case::create_v3(BranchMode::Create, 3, false)]
    fn uses_legacy_branch_mode_only_for_non_auto_v2_paths(
        #[case] mode: BranchMode,
        #[case] schema_version: u32,
        #[case] expected: bool,
    ) {
        assert_eq!(uses_legacy_branch_mode(mode, schema_version), expected);
    }

    #[rstest]
    #[case::auto_mode(BranchMode::Auto)]
    #[case::switch_mode(BranchMode::Switch)]
    fn activation_git_commands_fast_forward_existing_branch_after_materialization(
        #[case] mode: BranchMode,
    ) {
        let commands = activation_git_commands(mode, "track/demo", true, true, Some("main"), None);

        assert_eq!(
            commands,
            vec![
                vec![
                    "branch".to_owned(),
                    "-f".to_owned(),
                    "track/demo".to_owned(),
                    "HEAD".to_owned(),
                ],
                vec!["switch".to_owned(), "track/demo".to_owned()],
            ]
        );
    }

    #[test]
    fn activation_git_commands_switch_existing_materialized_branch_switches_only() {
        let commands = activation_git_commands(
            BranchMode::Switch,
            "track/demo",
            true,
            false,
            Some("main"),
            None,
        );

        assert_eq!(commands, vec![vec!["switch".to_owned(), "track/demo".to_owned()]]);
    }

    #[test]
    fn activation_resume_allowed_only_for_planned_materialization_commit_on_non_track_branch() {
        let dir = tempfile::tempdir().unwrap();
        let track_id = TrackId::new("demo").unwrap();
        write_activation_resume_marker(dir.path(), &track_id).unwrap();
        let repo = StubRepo { current_branch: Some("main".to_owned()), outputs: HashMap::new() };

        assert!(
            activation_resume_allowed(
                &repo,
                dir.path(),
                &dir.path().join("track/items"),
                &track_id,
                "track/demo",
                "planned",
                Some("main"),
            )
            .unwrap()
        );
        assert!(
            !activation_resume_allowed(
                &repo,
                dir.path(),
                &dir.path().join("track/items"),
                &track_id,
                "track/demo",
                "in_progress",
                Some("main"),
            )
            .unwrap()
        );
        assert!(
            !activation_resume_allowed(
                &repo,
                dir.path(),
                &dir.path().join("track/items"),
                &track_id,
                "track/demo",
                "planned",
                Some("track/demo"),
            )
            .unwrap()
        );
    }

    #[test]
    fn activation_resume_allowed_when_head_has_advanced_past_activation_commit() {
        let dir = tempfile::tempdir().unwrap();
        let track_id = TrackId::new("demo").unwrap();
        write_activation_resume_marker(dir.path(), &track_id).unwrap();
        let repo = StubRepo { current_branch: Some("main".to_owned()), outputs: HashMap::new() };

        assert!(
            activation_resume_allowed(
                &repo,
                dir.path(),
                &dir.path().join("track/items"),
                &track_id,
                "track/demo",
                "planned",
                Some("main"),
            )
            .unwrap()
        );
    }

    #[test]
    fn activation_resume_allowed_rejects_clean_existing_branch_without_resume_marker() {
        let dir = tempfile::tempdir().unwrap();
        let track_id = TrackId::new("demo").unwrap();
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([
                (
                    vec![
                        "log".to_owned(),
                        "-n".to_owned(),
                        "1".to_owned(),
                        "--format=%H".to_owned(),
                        "--grep".to_owned(),
                        "^track: activate demo$".to_owned(),
                    ],
                    success_output("abc1234\n"),
                ),
                (vec!["status".to_owned(), "--porcelain".to_owned()], success_output("")),
            ]),
        };
        assert!(
            !activation_resume_allowed(
                &repo,
                dir.path(),
                &dir.path().join("track/items"),
                &track_id,
                "track/demo",
                "planned",
                Some("main"),
            )
            .unwrap()
        );
    }

    #[test]
    fn activation_resume_allowed_when_activation_artifacts_are_dirty_without_commit() {
        let dir = tempfile::tempdir().unwrap();
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([
                (
                    vec![
                        "log".to_owned(),
                        "-n".to_owned(),
                        "1".to_owned(),
                        "--format=%H".to_owned(),
                        "--grep".to_owned(),
                        "^track: activate demo$".to_owned(),
                    ],
                    success_output(""),
                ),
                (
                    vec!["status".to_owned(), "--porcelain".to_owned()],
                    success_output(" M track/items/demo/metadata.json\n"),
                ),
            ]),
        };

        assert!(
            activation_resume_allowed(
                &repo,
                dir.path(),
                &dir.path().join("track/items"),
                &TrackId::new("demo").unwrap(),
                "track/demo",
                "planned",
                Some("main"),
            )
            .unwrap()
        );
    }

    #[test]
    fn activation_resume_marker_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let track_id = TrackId::new("demo").unwrap();

        assert!(!activation_resume_marker_path(dir.path(), &track_id).exists());
        write_activation_resume_marker(dir.path(), &track_id).unwrap();
        assert!(activation_resume_marker_path(dir.path(), &track_id).is_file());
        clear_activation_resume_marker(dir.path(), &track_id).unwrap();
        assert!(!activation_resume_marker_path(dir.path(), &track_id).exists());
    }

    #[rstest]
    #[case::switch_not_resume(BranchMode::Switch, false, true)]
    #[case::auto_resume(BranchMode::Auto, true, true)]
    #[case::create_not_resume(BranchMode::Create, false, true)]
    #[case::auto_not_resume(BranchMode::Auto, false, false)]
    fn allow_materialized_activation_only_allows_switch_or_auto_resume(
        #[case] mode: BranchMode,
        #[case] resume_allowed: bool,
        #[case] expected: bool,
    ) {
        assert_eq!(allow_materialized_activation(mode, resume_allowed), expected);
    }

    #[rstest]
    #[case::switch_materialized_no_resume(BranchMode::Switch, true, false, false)]
    #[case::auto_materialized_resume(BranchMode::Auto, true, true, true)]
    #[case::auto_not_materialized(BranchMode::Auto, false, false, true)]
    fn activation_side_effects_skip_for_materialized_switch(
        #[case] mode: BranchMode,
        #[case] already_materialized: bool,
        #[case] resume_allowed: bool,
        #[case] expected: bool,
    ) {
        assert_eq!(
            should_persist_activation_side_effects(mode, already_materialized, resume_allowed),
            expected
        );
    }

    #[rstest]
    #[case::auto_not_materialized(BranchMode::Auto, false, false, true)]
    #[case::auto_materialized_resume(BranchMode::Auto, true, true, true)]
    #[case::auto_materialized_no_resume(BranchMode::Auto, true, false, false)]
    #[case::switch_materialized_no_resume(BranchMode::Switch, true, false, false)]
    fn activation_resume_requires_clean_worktree(
        #[case] mode: BranchMode,
        #[case] already_materialized: bool,
        #[case] resume_allowed: bool,
        #[case] expected: bool,
    ) {
        assert_eq!(
            activation_requires_clean_worktree(mode, already_materialized, resume_allowed),
            expected
        );
    }

    #[rstest]
    #[case::auto_head(BranchMode::Auto, false, Some("HEAD"), true)]
    #[case::auto_track_branch(BranchMode::Auto, false, Some("track/other"), true)]
    #[case::auto_materialized_track_branch(BranchMode::Auto, true, Some("track/other"), true)]
    #[case::auto_no_branch(BranchMode::Auto, false, None, true)]
    #[case::auto_main(BranchMode::Auto, false, Some("main"), false)]
    fn activation_auto_requires_non_track_source_branch(
        #[case] mode: BranchMode,
        #[case] already_materialized: bool,
        #[case] current_branch: Option<&str>,
        #[case] expected: bool,
    ) {
        assert_eq!(
            activation_rejects_invalid_source_branch(mode, already_materialized, current_branch),
            expected
        );
    }

    #[rstest]
    #[case::switch_not_materialized_head(BranchMode::Switch, false, Some("HEAD"), true)]
    #[case::switch_not_materialized_track(BranchMode::Switch, false, Some("track/other"), true)]
    #[case::switch_materialized_track(BranchMode::Switch, true, Some("track/other"), false)]
    #[case::switch_materialized_main(BranchMode::Switch, true, Some("main"), false)]
    fn activation_switch_requires_non_track_source_branch_when_materializing(
        #[case] mode: BranchMode,
        #[case] already_materialized: bool,
        #[case] current_branch: Option<&str>,
        #[case] expected: bool,
    ) {
        assert_eq!(
            activation_rejects_invalid_source_branch(mode, already_materialized, current_branch),
            expected
        );
    }

    #[test]
    fn allowed_activation_dirty_paths_only_open_for_auto_resume() {
        let track_id = TrackId::new("demo").unwrap();
        assert_eq!(
            allowed_activation_dirty_paths(
                Path::new("."),
                Path::new("track/items"),
                &track_id,
                BranchMode::Auto,
                true,
                true
            ),
            std::collections::BTreeSet::from([
                "track/items/demo/metadata.json".to_owned(),
                "track/items/demo/plan.md".to_owned(),
                "track/registry.md".to_owned(),
            ])
        );
        assert!(
            allowed_activation_dirty_paths(
                Path::new("."),
                Path::new("track/items"),
                &track_id,
                BranchMode::Auto,
                false,
                false
            )
            .is_empty()
        );
        assert!(
            allowed_activation_dirty_paths(
                Path::new("."),
                Path::new("track/items"),
                &track_id,
                BranchMode::Switch,
                true,
                true
            )
            .is_empty()
        );
    }

    #[test]
    fn allowed_activation_dirty_paths_respects_items_dir() {
        let track_id = TrackId::new("demo").unwrap();
        assert_eq!(
            allowed_activation_dirty_paths(
                Path::new("."),
                Path::new("custom/track/items"),
                &track_id,
                BranchMode::Auto,
                true,
                true
            ),
            std::collections::BTreeSet::from([
                "custom/track/items/demo/metadata.json".to_owned(),
                "custom/track/items/demo/plan.md".to_owned(),
                "track/registry.md".to_owned(),
            ])
        );
    }

    #[test]
    fn ensure_clean_worktree_allows_only_activation_artifacts_for_resume() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([(
                vec!["status".to_owned(), "--porcelain".to_owned()],
                success_output(" M track/items/demo/metadata.json\n M track/registry.md\n"),
            )]),
        };

        let allowed = std::collections::BTreeSet::from([
            "track/items/demo/metadata.json".to_owned(),
            "track/items/demo/plan.md".to_owned(),
            "track/registry.md".to_owned(),
        ]);

        assert!(ensure_clean_worktree(&repo, &allowed).is_ok());
    }

    #[test]
    fn ensure_clean_worktree_rejects_unrelated_dirty_paths_during_resume() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([(
                vec!["status".to_owned(), "--porcelain".to_owned()],
                success_output(" M track/items/demo/metadata.json\n M src/lib.rs\n"),
            )]),
        };

        let allowed = std::collections::BTreeSet::from([
            "track/items/demo/metadata.json".to_owned(),
            "track/items/demo/plan.md".to_owned(),
            "track/registry.md".to_owned(),
        ]);

        let err = ensure_clean_worktree(&repo, &allowed).unwrap_err();
        assert!(err.contains("clean worktree"));
    }

    #[test]
    fn load_track_branch_record_uses_passed_items_dir() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("custom/track/items");
        let track_dir = items_dir.join("demo");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(
            track_dir.join("metadata.json"),
            r#"{"schema_version":3,"id":"demo","branch":"track/demo","title":"Demo","status":"planned","created_at":"2026-03-14T00:00:00Z","updated_at":"2026-03-14T00:00:00Z","tasks":[],"plan":{"summary":[],"sections":[]}}"#,
        )
        .unwrap();

        let record =
            load_track_branch_record(dir.path(), &items_dir, &TrackId::new("demo").unwrap())
                .unwrap();

        assert_eq!(record.display_path, "custom/track/items/demo");
    }

    #[test]
    fn activation_git_commands_noop_when_already_on_target_branch() {
        let commands = activation_git_commands(
            BranchMode::Auto,
            "track/demo",
            true,
            true,
            Some("track/demo"),
            None,
        );

        assert!(commands.is_empty());
    }

    #[test]
    fn activation_git_commands_create_track_branch_from_main() {
        let commands = activation_git_commands(
            BranchMode::Create,
            "track/demo",
            false,
            false,
            Some("feature"),
            Some("main"),
        );

        assert_eq!(
            commands,
            vec![vec![
                "switch".to_owned(),
                "-c".to_owned(),
                "track/demo".to_owned(),
                "main".to_owned(),
            ]]
        );
    }

    #[test]
    fn activation_git_commands_resume_missing_branch_from_activation_commit() {
        let commands = activation_git_commands(
            BranchMode::Auto,
            "track/demo",
            false,
            false,
            Some("main"),
            Some("abc1234"),
        );

        assert_eq!(
            commands,
            vec![vec![
                "switch".to_owned(),
                "-c".to_owned(),
                "track/demo".to_owned(),
                "abc1234".to_owned(),
            ]]
        );
    }

    #[test]
    fn activation_git_commands_realign_existing_branch_from_activation_commit_on_resume() {
        let commands = activation_git_commands(
            BranchMode::Auto,
            "track/demo",
            true,
            false,
            Some("main"),
            Some("abc1234"),
        );

        assert_eq!(
            commands,
            vec![
                vec![
                    "branch".to_owned(),
                    "-f".to_owned(),
                    "track/demo".to_owned(),
                    "abc1234".to_owned(),
                ],
                vec!["switch".to_owned(), "track/demo".to_owned()],
            ]
        );
    }

    #[test]
    fn preflight_branch_operation_rejects_existing_divergent_branch_in_auto_mode() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo".to_owned(),
                    ],
                    success_output("track/demo\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "HEAD^{commit}".to_owned(),
                    ],
                    success_output("aaa\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo^{commit}".to_owned(),
                    ],
                    success_output("bbb\n"),
                ),
            ]),
        };

        let err =
            preflight_branch_operation(&repo, "track/demo", BranchMode::Auto, true).unwrap_err();

        assert!(err.contains("stale/divergent"));
    }

    #[test]
    fn preflight_branch_operation_allows_existing_aligned_branch_in_auto_mode() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo".to_owned(),
                    ],
                    success_output("track/demo\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "HEAD^{commit}".to_owned(),
                    ],
                    success_output("aaa\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo^{commit}".to_owned(),
                    ],
                    success_output("aaa\n"),
                ),
            ]),
        };

        let result = preflight_branch_operation(&repo, "track/demo", BranchMode::Auto, true);

        assert!(result.is_ok());
    }

    #[test]
    fn preflight_branch_operation_allows_switch_to_existing_branch_with_different_head() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([(
                vec![
                    "rev-parse".to_owned(),
                    "--verify".to_owned(),
                    "--quiet".to_owned(),
                    "track/demo".to_owned(),
                ],
                success_output("track/demo\n"),
            )]),
        };

        let result = preflight_branch_operation(&repo, "track/demo", BranchMode::Switch, false);

        assert!(result.is_ok());
    }

    #[test]
    fn preflight_branch_operation_rejects_existing_divergent_branch_in_switch_mode_when_materializing()
     {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo".to_owned(),
                    ],
                    success_output("track/demo\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "HEAD^{commit}".to_owned(),
                    ],
                    success_output("aaa\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo^{commit}".to_owned(),
                    ],
                    success_output("bbb\n"),
                ),
            ]),
        };

        let err =
            preflight_branch_operation(&repo, "track/demo", BranchMode::Switch, true).unwrap_err();

        assert!(err.contains("stale/divergent"));
    }

    #[test]
    fn activation_branch_create_base_uses_activation_commit_for_resume() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([(
                vec![
                    "log".to_owned(),
                    "-n".to_owned(),
                    "1".to_owned(),
                    "--format=%H".to_owned(),
                    "--grep".to_owned(),
                    "^track: activate demo$".to_owned(),
                ],
                success_output("abc1234\n"),
            )]),
        };

        let base = activation_branch_create_base(
            &repo,
            &TrackId::new("demo").unwrap(),
            "track/demo",
            BranchMode::Auto,
            false,
            false,
        )
        .unwrap();

        assert_eq!(base, Some("abc1234".to_owned()));
    }

    #[rstest]
    #[case::create_not_materialized_non_main(BranchMode::Create, false, Some("feature"), true)]
    #[case::create_not_materialized_main(BranchMode::Create, false, Some("main"), false)]
    #[case::create_already_materialized(BranchMode::Create, true, Some("feature"), false)]
    #[case::auto_not_materialized(BranchMode::Auto, false, Some("feature"), false)]
    fn activation_create_requires_main_branch_for_new_materialization(
        #[case] mode: BranchMode,
        #[case] already_materialized: bool,
        #[case] current_branch: Option<&str>,
        #[case] expected: bool,
    ) {
        assert_eq!(
            activation_create_requires_main_branch(mode, already_materialized, current_branch),
            expected
        );
    }

    #[test]
    fn activation_branch_create_base_uses_activation_commit_to_realign_existing_branch() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([
                (
                    vec![
                        "log".to_owned(),
                        "-n".to_owned(),
                        "1".to_owned(),
                        "--format=%H".to_owned(),
                        "--grep".to_owned(),
                        "^track: activate demo$".to_owned(),
                    ],
                    success_output("abc1234\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo^{commit}".to_owned(),
                    ],
                    success_output("old1234\n"),
                ),
                (
                    vec![
                        "merge-base".to_owned(),
                        "--is-ancestor".to_owned(),
                        "old1234".to_owned(),
                        "abc1234".to_owned(),
                    ],
                    success_output(""),
                ),
            ]),
        };

        let base = activation_branch_create_base(
            &repo,
            &TrackId::new("demo").unwrap(),
            "track/demo",
            BranchMode::Auto,
            true,
            false,
        )
        .unwrap();

        assert_eq!(base, Some("abc1234".to_owned()));
    }

    #[test]
    fn activation_branch_create_base_does_not_rewind_branch_ahead_of_activation_commit() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([
                (
                    vec![
                        "log".to_owned(),
                        "-n".to_owned(),
                        "1".to_owned(),
                        "--format=%H".to_owned(),
                        "--grep".to_owned(),
                        "^track: activate demo$".to_owned(),
                    ],
                    success_output("abc1234\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo^{commit}".to_owned(),
                    ],
                    success_output("def5678\n"),
                ),
                (
                    vec![
                        "merge-base".to_owned(),
                        "--is-ancestor".to_owned(),
                        "def5678".to_owned(),
                        "abc1234".to_owned(),
                    ],
                    exit_output(1, ""),
                ),
                (
                    vec![
                        "merge-base".to_owned(),
                        "--is-ancestor".to_owned(),
                        "abc1234".to_owned(),
                        "def5678".to_owned(),
                    ],
                    success_output(""),
                ),
            ]),
        };

        let base = activation_branch_create_base(
            &repo,
            &TrackId::new("demo").unwrap(),
            "track/demo",
            BranchMode::Auto,
            true,
            false,
        )
        .unwrap();

        assert_eq!(base, None);
    }

    #[test]
    fn persist_activation_commit_skips_when_activation_artifacts_are_clean() {
        let repo = RecordingRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([(
                vec![
                    "status".to_owned(),
                    "--porcelain".to_owned(),
                    "--".to_owned(),
                    "track/items/demo/metadata.json".to_owned(),
                ],
                success_output(""),
            )]),
            status_calls: Mutex::new(Vec::new()),
        };

        let created = persist_activation_commit(
            &repo,
            Path::new("."),
            Path::new("track/items"),
            &TrackId::new("demo").unwrap(),
            &[],
        )
        .unwrap();

        assert!(!created);
        assert!(repo.status_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn persist_activation_commit_creates_commit_when_activation_artifacts_are_dirty() {
        let repo = RecordingRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([(
                vec![
                    "status".to_owned(),
                    "--porcelain".to_owned(),
                    "--".to_owned(),
                    "track/items/demo/metadata.json".to_owned(),
                ],
                success_output(" M track/items/demo/metadata.json\n"),
            )]),
            status_calls: Mutex::new(Vec::new()),
        };

        let created = persist_activation_commit(
            &repo,
            Path::new("."),
            Path::new("track/items"),
            &TrackId::new("demo").unwrap(),
            &[],
        )
        .unwrap();

        assert!(created);
        assert_eq!(
            repo.status_calls.lock().unwrap().as_slice(),
            &[
                vec![
                    "add".to_owned(),
                    "--".to_owned(),
                    "track/items/demo/metadata.json".to_owned(),
                ],
                vec![
                    "commit".to_owned(),
                    "-m".to_owned(),
                    "track: activate demo".to_owned(),
                    "--only".to_owned(),
                    "--".to_owned(),
                    "track/items/demo/metadata.json".to_owned(),
                ],
            ]
        );
    }
}
