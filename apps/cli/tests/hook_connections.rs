//! Verification of the shipped provider hook connections.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

const CLAUDE_SETTINGS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.claude/settings.json"));
const CODEX_CONFIG: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.codex/config.toml"));
const GROK_SETTINGS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.grok/hooks/sotp.json"));
const GIT_PRE_PUSH: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.githooks/pre-push"));
const GIT_REFERENCE_TRANSACTION: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.githooks/reference-transaction"));

fn json_hook_commands(config: &serde_json::Value, event: &str) -> Vec<String> {
    let groups = config
        .get("hooks")
        .and_then(serde_json::Value::as_object)
        .and_then(|hooks| hooks.get(event))
        .and_then(serde_json::Value::as_array)
        .expect("provider config must define the hook event");

    groups
        .iter()
        .flat_map(|group| {
            group.get("hooks").and_then(serde_json::Value::as_array).into_iter().flatten()
        })
        .map(|hook| {
            hook.get("command")
                .and_then(serde_json::Value::as_str)
                .expect("hook entry must define a command")
                .to_owned()
        })
        .collect()
}

fn toml_hook_commands(config: &toml::Value, event: &str) -> Vec<String> {
    let groups = config
        .get("hooks")
        .and_then(toml::Value::as_table)
        .and_then(|hooks| hooks.get(event))
        .and_then(toml::Value::as_array)
        .expect("provider config must define the hook event");

    groups
        .iter()
        .flat_map(|group| group.get("hooks").and_then(toml::Value::as_array).into_iter().flatten())
        .map(|hook| {
            hook.get("command")
                .and_then(toml::Value::as_str)
                .expect("hook entry must define a command")
                .to_owned()
        })
        .collect()
}

#[cfg(unix)]
mod configured_command_tests {
    use std::ffi::OsString;
    use std::fs;
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};

    use tempfile::TempDir;

    use super::{
        CLAUDE_SETTINGS, CODEX_CONFIG, GIT_PRE_PUSH, GIT_REFERENCE_TRANSACTION, GROK_SETTINGS,
        json_hook_commands, toml_hook_commands,
    };

    const INPUT: &[u8] = br#"{"toolName":"run_terminal_command","toolInput":{"command":"printf 'unchanged'"},"tool_name":"Write","tool_input":{"file_path":"safe.txt","content":"alias"}}"#;

    struct Invocation {
        status: Option<i32>,
        args: Vec<String>,
        stdin: Vec<u8>,
    }

    #[derive(Clone, Copy)]
    enum BinaryResolution {
        Environment,
        RepositoryBinary,
        PathCommand,
    }

    impl BinaryResolution {
        const ALL: [Self; 3] = [Self::Environment, Self::RepositoryBinary, Self::PathCommand];
    }

    fn write_recorder(path: &Path, exit_code: i32) {
        fs::write(
            path,
            format!(
                "#!/bin/sh\nset -eu\nprintf '%s\\n' \"$@\" > \"$HOOK_TRACE_ARGS\"\n/bin/cat > \"$HOOK_TRACE_STDIN\"\nexit {exit_code}\n"
            ),
        )
        .unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions).unwrap();
    }

    fn make_executable(path: &Path) {
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions).unwrap();
    }

    fn project_fixture(repository_root: &Path) -> TempDir {
        let directory = TempDir::new().unwrap();
        let root = directory.path();
        let git_status =
            Command::new("git").args(["init", "--quiet"]).current_dir(root).status().unwrap();
        assert!(git_status.success());

        for (relative_path, source_path) in [
            (".codex/hooks/sotp-hook.sh", repository_root.join("../../.codex/hooks/sotp-hook.sh")),
            (".grok/hooks/sotp-hook.sh", repository_root.join("../../.grok/hooks/sotp-hook.sh")),
        ] {
            let destination = root.join(relative_path);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(source_path, &destination).unwrap();
            make_executable(&destination);
        }

        directory
    }

    fn run_configured_command(
        command_line: &str,
        repository_root: &Path,
        resolution: BinaryResolution,
        input: &[u8],
        exit_code: i32,
    ) -> Invocation {
        let project = project_fixture(repository_root);
        let root = project.path();
        let trace_directory = TempDir::new().unwrap();
        let args_path = trace_directory.path().join("args");
        let stdin_path = trace_directory.path().join("stdin");
        let recorder = trace_directory.path().join("recorder");
        write_recorder(&recorder, exit_code);

        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg(command_line)
            .current_dir(root)
            .env("CLAUDE_PROJECT_DIR", root)
            .env("HOOK_TRACE_ARGS", &args_path)
            .env("HOOK_TRACE_STDIN", &stdin_path)
            .stdin(Stdio::piped());
        command.env_remove("SOTP_CLI_BINARY");

        match resolution {
            BinaryResolution::Environment => {
                command.env("SOTP_CLI_BINARY", &recorder);
            }
            BinaryResolution::RepositoryBinary => {
                let binary_directory = root.join("bin");
                fs::create_dir_all(&binary_directory).unwrap();
                write_recorder(&binary_directory.join("sotp"), exit_code);
            }
            BinaryResolution::PathCommand => {
                let path_directory = trace_directory.path().join("path");
                fs::create_dir_all(&path_directory).unwrap();
                write_recorder(&path_directory.join("sotp"), exit_code);

                let mut path = OsString::from(path_directory);
                path.push(":");
                if let Some(system_path) = std::env::var_os("PATH") {
                    path.push(system_path);
                }
                command.env("PATH", path);
            }
        }

        let mut child = command.spawn().unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        let output = child.wait_with_output().unwrap();

        let args =
            fs::read_to_string(args_path).unwrap_or_default().lines().map(str::to_owned).collect();
        let stdin = fs::read(stdin_path).unwrap_or_default();
        Invocation { status: output.status.code(), args, stdin }
    }

    fn init_repository(root: &Path) {
        let status =
            Command::new("git").args(["init", "--quiet"]).current_dir(root).status().unwrap();
        assert!(status.success());
    }

    fn create_commit(root: &Path) {
        let status = Command::new("git")
            .args([
                "-c",
                "user.name=SoTOHE hook test",
                "-c",
                "user.email=hook-test@example.invalid",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "fixture",
            ])
            .current_dir(root)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[derive(Debug, Clone, Copy)]
    enum GitBinaryResolution {
        Worktree,
        CommonDirectory,
        WorktreeList,
    }

    impl GitBinaryResolution {
        const ALL: [Self; 3] = [Self::Worktree, Self::CommonDirectory, Self::WorktreeList];
    }

    fn run_git_script(
        script_contents: &str,
        arguments: &[&str],
        input: &[u8],
        resolution: GitBinaryResolution,
    ) -> Invocation {
        let directory = TempDir::new().unwrap();
        let root = directory.path().to_path_buf();
        let (working_directory, script, command_path) = match resolution {
            GitBinaryResolution::Worktree => {
                init_repository(&root);
                let binary_directory = root.join("bin");
                fs::create_dir_all(&binary_directory).unwrap();
                write_recorder(&binary_directory.join("sotp"), 0);
                (root.clone(), root.join("hook.sh"), None)
            }
            GitBinaryResolution::CommonDirectory => {
                init_repository(&root);
                create_commit(&root);
                let linked = root.join("linked");
                let status = Command::new("git")
                    .args(["worktree", "add", "--quiet", "--detach"])
                    .arg(&linked)
                    .arg("HEAD")
                    .current_dir(&root)
                    .status()
                    .unwrap();
                assert!(status.success());

                let binary_directory = root.join("bin");
                fs::create_dir_all(&binary_directory).unwrap();
                write_recorder(&binary_directory.join("sotp"), 0);
                (linked.clone(), linked.join("hook.sh"), None)
            }
            GitBinaryResolution::WorktreeList => {
                let main = root.join("main");
                let common = root.join("common.git");
                fs::create_dir_all(&main).unwrap();
                let status = Command::new("git")
                    .args(["init", "--quiet", "--separate-git-dir"])
                    .arg(&common)
                    .arg(&main)
                    .status()
                    .unwrap();
                assert!(status.success());
                create_commit(&main);

                let binary_directory = main.join("bin");
                fs::create_dir_all(&binary_directory).unwrap();
                write_recorder(&binary_directory.join("sotp"), 0);

                let linked = root.join("linked");
                let status = Command::new("git")
                    .args(["worktree", "add", "--quiet", "--detach"])
                    .arg(&linked)
                    .arg("HEAD")
                    .current_dir(&main)
                    .status()
                    .unwrap();
                assert!(status.success());

                let fake_bin = root.join("fake-bin");
                fs::create_dir_all(&fake_bin).unwrap();
                let fake_git = fake_bin.join("git");
                fs::write(
                    &fake_git,
                    format!(
                        "#!/bin/sh\ncase \"$*\" in\n  *--show-toplevel*) printf '%s\\n' '{}' ;;\n  *--git-common-dir*) printf '%s\\n' '{}' ;;\n  'worktree list --porcelain') printf 'worktree %s\\n' '{}' ;;\n  *) exit 1 ;;\nesac\n",
                        linked.display(),
                        root.join("unusable-common/.git").display(),
                        main.display(),
                    ),
                )
                .unwrap();
                make_executable(&fake_git);
                (
                    linked.clone(),
                    linked.join("hook.sh"),
                    Some(format!("{}:/usr/bin:/bin", fake_bin.display())),
                )
            }
        };

        fs::write(&script, script_contents).unwrap();
        make_executable(&script);

        let args_path = directory.path().join("args");
        let stdin_path = directory.path().join("stdin");
        let mut command = Command::new("/bin/sh");
        command
            .arg(&script)
            .args(arguments)
            .current_dir(working_directory)
            .env("HOOK_TRACE_ARGS", &args_path)
            .env("HOOK_TRACE_STDIN", &stdin_path)
            .stdin(Stdio::piped());
        if let Some(path) = command_path {
            command.env("PATH", path);
        }

        let mut child = command.spawn().unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        let output = child.wait_with_output().unwrap();

        let args =
            fs::read_to_string(args_path).unwrap_or_default().lines().map(str::to_owned).collect();
        let stdin = fs::read(stdin_path).unwrap_or_default();
        Invocation { status: output.status.code(), args, stdin }
    }

    fn assert_agent_commands(
        commands: Vec<String>,
        host: &str,
        hook_ids: &[&str],
        advisory: bool,
        repository_root: &Path,
    ) {
        assert_eq!(commands.len(), hook_ids.len());
        for (command, hook_id) in commands.iter().zip(hook_ids) {
            for resolution in BinaryResolution::ALL {
                let success =
                    run_configured_command(command, repository_root, resolution, INPUT, 0);
                assert_eq!(success.status, Some(0));
                assert_eq!(success.args, vec!["hook", "dispatch", "--host", host, *hook_id]);
                assert_eq!(success.stdin, INPUT);

                let failure =
                    run_configured_command(command, repository_root, resolution, INPUT, 2);
                assert_eq!(failure.args, success.args);
                assert_eq!(failure.stdin, success.stdin);
                assert_eq!(failure.status, Some(if advisory { 0 } else { 2 }));
            }
        }
    }

    #[test]
    fn test_provider_commands_execute_with_fixed_hosts_and_advisory_policy() {
        let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        let claude: serde_json::Value = serde_json::from_str(CLAUDE_SETTINGS).unwrap();
        assert_agent_commands(
            json_hook_commands(&claude, "UserPromptSubmit"),
            "claude",
            &["skill-compliance"],
            true,
            &repository_root,
        );
        assert_agent_commands(
            json_hook_commands(&claude, "PreToolUse"),
            "claude",
            &["hooks-path-setup", "block-direct-git-ops", "block-test-file-deletion"],
            false,
            &repository_root,
        );

        let codex: toml::Value = toml::from_str(CODEX_CONFIG).unwrap();
        assert_agent_commands(
            toml_hook_commands(&codex, "UserPromptSubmit"),
            "codex",
            &["skill-compliance"],
            true,
            &repository_root,
        );
        assert_agent_commands(
            toml_hook_commands(&codex, "PreToolUse"),
            "codex",
            &["hooks-path-setup", "block-direct-git-ops", "block-test-file-deletion"],
            false,
            &repository_root,
        );

        let grok: serde_json::Value = serde_json::from_str(GROK_SETTINGS).unwrap();
        assert_agent_commands(
            json_hook_commands(&grok, "UserPromptSubmit"),
            "grok",
            &["skill-compliance"],
            true,
            &repository_root,
        );
        assert_agent_commands(
            json_hook_commands(&grok, "PreToolUse"),
            "grok",
            &["hooks-path-setup", "block-direct-git-ops", "block-test-file-deletion"],
            false,
            &repository_root,
        );
    }

    #[test]
    fn test_git_hook_scripts_forward_arguments_without_a_host() {
        for resolution in GitBinaryResolution::ALL {
            let pre_push = run_git_script(
                GIT_PRE_PUSH,
                &["origin", "https://example.com"],
                b"git hook input\n",
                resolution,
            );
            assert_eq!(pre_push.status, Some(0), "resolution: {resolution:?}");
            assert_eq!(
                pre_push.args,
                vec!["hook", "dispatch", "git-pre-push", "origin", "https://example.com"]
            );
            assert_eq!(pre_push.stdin, b"git hook input\n");

            let reference_transaction = run_git_script(
                GIT_REFERENCE_TRANSACTION,
                &["committed"],
                b"git hook input\n",
                resolution,
            );
            assert_eq!(reference_transaction.status, Some(0));
            assert_eq!(
                reference_transaction.args,
                vec!["hook", "dispatch", "git-ref-update", "committed"]
            );
            assert_eq!(reference_transaction.stdin, b"git hook input\n");
        }
    }
}

#[cfg(unix)]
mod wrapper_tests {
    use std::fs;
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};

    use tempfile::TempDir;

    const INPUT: &[u8] = br#"{"toolName":"run_terminal_command","toolInput":{"command":"printf 'unchanged'"},"tool_name":"Write","tool_input":{"file_path":"safe.txt","content":"alias"}}"#;

    #[derive(Clone, Copy)]
    enum BinaryResolution {
        Environment,
        RepositoryBinary,
        PathCommand,
    }

    fn write_recorder(path: &Path) {
        fs::write(
            path,
            "#!/bin/sh\nset -eu\nprintf '%s\\n' \"$@\" > \"$HOOK_TRACE_ARGS\"\n/bin/cat > \"$HOOK_TRACE_STDIN\"\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions).unwrap();
    }

    fn run_wrapper(
        wrapper: &Path,
        host: &str,
        resolution: BinaryResolution,
    ) -> (Vec<String>, Vec<u8>) {
        let directory = TempDir::new().unwrap();
        let args_path = directory.path().join("args");
        let stdin_path = directory.path().join("stdin");
        let recorder = directory.path().join("recorder");
        write_recorder(&recorder);

        let mut command = Command::new("/bin/sh");
        command
            .arg(wrapper)
            .arg("block-direct-git-ops")
            .current_dir(directory.path())
            .env("HOOK_TRACE_ARGS", &args_path)
            .env("HOOK_TRACE_STDIN", &stdin_path)
            .env_remove("SOTP_CLI_BINARY")
            .stdin(Stdio::piped());

        match resolution {
            BinaryResolution::Environment => {
                command.env("SOTP_CLI_BINARY", &recorder);
            }
            BinaryResolution::RepositoryBinary => {
                let bin_dir = directory.path().join("bin");
                fs::create_dir_all(&bin_dir).unwrap();
                write_recorder(&bin_dir.join("sotp"));
            }
            BinaryResolution::PathCommand => {
                let path_dir = directory.path().join("path");
                fs::create_dir_all(&path_dir).unwrap();
                write_recorder(&path_dir.join("sotp"));
                command.env("PATH", path_dir);
            }
        }

        let mut child = command.spawn().unwrap();
        child.stdin.take().unwrap().write_all(INPUT).unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0), "wrapper stderr: {:?}", output.stderr);

        let args = String::from_utf8(fs::read(args_path).unwrap()).unwrap();
        let stdin = fs::read(stdin_path).unwrap();
        let args: Vec<String> = args.lines().map(str::to_owned).collect();
        assert_eq!(host, args.get(3).map(String::as_str).unwrap_or_default());
        (args, stdin)
    }

    #[test]
    fn test_agent_wrappers_add_fixed_host_and_preserve_stdin_across_resolution_paths() {
        let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let wrappers = [
            (repository_root.join("../../.codex/hooks/sotp-hook.sh"), "codex"),
            (repository_root.join("../../.grok/hooks/sotp-hook.sh"), "grok"),
        ];
        let resolutions = [
            BinaryResolution::Environment,
            BinaryResolution::RepositoryBinary,
            BinaryResolution::PathCommand,
        ];

        for (wrapper, host) in wrappers {
            for resolution in resolutions {
                let (args, stdin) = run_wrapper(&wrapper, host, resolution);
                assert_eq!(args, ["hook", "dispatch", "--host", host, "block-direct-git-ops"]);
                assert_eq!(stdin, INPUT);
            }
        }
    }
}
