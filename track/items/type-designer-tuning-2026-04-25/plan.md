<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# type-designer の reconnaissance フェーズ追加 — baseline+type-graph 先行実行で既存型インベントリを設計判断に取り込む

## Summary

Three focused tasks covering the two affected files: (1) the Rust CLI output-path fix, (2) the type-designer Internal pipeline rewrite, and (3) the type-designer Output section trim. Tasks are ordered so the CLI is stable before the agent definition references the new paths.

## Tasks (3/3 resolved)

### S001 — CLI: depth-suffix output path fix

> Change `write_type_graph_dir` output directory naming from `<layer>-graph/` to `<layer>-graph-d<depth>/` so depth=1 and depth=2 outputs coexist without overwriting each other (ADR D2). Flat mode (`--cluster-depth 0`) is unchanged. Add unit tests that assert the new path pattern for depth >= 1 and the unchanged flat-mode path for depth 0. Add `.gitignore` entry for the new cluster-mode directories.

- [x] **T001**: Update `bin/sotp track type-graph` Rust CLI: change cluster-mode output directory from `<layer>-graph/` to `<layer>-graph-d<depth>/`, preserve flat-mode output as `<layer>-graph.md`, add unit tests for new path logic, and add `track/items/**/*-graph-d*/` to `.gitignore` (`b69924f551c37d95b51ef16a2c260e668a74764f`)

### S002 — Agent definition: reconnaissance pipeline

> Rewrite the Internal pipeline section of `.claude/agents/type-designer.md` to reflect the 9-step reconnaissance-first order decided in ADR D1+D2. Read targets change from `<layer>-graph/index.md` to `<layer>-graph-d1/` and `<layer>-graph-d2/`. Update the Mission `Reconnaissance first` paragraph to make explicit that reconnaissance output stays internal and that neither step may be skipped.

- [x] **T002**: Update `.claude/agents/type-designer.md` Internal pipeline section: introduce 9-step reconnaissance procedure (baseline-capture → type-graph depth=1 edges=all → type-graph depth=2 edges=all → Read depth=1 output from `<layer>-graph-d1/` → Read depth=2 output from `<layer>-graph-d2/` → catalogue draft → Write → contract-map → type-signals), and align the Mission `Reconnaissance first` paragraph to state that reconnaissance is internal only and must not be skipped (`a2764f312bbf4a555041279e55b6a3457a888a64`)

### S003 — Agent definition: output section trim

> Remove `Entries written`, `Action rationale`, and `Cross-partition migrations` from the Output section of `.claude/agents/type-designer.md` per ADR D2. The remaining output is `Signal evaluation` (per-layer) and `Open Questions`. This aligns the agent contract with the parent ADR's mandate that orchestrator-facing output is signal evaluation only.

- [x] **T003**: Update `.claude/agents/type-designer.md` Output section: remove `Entries written`, `Action rationale`, and `Cross-partition migrations` sections; keep only `Signal evaluation` (per-layer) and `Open Questions` (`aed93aee8afaa5f21271effd7f1b21728e2c3435`)
