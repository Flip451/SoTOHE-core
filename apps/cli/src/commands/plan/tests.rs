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
            PlanInput::RunCodexLocal { model, timeout_seconds, briefing_file, prompt } => {
                assert_eq!(model, "gpt-5.4");
                assert_eq!(timeout_seconds, 42);
                assert_eq!(briefing_file, None);
                assert_eq!(prompt, Some("Plan this change.".to_owned()));
            }
        }
        CommandOutcome { stdout: None, stderr: None, exit_code: 7 }
    });

    assert_eq!(exit, ExitCode::from(7));
}

#[test]
fn test_codex_local_briefing_file_dispatches_raw_path() {
    let dir = tempfile::tempdir().unwrap();
    let briefing = dir.path().join("briefing.md");
    std::fs::write(&briefing, "# Task\n").unwrap();
    let args = fake_args(None, Some(briefing.clone()));

    let exit = run_execute_codex_local(&args, |input| {
        match input {
            PlanInput::RunCodexLocal { model, timeout_seconds, briefing_file, prompt } => {
                assert_eq!(model, "gpt-5.4");
                assert_eq!(timeout_seconds, 42);
                // Raw path is forwarded; prompt resolution happens in PlannerInteractor.
                assert_eq!(briefing_file, Some(briefing.clone()));
                assert_eq!(prompt, None);
            }
        }
        CommandOutcome { stdout: None, stderr: None, exit_code: 0 }
    });

    assert_eq!(exit, ExitCode::SUCCESS);
}

#[test]
fn test_codex_local_nonexistent_briefing_file_still_dispatches() {
    // Prompt resolution and file existence checks now live in PlannerInteractor,
    // not in the cli layer. The thin-bin dispatcher forwards raw args unconditionally.
    let args = fake_args(None, Some(PathBuf::from("/nonexistent/briefing.md")));

    let mut called = false;
    let exit = run_execute_codex_local(&args, |input| {
        called = true;
        match input {
            PlanInput::RunCodexLocal { briefing_file, .. } => {
                assert_eq!(briefing_file, Some(PathBuf::from("/nonexistent/briefing.md")));
            }
        }
        CommandOutcome { stdout: None, stderr: None, exit_code: 1 }
    });

    assert!(called, "handler must be called — cli layer no longer validates file existence");
    assert_eq!(exit, ExitCode::from(1));
}

#[test]
fn test_plan_input_from_args_passes_raw_args_through() {
    // plan_input_from_args is now infallible — it converts clap args to PlanInput
    // without any validation. Prompt resolution happens in PlannerInteractor.
    let args = fake_args(None, None);
    let input = plan_input_from_args(&args);

    match input {
        PlanInput::RunCodexLocal { model, timeout_seconds, briefing_file, prompt } => {
            assert_eq!(model, "gpt-5.4");
            assert_eq!(timeout_seconds, 42);
            assert_eq!(briefing_file, None);
            assert_eq!(prompt, None);
        }
    }
}
