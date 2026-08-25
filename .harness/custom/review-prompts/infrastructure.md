# Infrastructure Layer Review: Severity Policy

The reviewer's role is **adapter correctness and I/O boundary review** of
`libs/infrastructure/`. Infrastructure is where ports get their adapters,
and where external I/O actually happens (file system, processes, network,
git, JSON / TOML / YAML codecs). The reviewer must catch issues that compile
cleanly but break trust boundaries at runtime.

## Priority categories

Violations of the role statement above are always reportable. The following priority categories focus the review and guide severity assessment; they are not an exhaustive list of reportable design deviations. The exclusions in **What NOT to report** still apply:

- **external-boundary assumptions**: What external boundaries does the diff touch (OS, process, encoding, concurrency, resource limits, time, and other versions of its own artifacts)? Enumerate operations directly reached from the changed behavior. If a depended-on assumption is in neither the spec nor `knowledge/conventions/environment-assumptions.md`, report it as `未宣言の前提への依存`; treat unresolvable indirect boundaries the same way rather than searching exhaustively.
- **trusted-root violation**: a path-handling code path that resolves user
  / config input via `Path::join` / canonicalize WITHOUT verifying the
  resolved path stays under a `trusted_root: &Path` (path traversal).
  Cite the existing `is_safe_briefing_path` pattern and the fail-closed
  trusted-root policy.
- **symlink not rejected at boundary**: a file load that follows symlinks
  (the default for `std::fs::read_to_string` / `read_dir`) without an
  explicit `symlink_metadata().file_type().is_symlink()` check + reject when
  the convention requires real-file semantics. Cite the existing
  symlink-rejection tests in `scope_config_loader.rs`.
- **panic-able adapter**: `unwrap()` / `expect()` / index access in adapter
  code that runs in production (not test). Adapter errors must map to a
  typed error enum, not panic.
- **port impl missing a domain trait method**: adapter `impl X for Y` whose
  trait surface is incomplete and silently uses a default `unimplemented!()`
  or a no-op, breaking the contract upstream code relies on.
- **serde codec break**: `#[serde(deny_unknown_fields)]` removed / forgotten
  on a versioned-schema DTO, allowing forward-schema rows to silently
  truncate; OR a `schema_version` check that does not fail-closed on
  unrecognized version (compare with `agent_profiles.rs` / `dry_check/config.rs`
  pattern: parse `SchemaVersionEnvelope` first → reject unsupported version
  → only then parse the strict DTO).
- **fingerprint or hash drift**: a hash / fingerprint function over a struct
  that omits a field affecting semantics, producing identical hashes for
  semantically different inputs. Cite `dry_check/config.rs::fingerprint`
  precedent: every semantics-affecting field must be in the canonical
  encoding.
- **layer leak**: code in infrastructure that calls into `apps/cli` /
  `apps/cli-composition` (reverse-direction dependency, violates
  `architecture-rules.json`).
- **non-fail-closed config**: a config loader that returns a default on
  parse error / missing file where the config decides a security-relevant or
  gate-relevant outcome. The existing loader patterns in this crate reject
  fallback-to-default for such configs; cite the loader you are comparing against.

## What NOT to report

- Choice of crate (`serde_json` vs `simd-json`) unless it impacts correctness
- Renaming adapter structs to be "more descriptive" — `architecture-rules.json`
  defines the structural rules
- Test fixture cleanups (the unit-test scaffolding has its own conventions
  that the production review should not touch)
- Doc string wording beyond "is the `# Errors` section present?"
- Suggestions to extract a "common base" between two adapters when the
  current duplication is the natural outcome of differing port contracts
- Performance micro-optimization unless an I/O path lacks a bound (unbounded
  loop / unbounded read) — that IS a correctness concern, report it
