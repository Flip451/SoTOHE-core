# Archive Workflow SSoT

> Provider-agnostic workflow SSoT for the `archive` track workflow. Provider adapters reference
> this file; provider-specific invocation framing lives in those adapters.

## Mission

Archive a track through the CLI, then refresh the rendered views. The workflow delegates archive
behavior to `sotp track archive`; it does not edit metadata, move directories, or stage files.

## Inputs

- **Track ID** — optional. When supplied, pass it to the CLI. When omitted, the CLI resolves the
  current track from the branch.
- **Repository context** — the command must run from a Git worktree containing the active track.

## Sequence

**Step 1: Confirm the track is ready to ship**

Run the matching phase-resolution command for the selected track:

```
bin/sotp track resolve <track-id>
```

or, when the current branch identifies the target track:

```
bin/sotp track resolve
```

Continue only when the command reports `Current phase: Ready to Ship`; otherwise stop and report
the phase and reason.

**Step 2: Archive through the CLI**

Run one of:

```
bin/sotp track archive --track-id <track-id>
```

or, when the current branch identifies the target track:

```
bin/sotp track archive
```

Stop on a non-zero exit status and report the CLI error.

**Step 3: Refresh rendered views**

After a successful archive, run:

```
bin/sotp track views sync
```

Stop on a non-zero exit status and report the error.

## Gates

| Step | Gate | Verdict |
|---|---|---|
| 1 | `sotp track resolve` reports `Current phase: Ready to Ship` | OK / ERROR |
| 2 | `sotp track archive` exits 0 | OK / ERROR |
| 3 | `sotp track views sync` exits 0 | OK / ERROR |

## Outputs

- The track is archived by the CLI.
- Rendered views are refreshed.
- The caller receives the archived track id and both command results.
