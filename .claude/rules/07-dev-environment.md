# Development Environment (Rust)

## Toolchain

The distributed scaffold is host-first. `rust-toolchain.toml` selects the required Rust
toolchain (including rustfmt and clippy); install `cargo-make` before running its tasks.
Docker is optional: change the single `extend` target in `Makefile.toml` from
`Makefile.host.toml` to `Makefile.docker.toml` when an isolated toolchain is needed.

```bash
rustup show
cargo install --locked cargo-make --version 0.37.24
cargo make bootstrap
```

`bootstrap` installs the pinned `cargo-nextest` and `cargo-deny` tools with `--locked`,
installs `bin/sotp` when a transplanted binary is unavailable, configures local hooks, and
runs the repository gate. It does not add a separate tool-version preflight.

## Task Runner: cargo-make

`Makefile.toml` owns the current task table. Use `cargo make --list-all-steps` for discovery;
the workflow-stable commands below work in both the source repository and an exported scaffold.

```bash
cargo make bootstrap    # install pinned auxiliary tools, provision sotp, and run CI
cargo make fmt-check    # verify formatting
cargo make clippy       # lint with warnings denied
cargo make test         # run cargo-nextest
cargo make deny         # run cargo-deny
cargo make ci-rust      # Rust-only inner-loop gate
cargo make ci           # repository-wide gate
cargo make ci-track     # track-aware gate on track/<id>
cargo make --list-all-steps # task catalogue

bin/sotp arch tree
bin/sotp arch tree-full
```

## Workflow commands

Use `bin/sotp` for single workflow operations; use `cargo make` only for aggregate gates.
This keeps the documentation valid when the source repository and exported scaffold select
different Makefile execution environments.

```bash
bin/sotp git add-from-file tmp/track-commit/add-paths.txt
bin/sotp git unstage <paths>
bin/sotp git commit-from-file tmp/track-commit/commit-message.txt --cleanup
bin/sotp git note-from-file tmp/track-commit/note.md --cleanup
bin/sotp git sync
bin/sotp track branch create <track-id>
bin/sotp track branch switch <track-id>
bin/sotp track switch-base
bin/sotp pr push
bin/sotp pr ensure-pr
bin/sotp pr review-cycle
bin/sotp track transition T001 done
bin/sotp track views sync
bin/sotp track resolve
bin/sotp capability exec <capability> --host <provider> --briefing-file tmp/capability-runtime/briefing.md
```

## `bin/sotp` provisioning

`bin/sotp` is gitignored. Template export transplants the running binary when possible;
otherwise `cargo make install-sotp` retrieves the pinned tag declared in
`.harness/config/sotp-version.json`. CI follows the latter path and caches `.cargo-install`
under a key containing that tag.

## Project Bootstrap (Version Research)

Research new version baselines before changing pinned tool versions. Record the result under
`knowledge/research/` and update the corresponding toolchain, bootstrap, and CI pins together.

## Testing and dependency auditing

```bash
cargo make test
cargo make deny
```

Run `cargo make ci` before handing off a change.
