//! TestFileDeletionGuardHandler blocks Bash and Write tool calls that delete
//! or truncate test files.
//!
//! Shell syntax is interpreted only through `SimpleCommand` values produced by
//! the injected `ShellParser`.

use std::sync::Arc;

use domain::guard::{ShellParser, SimpleCommand};
use domain::hook::{HookContext, HookError, HookInput, HookVerdict};

use super::HookHandler;

const SHELL_LAUNCHERS: &[&str] =
    &["env", "command", "time", "exec", "nice", "nohup", "timeout", "stdbuf", "sudo", "doas"];
const LAUNCHER_POSITIONAL_ARGS: &[(&str, usize)] = &[("timeout", 1)];
const LAUNCHER_OPTIONS_WITH_ARG: &[(&str, &str)] = &[
    ("env", "-u"),
    ("env", "--unset"),
    ("env", "-C"),
    ("env", "--chdir"),
    ("env", "-S"),
    ("env", "--split-string"),
    ("exec", "-a"),
    ("nice", "-n"),
    ("nice", "--adjustment"),
    ("timeout", "-k"),
    ("timeout", "--kill-after"),
    ("timeout", "-s"),
    ("timeout", "--signal"),
    ("stdbuf", "-i"),
    ("stdbuf", "--input"),
    ("stdbuf", "-o"),
    ("stdbuf", "--output"),
    ("stdbuf", "-e"),
    ("stdbuf", "--error"),
    ("sudo", "-u"),
    ("sudo", "--user"),
    ("sudo", "-g"),
    ("sudo", "--group"),
    ("sudo", "-C"),
    ("sudo", "--close-from"),
    ("sudo", "-p"),
    ("sudo", "--prompt"),
    ("sudo", "-D"),
    ("sudo", "--chdir"),
    ("sudo", "-r"),
    ("sudo", "--role"),
    ("sudo", "-t"),
    ("sudo", "--type"),
    ("sudo", "-h"),
    ("sudo", "--host"),
    ("doas", "-u"),
];
const REENTRY_SHELLS: &[&str] = &["bash", "sh", "zsh", "dash", "ksh", "ash"];
const MAX_REENTRY_DEPTH: u8 = 3;

struct EnvSplitPayload<'a> {
    command: &'a str,
    remaining: &'a [String],
}

#[derive(Debug, Clone, Copy)]
enum EnvSplitPayloadError {
    MissingPayload,
}

enum EnvSplitArg<'a> {
    Inline(&'a str),
    NextToken,
}

enum EnvTokenStep<'a> {
    Payload { command: &'a str, remaining_start: usize },
    ReenterAt(usize),
    Advance(usize),
}

/// Hook handler for `block-test-file-deletion`.
///
/// Blocks Bash commands that delete or overwrite test files through parsed
/// `rm` argv or output redirect metadata, and Write tool calls that truncate
/// test files with empty content.
pub struct TestFileDeletionGuardHandler {
    pub parser: Arc<dyn ShellParser>,
}

impl HookHandler for TestFileDeletionGuardHandler {
    fn handle(&self, _ctx: &HookContext, input: &HookInput) -> Result<HookVerdict, HookError> {
        match input.tool_name.as_str() {
            "Write" => return Ok(check_write_input(input)),
            "Bash" => {}
            _ => return Ok(HookVerdict::allow()),
        }

        let command =
            input.command.as_deref().ok_or_else(|| HookError::Input("missing command".into()))?;

        let commands = match self.parser.split_shell(command) {
            Ok(cmds) => cmds,
            Err(err) => {
                return Ok(HookVerdict::block(format!(
                    "blocked: unable to parse shell command ({err})"
                )));
            }
        };

        Ok(check_commands_for_test_deletion(self.parser.as_ref(), &commands, 0)
            .unwrap_or_else(HookVerdict::allow))
    }
}

fn check_write_input(input: &HookInput) -> HookVerdict {
    let Some(file_path) = input.file_path.as_ref().and_then(|p| p.to_str()) else {
        return HookVerdict::block("blocked: Write tool missing file_path (fail-closed)");
    };

    let has_content = input.content.as_deref().is_some_and(|s| !s.is_empty());
    if is_test_file(file_path) && !has_content {
        return HookVerdict::block(format!(
            "blocked: cannot overwrite test file '{file_path}' with empty or missing content"
        ));
    }

    HookVerdict::allow()
}

fn check_commands_for_test_deletion(
    parser: &dyn ShellParser,
    commands: &[SimpleCommand],
    depth: u8,
) -> Option<HookVerdict> {
    for cmd in commands {
        if let Some(verdict) = check_env_split_string(parser, cmd, depth) {
            return Some(verdict);
        }

        if let Some(arg) = test_file_rm_arg(cmd) {
            return Some(HookVerdict::block(format!("blocked: cannot delete test file '{arg}'")));
        }

        if let Some(target) = test_file_output_redirect_target(cmd) {
            return Some(HookVerdict::block(format!(
                "blocked: cannot redirect output to test file '{target}'"
            )));
        }

        if let Some(inner) = extract_shell_reentry_arg(cmd) {
            if depth > MAX_REENTRY_DEPTH {
                return Some(HookVerdict::block(
                    "blocked: shell re-entry depth limit reached".to_string(),
                ));
            }

            match parser.split_shell(&inner) {
                Ok(inner_cmds) => {
                    if let Some(verdict) =
                        check_commands_for_test_deletion(parser, &inner_cmds, depth + 1)
                    {
                        return Some(verdict);
                    }
                }
                Err(err) => {
                    return Some(HookVerdict::block(format!(
                        "blocked: unable to parse shell re-entry command ({err})"
                    )));
                }
            }
        }
    }

    None
}

fn check_env_split_string(
    parser: &dyn ShellParser,
    cmd: &SimpleCommand,
    depth: u8,
) -> Option<HookVerdict> {
    let payload = env_split_string_payload_after_launchers(&cmd.argv)?;

    if depth > MAX_REENTRY_DEPTH {
        return Some(HookVerdict::block(
            "blocked: env split-string depth limit reached".to_string(),
        ));
    }

    let payload = match payload {
        Ok(payload) => payload,
        Err(err) => {
            return Some(HookVerdict::block(format!(
                "blocked: unable to parse env split-string command ({})",
                env_split_error_message(err)
            )));
        }
    };

    let mut child_commands = match parser.split_shell(payload.command) {
        Ok(commands) => commands,
        Err(err) => {
            return Some(HookVerdict::block(format!(
                "blocked: unable to parse env split-string command ({err})"
            )));
        }
    };

    apply_env_split_context(&mut child_commands, payload.remaining);

    check_commands_for_test_deletion(parser, &child_commands, depth + 1)
}

fn apply_env_split_context(commands: &mut Vec<SimpleCommand>, remaining: &[String]) {
    match commands.as_mut_slice() {
        [] if !remaining.is_empty() => {
            commands.push(SimpleCommand {
                argv: std::iter::once("env".to_string()).chain(remaining.iter().cloned()).collect(),
                redirect_texts: Vec::new(),
                output_redirect_texts: Vec::new(),
                has_output_redirect: false,
            });
        }
        [child_command] => {
            child_command.argv.insert(0, "env".to_string());
            child_command.argv.extend(remaining.iter().cloned());
        }
        _ => {}
    }
}

fn env_split_error_message(err: EnvSplitPayloadError) -> &'static str {
    match err {
        EnvSplitPayloadError::MissingPayload => "missing split-string payload",
    }
}

fn test_file_output_redirect_target(cmd: &SimpleCommand) -> Option<&str> {
    cmd.output_redirect_texts.iter().find(|target| is_test_file(target)).map(String::as_str)
}

fn test_file_rm_arg(cmd: &SimpleCommand) -> Option<&str> {
    let mut tokens = cmd.argv.iter().map(String::as_str).peekable();

    while let Some(token) = tokens.next() {
        if is_assignment(token) {
            continue;
        }
        if is_shell_launcher(token) {
            let launcher = command_name(token);
            skip_launcher_args(launcher, &mut tokens);
            continue;
        }
        if !is_rm_token(token) {
            return None;
        }

        return test_file_arg_after_rm(tokens);
    }

    None
}

fn skip_launcher_args<'a, I>(launcher: &str, tokens: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = &'a str>,
{
    let mut pending_value_skip = false;
    let mut positionals_to_skip = launcher_positional_args(launcher);

    while let Some(token) = tokens.peek().copied() {
        if pending_value_skip {
            let _ = tokens.next();
            pending_value_skip = false;
            continue;
        }
        if is_assignment(token) {
            let _ = tokens.next();
            continue;
        }
        if positionals_to_skip > 0 && !is_launcher_option(token) {
            let _ = tokens.next();
            positionals_to_skip -= 1;
            continue;
        }
        if is_launcher_option(token) {
            let _ = tokens.next();
            pending_value_skip = launcher_option_consumes_next(launcher, token);
            continue;
        }
        break;
    }
}

fn test_file_arg_after_rm<'a, I>(tokens: I) -> Option<&'a str>
where
    I: Iterator<Item = &'a str>,
{
    let mut end_of_options = false;

    for arg in tokens {
        if arg == "--" && !end_of_options {
            end_of_options = true;
            continue;
        }
        if !end_of_options && arg.starts_with('-') {
            continue;
        }
        if is_test_file(arg) {
            return Some(arg);
        }
    }

    None
}

fn launcher_positional_args(launcher: &str) -> usize {
    LAUNCHER_POSITIONAL_ARGS
        .iter()
        .find(|(name, _)| *name == launcher)
        .map(|(_, count)| *count)
        .unwrap_or(0)
}

fn launcher_option_consumes_next(launcher: &str, option: &str) -> bool {
    !option.contains('=')
        && LAUNCHER_OPTIONS_WITH_ARG.iter().any(|(name, flag)| *name == launcher && *flag == option)
}

fn is_launcher_option(token: &str) -> bool {
    token.starts_with('-') && token != "-"
}

fn extract_shell_reentry_arg(cmd: &SimpleCommand) -> Option<String> {
    for (i, token) in cmd.argv.iter().enumerate() {
        let name = command_name(token).to_ascii_lowercase();
        if !REENTRY_SHELLS.contains(&name.as_str()) {
            continue;
        }

        let rest = cmd.argv.get(i + 1..)?;
        for (j, arg) in rest.iter().enumerate() {
            if arg == "-c" {
                return rest.get(j + 1).cloned();
            }
            if let Some(flags) = arg.strip_prefix('-') {
                if !flags.starts_with('-') && flags.len() > 1 && flags.contains('c') {
                    return rest.get(j + 1).cloned();
                }
            }
        }
    }

    None
}

fn is_shell_launcher(token: &str) -> bool {
    let name = command_name(token);
    SHELL_LAUNCHERS.contains(&name)
}

fn is_rm_token(token: &str) -> bool {
    let name = command_name(token);
    name == "rm"
}

fn command_name(token: &str) -> &str {
    token.rsplit('/').next().unwrap_or(token)
}

fn is_assignment(token: &str) -> bool {
    token.contains('=') && !token.starts_with('=')
}

fn env_split_string_payload_after_launchers(
    argv: &[String],
) -> Option<Result<EnvSplitPayload<'_>, EnvSplitPayloadError>> {
    env_split_string_payload_in_argv(argv, skip_assignments(argv, 0))
}

fn env_split_string_payload_in_argv(
    argv: &[String],
    start: usize,
) -> Option<Result<EnvSplitPayload<'_>, EnvSplitPayloadError>> {
    let mut index = start;

    loop {
        let token = argv.get(index)?;
        if is_assignment(token) {
            index = skip_assignments(argv, index);
            continue;
        }

        if !is_shell_launcher(token) {
            return None;
        }

        if command_name(token) == "env" {
            return env_split_string_payload_at_env(argv, index);
        }

        index = skip_launcher_args_at(argv, index);
    }
}

fn env_split_string_payload_at_env(
    argv: &[String],
    env_index: usize,
) -> Option<Result<EnvSplitPayload<'_>, EnvSplitPayloadError>> {
    let mut index = env_index + 1;

    while let Some(step) = env_token_step(argv, index) {
        let step = match step {
            Ok(step) => step,
            Err(err) => return Some(Err(err)),
        };

        match step {
            EnvTokenStep::Payload { command, remaining_start } => {
                return Some(Ok(EnvSplitPayload {
                    command,
                    remaining: tail_from(argv, remaining_start),
                }));
            }
            EnvTokenStep::ReenterAt(next) => return env_split_string_payload_in_argv(argv, next),
            EnvTokenStep::Advance(width) => index += width,
        }
    }

    None
}

fn env_token_step<'a>(
    argv: &'a [String],
    index: usize,
) -> Option<Result<EnvTokenStep<'a>, EnvSplitPayloadError>> {
    let token = argv.get(index)?;

    if let Some(arg) = env_split_arg(token) {
        return Some(env_payload_step(argv, index, arg));
    }

    if is_assignment(token) {
        return Some(Ok(EnvTokenStep::ReenterAt(skip_assignments(argv, index))));
    }

    if is_launcher_option(token) {
        let width = if launcher_option_consumes_next("env", token) { 2 } else { 1 };
        return Some(Ok(EnvTokenStep::Advance(width)));
    }

    Some(Ok(EnvTokenStep::ReenterAt(index)))
}

fn env_payload_step<'a>(
    argv: &'a [String],
    index: usize,
    arg: EnvSplitArg<'a>,
) -> Result<EnvTokenStep<'a>, EnvSplitPayloadError> {
    match arg {
        EnvSplitArg::Inline(command) => {
            Ok(EnvTokenStep::Payload { command, remaining_start: index + 1 })
        }
        EnvSplitArg::NextToken => {
            let Some(command) = argv.get(index + 1) else {
                return Err(EnvSplitPayloadError::MissingPayload);
            };
            Ok(EnvTokenStep::Payload { command, remaining_start: index + 2 })
        }
    }
}

fn env_split_arg(token: &str) -> Option<EnvSplitArg<'_>> {
    if let Some(payload) = token.strip_prefix("--split-string=") {
        return Some(EnvSplitArg::Inline(payload));
    }
    if token == "-S" || token == "--split-string" {
        return Some(EnvSplitArg::NextToken);
    }
    if let Some(payload) = token.strip_prefix("-S").filter(|payload| !payload.is_empty()) {
        return Some(EnvSplitArg::Inline(payload));
    }

    let options = token.strip_prefix('-')?;
    if options.starts_with('-') {
        return None;
    }

    for (idx, opt) in options.char_indices() {
        match opt {
            'S' => {
                let payload = &options[idx + opt.len_utf8()..];
                return Some(if payload.is_empty() {
                    EnvSplitArg::NextToken
                } else {
                    EnvSplitArg::Inline(payload)
                });
            }
            'C' | 'a' | 'u' => return None,
            _ => {}
        }
    }

    None
}

fn skip_assignments(argv: &[String], start: usize) -> usize {
    let mut index = start;
    while argv.get(index).is_some_and(|token| is_assignment(token)) {
        index += 1;
    }
    index
}

fn skip_launcher_args_at(argv: &[String], launcher_index: usize) -> usize {
    let Some(launcher_token) = argv.get(launcher_index) else {
        return argv.len();
    };
    let mut tokens = tail_from(argv, launcher_index + 1).iter().map(String::as_str).peekable();
    skip_launcher_args(command_name(launcher_token), &mut tokens);
    argv.len() - tokens.count()
}

fn tail_from<T>(items: &[T], start: usize) -> &[T] {
    items.get(start..).map_or(&[], |tail| tail)
}

pub(crate) fn is_test_file(path: &str) -> bool {
    use std::path::Component;

    let mut normalized = Vec::new();
    for component in std::path::Path::new(path).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(normalized.last(), Some(Component::Normal(_))) {
                    normalized.pop();
                }
            }
            other => normalized.push(other),
        }
    }

    let clean = normalized
        .iter()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    let filename = clean.rsplit('/').next().unwrap_or(&clean);

    clean == "tests"
        || clean.starts_with("tests/")
        || clean.contains("/tests/")
        || clean.ends_with("/tests")
        || filename.ends_with("_test.rs")
        || (filename.starts_with("test_") && filename.ends_with(".rs"))
        || filename == "tests.rs"
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::hook::test_support::simple_command;
    use domain::guard::ParseError;
    use std::path::PathBuf;

    #[derive(Clone)]
    struct ParserResponse {
        input: &'static str,
        result: Result<Vec<SimpleCommand>, ParseError>,
    }

    struct StaticParser {
        responses: Vec<ParserResponse>,
    }

    impl ShellParser for StaticParser {
        fn split_shell(&self, input: &str) -> Result<Vec<SimpleCommand>, ParseError> {
            self.responses
                .iter()
                .find(|response| response.input == input)
                .map(|response| response.result.clone())
                .unwrap_or_else(|| Ok(Vec::new()))
        }
    }

    fn parser_for(input: &'static str, commands: Vec<SimpleCommand>) -> Arc<dyn ShellParser> {
        parser_with_responses(vec![ParserResponse { input, result: Ok(commands) }])
    }

    fn parser_with_responses(responses: Vec<ParserResponse>) -> Arc<dyn ShellParser> {
        Arc::new(StaticParser { responses })
    }

    fn failing_parser(input: &'static str) -> Arc<dyn ShellParser> {
        parser_with_responses(vec![ParserResponse {
            input,
            result: Err(ParseError::UnmatchedQuote),
        }])
    }

    fn shell_reentry_depth_parser(
        final_payload: &'static str,
        final_commands: Option<Vec<SimpleCommand>>,
    ) -> Arc<dyn ShellParser> {
        let mut responses = vec![
            ParserResponse {
                input: "bash -c 'sh -c inner_1'",
                result: Ok(vec![simple_command(&["bash", "-c", "sh -c inner_1"])]),
            },
            ParserResponse {
                input: "sh -c inner_1",
                result: Ok(vec![simple_command(&["sh", "-c", "dash -c inner_2"])]),
            },
            ParserResponse {
                input: "dash -c inner_2",
                result: Ok(vec![simple_command(&["dash", "-c", "zsh -c inner_3"])]),
            },
            ParserResponse {
                input: "zsh -c inner_3",
                result: Ok(vec![simple_command(&["zsh", "-c", final_payload])]),
            },
        ];

        if let Some(commands) = final_commands {
            responses.push(ParserResponse { input: final_payload, result: Ok(commands) });
        }

        parser_with_responses(responses)
    }

    fn command_with_redirects(
        argv: &[&str],
        redirect_texts: &[&str],
        output_redirect_texts: &[&str],
        has_output_redirect: bool,
    ) -> SimpleCommand {
        SimpleCommand {
            argv: argv.iter().map(|arg| (*arg).to_string()).collect(),
            redirect_texts: redirect_texts.iter().map(|target| (*target).to_string()).collect(),
            output_redirect_texts: output_redirect_texts
                .iter()
                .map(|target| (*target).to_string())
                .collect(),
            has_output_redirect,
        }
    }

    fn bash_input(command: &str) -> HookInput {
        HookInput {
            tool_name: "Bash".to_string(),
            command: Some(command.to_string()),
            file_path: None,
            content: None,
        }
    }

    fn write_input(path: &str, content: Option<&str>) -> HookInput {
        HookInput {
            tool_name: "Write".to_string(),
            command: None,
            file_path: Some(PathBuf::from(path)),
            content: content.map(str::to_string),
        }
    }

    fn handle(parser: Arc<dyn ShellParser>, input: HookInput) -> HookVerdict {
        let handler = TestFileDeletionGuardHandler { parser };
        let ctx = HookContext { project_dir: None };
        handler.handle(&ctx, &input).unwrap()
    }

    #[test]
    fn test_write_test_file_empty_content_blocks() {
        let verdict = handle(parser_for("", Vec::new()), write_input("tests/foo.rs", Some("")));
        assert!(verdict.is_blocked());
    }

    #[test]
    fn test_write_test_file_non_empty_content_allows() {
        let verdict = handle(parser_for("", Vec::new()), write_input("tests/foo.rs", Some("ok")));
        assert!(!verdict.is_blocked());
    }

    #[test]
    fn test_write_non_test_file_empty_content_allows() {
        let verdict = handle(parser_for("", Vec::new()), write_input("src/lib.rs", Some("")));
        assert!(!verdict.is_blocked());
    }

    #[test]
    fn test_bash_rm_test_file_blocks() {
        let parser = parser_for("rm tests/foo.rs", vec![simple_command(&["rm", "tests/foo.rs"])]);
        let verdict = handle(parser, bash_input("rm tests/foo.rs"));
        assert!(verdict.is_blocked());
    }

    #[test]
    fn test_bash_rm_non_test_file_allows() {
        let parser = parser_for("rm src/lib.rs", vec![simple_command(&["rm", "src/lib.rs"])]);
        let verdict = handle(parser, bash_input("rm src/lib.rs"));
        assert!(!verdict.is_blocked());
    }

    #[test]
    fn test_bash_env_i_rm_test_file_blocks() {
        let parser = parser_for(
            "env -i rm tests/foo.rs",
            vec![simple_command(&["env", "-i", "rm", "tests/foo.rs"])],
        );
        let verdict = handle(parser, bash_input("env -i rm tests/foo.rs"));
        assert!(verdict.is_blocked());
    }

    #[test]
    fn test_bash_env_split_string_rm_test_file_blocks() {
        let cases = [
            (
                "env -S 'rm tests/foo_test.rs'",
                &["env", "-S", "rm tests/foo_test.rs"][..],
                "rm tests/foo_test.rs",
                &["rm", "tests/foo_test.rs"][..],
            ),
            (
                "env --split-string='rm tests/foo_test.rs'",
                &["env", "--split-string=rm tests/foo_test.rs"][..],
                "rm tests/foo_test.rs",
                &["rm", "tests/foo_test.rs"][..],
            ),
            (
                "env -Srm tests/foo_test.rs",
                &["env", "-Srm", "tests/foo_test.rs"][..],
                "rm",
                &["rm"][..],
            ),
            (
                "env -iSrm tests/foo_test.rs",
                &["env", "-iSrm", "tests/foo_test.rs"][..],
                "rm",
                &["rm"][..],
            ),
        ];

        for (command, argv, payload, payload_argv) in cases {
            let parser = parser_with_responses(vec![
                ParserResponse { input: command, result: Ok(vec![simple_command(argv)]) },
                ParserResponse { input: payload, result: Ok(vec![simple_command(payload_argv)]) },
            ]);
            let verdict = handle(parser, bash_input(command));

            assert!(verdict.is_blocked(), "command: {command}");
        }
    }

    #[test]
    fn test_bash_env_split_string_shell_reentry_test_file_blocks() {
        let parser = parser_with_responses(vec![
            ParserResponse {
                input: "env -S 'sh -c \"rm tests/foo_test.rs\"'",
                result: Ok(vec![simple_command(&["env", "-S", "sh -c \"rm tests/foo_test.rs\""])]),
            },
            ParserResponse {
                input: "sh -c \"rm tests/foo_test.rs\"",
                result: Ok(vec![simple_command(&["sh", "-c", "rm tests/foo_test.rs"])]),
            },
            ParserResponse {
                input: "rm tests/foo_test.rs",
                result: Ok(vec![simple_command(&["rm", "tests/foo_test.rs"])]),
            },
        ]);
        let verdict = handle(parser, bash_input("env -S 'sh -c \"rm tests/foo_test.rs\"'"));
        assert!(verdict.is_blocked());
    }

    #[test]
    fn test_bash_env_split_string_benign_command_allows() {
        let parser = parser_with_responses(vec![
            ParserResponse {
                input: "env -S 'printf ok'",
                result: Ok(vec![simple_command(&["env", "-S", "printf ok"])]),
            },
            ParserResponse {
                input: "printf ok",
                result: Ok(vec![simple_command(&["printf", "ok"])]),
            },
        ]);
        let verdict = handle(parser, bash_input("env -S 'printf ok'"));
        assert!(!verdict.is_blocked());
    }

    #[test]
    fn test_bash_env_split_string_option_payload_remaining_rm_blocks() {
        let cases = [
            (
                "env -S -u PATH rm tests/foo_test.rs",
                &["env", "-S", "-u", "PATH", "rm", "tests/foo_test.rs"][..],
                "-u",
                &["-u"][..],
            ),
            (
                "env -S -- rm tests/foo_test.rs",
                &["env", "-S", "--", "rm", "tests/foo_test.rs"][..],
                "--",
                &["--"][..],
            ),
            (
                "env -S '' rm tests/foo_test.rs",
                &["env", "-S", "", "rm", "tests/foo_test.rs"][..],
                "",
                &[][..],
            ),
        ];

        for (command, argv, payload, payload_argv) in cases {
            let payload_commands = if payload_argv.is_empty() {
                Vec::new()
            } else {
                vec![simple_command(payload_argv)]
            };
            let parser = parser_with_responses(vec![
                ParserResponse { input: command, result: Ok(vec![simple_command(argv)]) },
                ParserResponse { input: payload, result: Ok(payload_commands) },
            ]);
            let verdict = handle(parser, bash_input(command));

            assert!(verdict.is_blocked(), "command: {command}");
        }
    }

    #[test]
    fn test_bash_env_assignment_before_split_string_allows() {
        let parser = parser_with_responses(vec![
            ParserResponse {
                input: "env FOO=bar -S 'rm tests/foo_test.rs'",
                result: Ok(vec![simple_command(&["env", "FOO=bar", "-S", "rm tests/foo_test.rs"])]),
            },
            ParserResponse {
                input: "rm tests/foo_test.rs",
                result: Ok(vec![simple_command(&["rm", "tests/foo_test.rs"])]),
            },
        ]);
        let verdict = handle(parser, bash_input("env FOO=bar -S 'rm tests/foo_test.rs'"));
        assert!(!verdict.is_blocked());
    }

    #[test]
    fn test_bash_nested_env_after_assignment_split_string_blocks() {
        let parser = parser_with_responses(vec![
            ParserResponse {
                input: "env FOO=bar env -S 'rm tests/foo_test.rs'",
                result: Ok(vec![simple_command(&[
                    "env",
                    "FOO=bar",
                    "env",
                    "-S",
                    "rm tests/foo_test.rs",
                ])]),
            },
            ParserResponse {
                input: "rm tests/foo_test.rs",
                result: Ok(vec![simple_command(&["rm", "tests/foo_test.rs"])]),
            },
        ]);
        let verdict = handle(parser, bash_input("env FOO=bar env -S 'rm tests/foo_test.rs'"));
        assert!(verdict.is_blocked());
    }

    #[test]
    fn test_bash_env_split_string_missing_payload_blocks() {
        let parser = parser_for("env -S", vec![simple_command(&["env", "-S"])]);
        let verdict = handle(parser, bash_input("env -S"));
        assert!(verdict.is_blocked());
    }

    #[test]
    fn test_bash_timeout_rm_test_file_blocks() {
        let parser = parser_for(
            "timeout 5 rm tests/foo.rs",
            vec![simple_command(&["timeout", "5", "rm", "tests/foo.rs"])],
        );
        let verdict = handle(parser, bash_input("timeout 5 rm tests/foo.rs"));
        assert!(verdict.is_blocked());
    }

    #[test]
    fn test_bash_sudo_user_rm_test_file_blocks() {
        let parser = parser_for(
            "sudo -u root rm tests/foo.rs",
            vec![simple_command(&["sudo", "-u", "root", "rm", "tests/foo.rs"])],
        );
        let verdict = handle(parser, bash_input("sudo -u root rm tests/foo.rs"));
        assert!(verdict.is_blocked());
    }

    #[test]
    fn test_bash_echo_rm_argument_test_file_allows() {
        let parser = parser_for(
            "echo rm tests/foo.rs",
            vec![simple_command(&["echo", "rm", "tests/foo.rs"])],
        );
        let verdict = handle(parser, bash_input("echo rm tests/foo.rs"));
        assert!(!verdict.is_blocked());
    }

    #[test]
    fn test_bash_git_rm_subcommand_test_file_allows() {
        let parser =
            parser_for("git rm tests/foo.rs", vec![simple_command(&["git", "rm", "tests/foo.rs"])]);
        let verdict = handle(parser, bash_input("git rm tests/foo.rs"));
        assert!(!verdict.is_blocked());
    }

    #[test]
    fn test_bash_shell_reentry_rm_test_file_blocks() {
        let parser = parser_with_responses(vec![
            ParserResponse {
                input: "bash -c 'rm tests/foo.rs'",
                result: Ok(vec![simple_command(&["bash", "-c", "rm tests/foo.rs"])]),
            },
            ParserResponse {
                input: "rm tests/foo.rs",
                result: Ok(vec![simple_command(&["rm", "tests/foo.rs"])]),
            },
        ]);
        let verdict = handle(parser, bash_input("bash -c 'rm tests/foo.rs'"));
        assert!(verdict.is_blocked());
    }

    #[test]
    fn test_bash_shell_reentry_redirect_to_test_file_blocks() {
        let parser = parser_with_responses(vec![
            ParserResponse {
                input: "bash -c 'echo hi > tests/foo.rs'",
                result: Ok(vec![simple_command(&["bash", "-c", "echo hi > tests/foo.rs"])]),
            },
            ParserResponse {
                input: "echo hi > tests/foo.rs",
                result: Ok(vec![command_with_redirects(
                    &["echo", "hi"],
                    &["tests/foo.rs"],
                    &["tests/foo.rs"],
                    true,
                )]),
            },
        ]);
        let verdict = handle(parser, bash_input("bash -c 'echo hi > tests/foo.rs'"));
        assert!(verdict.is_blocked());
    }

    #[test]
    fn test_bash_shell_reentry_depth_limit_payloads_match_expected_verdict() {
        let cases = vec![
            ("echo hi", None, false),
            ("rm tests/foo.rs", Some(vec![simple_command(&["rm", "tests/foo.rs"])]), true),
            (
                "echo hi > tests/foo.rs",
                Some(vec![command_with_redirects(
                    &["echo", "hi"],
                    &["tests/foo.rs"],
                    &["tests/foo.rs"],
                    true,
                )]),
                true,
            ),
        ];

        for (final_payload, final_commands, should_block) in cases {
            let parser = shell_reentry_depth_parser(final_payload, final_commands);
            let verdict = handle(parser, bash_input("bash -c 'sh -c inner_1'"));

            assert_eq!(verdict.is_blocked(), should_block, "payload: {final_payload}");
        }
    }

    #[test]
    fn test_bash_output_redirect_to_test_files_blocks() {
        let cases = [
            ("echo hi > tests/foo.rs", "tests/foo.rs"),
            ("echo hi > src/user_test.rs", "src/user_test.rs"),
        ];

        for (command, target) in cases {
            let parser = parser_for(
                command,
                vec![command_with_redirects(&["echo", "hi"], &[target], &[target], true)],
            );
            let verdict = handle(parser, bash_input(command));

            assert!(verdict.is_blocked(), "target: {target}");
        }
    }

    #[test]
    fn test_bash_output_redirect_to_tmp_file_allows() {
        let parser = parser_for(
            "echo hi > /tmp/file.txt",
            vec![command_with_redirects(
                &["echo", "hi"],
                &["/tmp/file.txt"],
                &["/tmp/file.txt"],
                true,
            )],
        );
        let verdict = handle(parser, bash_input("echo hi > /tmp/file.txt"));
        assert!(!verdict.is_blocked());
    }

    #[test]
    fn test_bash_input_redirect_from_test_file_allows() {
        let parser = parser_for(
            "cat < tests/foo.rs",
            vec![command_with_redirects(&["cat"], &["tests/foo.rs"], &[], false)],
        );
        let verdict = handle(parser, bash_input("cat < tests/foo.rs"));
        assert!(!verdict.is_blocked());
    }

    #[test]
    fn test_bash_input_test_file_output_tmp_redirect_allows() {
        let parser = parser_for(
            "cat < tests/foo.rs > /tmp/out",
            vec![command_with_redirects(
                &["cat"],
                &["tests/foo.rs", "/tmp/out"],
                &["/tmp/out"],
                true,
            )],
        );
        let verdict = handle(parser, bash_input("cat < tests/foo.rs > /tmp/out"));
        assert!(!verdict.is_blocked());
    }

    #[test]
    fn test_bash_heredoc_body_mentions_test_file_allows() {
        let command_text = "cat <<EOF > /tmp/file.txt\ntests/foo.rs\nEOF";
        let parser = parser_for(
            command_text,
            vec![command_with_redirects(
                &["cat"],
                &["EOF", "/tmp/file.txt", "tests/foo.rs\n"],
                &["/tmp/file.txt"],
                true,
            )],
        );
        let verdict = handle(parser, bash_input(command_text));
        assert!(!verdict.is_blocked());
    }

    #[test]
    fn test_bash_parse_error_blocks() {
        let verdict =
            handle(failing_parser("echo 'unterminated"), bash_input("echo 'unterminated"));
        assert!(verdict.is_blocked());
    }

    #[test]
    fn test_bash_missing_command_returns_error() {
        let handler = TestFileDeletionGuardHandler { parser: parser_for("", Vec::new()) };
        let ctx = HookContext { project_dir: None };
        let input = HookInput {
            tool_name: "Bash".to_string(),
            command: None,
            file_path: None,
            content: None,
        };
        let result = handler.handle(&ctx, &input);
        assert!(matches!(result, Err(HookError::Input(message)) if message == "missing command"));
    }

    #[test]
    fn test_other_tool_allows() {
        let input = HookInput {
            tool_name: "Read".to_string(),
            command: None,
            file_path: Some(PathBuf::from("tests/foo.rs")),
            content: None,
        };
        let verdict = handle(parser_for("", Vec::new()), input);
        assert!(!verdict.is_blocked());
    }

    #[test]
    fn test_is_test_file_tests_directory_matches() {
        assert!(is_test_file("tests/foo.rs"));
        assert!(is_test_file("/work/repo/tests/foo.rs"));
    }

    #[test]
    fn test_is_test_file_rust_test_naming_matches() {
        assert!(is_test_file("src/user_test.rs"));
        assert!(is_test_file("src/test_user.rs"));
        assert!(is_test_file("src/tests.rs"));
    }

    #[test]
    fn test_is_test_file_non_test_path_does_not_match() {
        assert!(!is_test_file("src/lib.rs"));
        assert!(!is_test_file("/tmp/file.txt"));
    }
}
