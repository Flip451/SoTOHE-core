#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::process::ExitCode;

use cli_driver::{CommandOutcome, plan::PlanInput};

use super::{
    PlanCodexLocalArgs,
    codex_local::{plan_input_from_args, run_execute_codex_local},
};

fn fake_args(prompt: Option<String>, briefing_file: Option<PathBuf>) -> PlanCodexLocalArgs {
    PlanCodexLocalArgs { model: "gpt-5.4".to_owned(), timeout_seconds: 42, briefing_file, prompt }
}

#[test]
fn test_codex_local_inline_prompt_dispatches_plan_input() {
    let args = fake_args(Some("Plan this change.".to_owned()), None);

    let exit = run_execute_codex_local(&args, |input| {
        match input {
            PlanInput::RunCodexLocal { model, timeout_seconds, prompt } => {
                assert_eq!(model, "gpt-5.4");
                assert_eq!(timeout_seconds, 42);
                assert_eq!(prompt, "Plan this change.");
            }
        }
        CommandOutcome { stdout: None, stderr: None, exit_code: 7 }
    });

    assert_eq!(exit, ExitCode::from(7));
}

#[test]
fn test_codex_local_briefing_file_dispatches_read_instruction() {
    let dir = tempfile::tempdir().unwrap();
    let briefing = dir.path().join("briefing.md");
    std::fs::write(&briefing, "# Task\n").unwrap();
    let args = fake_args(None, Some(briefing.clone()));

    let exit = run_execute_codex_local(&args, |input| {
        match input {
            PlanInput::RunCodexLocal { model, timeout_seconds, prompt } => {
                assert_eq!(model, "gpt-5.4");
                assert_eq!(timeout_seconds, 42);
                assert_eq!(
                    prompt,
                    format!("Read {} and perform the task described there.", briefing.display())
                );
            }
        }
        CommandOutcome { stdout: None, stderr: None, exit_code: 0 }
    });

    assert_eq!(exit, ExitCode::SUCCESS);
}

#[test]
fn test_codex_local_missing_briefing_file_fails_before_dispatch() {
    let args = fake_args(None, Some(PathBuf::from("/nonexistent/briefing.md")));

    let exit = run_execute_codex_local(&args, |_| {
        panic!("driver must not be called when briefing file validation fails");
    });

    assert_eq!(exit, ExitCode::FAILURE);
}

#[test]
fn test_plan_input_from_args_missing_prompt_returns_typed_error() {
    let args = fake_args(None, None);

    let Err(err) = plan_input_from_args(&args) else {
        panic!("expected missing prompt args to fail");
    };

    assert!(err.to_string().contains("either --briefing-file or --prompt is required"));
}
