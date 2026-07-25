# Catchup Workflow SSoT

> Provider-agnostic workflow SSoT for bringing a contributor to a usable local environment
> and current project context. Provider-specific adapters (for example,
> `.claude/commands/track/catchup.md`) reference this file; their invocation framing, tool
> constraints, and report presentation live in those adapters.

## Mission

Prepare the current repository for a contributor, then summarize its current track and
convention state. This workflow is safe from any branch and does not start implementation work.

## Sequence

### 1. Select and run the environment command

Inspect Git metadata owned by the current directory, without treating an enclosing repository as
the current repository.

- When the current directory has no `.git` metadata, it is a newly exported scaffold. Run
  `cargo make init`; it creates the repository, generates the lockfile, makes the initial
  commit, and runs bootstrap.
- When the current directory already has `.git` metadata, it is an initialized repository. Run
  `cargo make bootstrap`.

Monitor the selected command step by step. If it fails, diagnose the error and give the
contributor a concrete corrective action. After that action is applied, retry the same selected
command. `bootstrap` is idempotent; `init` rolls back Git metadata it created on failure, so a
fresh exported scaffold remains eligible for a corrected retry.

### 2. Set up the track workflow

Run the provider's track setup utility. It verifies the project prerequisites and initializes any
missing track-workflow foundation without beginning implementation work.

### 3. Brief the current project state

1. Read `track/registry.md` and list active and completed tracks.
2. Resolve the current track. If the current branch matches `track/<id>`, use that track.
   Otherwise use the latest materialized active track (non-archived, non-done, with a branch).
   If none exists, use the latest branchless planning-only track.
3. Read that track's `spec.md` and `plan.md` when a track was resolved.
4. Read `knowledge/adr/README.md` for recent pre-track ADRs.
5. Show the most recent ten commits for context.
6. Read `knowledge/conventions/README.md` and list the active conventions.

## Constraints

- Do not run `cargo make bootstrap` for a newly exported scaffold: it has no local repository
  for bootstrap's local Git configuration.
- Do not start implementation work in this workflow.
- Do not use direct Git write commands; follow the repository's guarded workflow commands.

## Report

Report the selected environment command and its outcome, track-workflow setup status, the
project-state briefing, and suggested next actions.
