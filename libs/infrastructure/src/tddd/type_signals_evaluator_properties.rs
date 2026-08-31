//! Generated-state style checks for the evaluator's D3–D8 boundaries.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fs;

use domain::tddd::type_signals_doc::{
    BaselineHash, CargoProfileName, CatalogueDeclarationHash, ExpectedRustdocJsonPath,
    ImplementationFingerprint, ResolutionFingerprint, ResolvedCargoTargetDirectory,
    RustdocExecutionIdentity, Sha256Digest, TypeSignalsCacheKey, TypeSignalsDocument,
    TypeSignalsReuseDecision,
};
use domain::{CommitHash, Timestamp};

use super::freshness::{self, decide_reuse_for_recorded_document};
#[cfg(unix)]
use crate::tddd::rustdoc_output_lock::RUSTDOC_OUTPUT_LOCK_TIMEOUT;
use crate::tddd::rustdoc_output_lock::RustdocOutputLock;

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[cfg(unix)]
struct PropertyGenerator {
    state: u64,
}

#[cfg(unix)]
impl PropertyGenerator {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        (self.state >> 32) as u32
    }
}

fn digest(value: &str) -> Sha256Digest {
    Sha256Digest::try_new(value.to_owned()).expect("test digest is valid")
}

fn identity(suffix: &str) -> RustdocExecutionIdentity {
    let target = ResolvedCargoTargetDirectory::try_new(std::path::PathBuf::from(format!(
        "/tmp/sotohe-property-target-{suffix}"
    )))
    .expect("test target is absolute");
    let expected =
        ExpectedRustdocJsonPath::try_new(target.as_path().join("doc/infrastructure.json"), &target)
            .expect("test output is contained");
    RustdocExecutionIdentity::new(
        target,
        domain::tddd::catalogue_v2::CrateName::new("infrastructure")
            .expect("test crate name is valid"),
        vec![],
        CargoProfileName::try_new("dev".to_owned()).expect("test profile is valid"),
        expected,
    )
    .expect("test identity is internally consistent")
}

fn cache_key(changed_component: usize) -> TypeSignalsCacheKey {
    TypeSignalsCacheKey::new(
        CatalogueDeclarationHash::new(digest(if changed_component == 1 { B } else { A })),
        CommitHash::try_new(if changed_component == 2 { "b" } else { "a" }.repeat(40))
            .expect("test commit is valid"),
        BaselineHash::new(digest(if changed_component == 3 { B } else { A })),
        ImplementationFingerprint::new(digest(if changed_component == 4 { B } else { A })),
        ResolutionFingerprint::new(digest(if changed_component == 5 { B } else { A })),
        identity(if changed_component == 6 { "changed" } else { "same" }),
    )
}

#[test]
fn property_evaluator_reuses_only_when_every_cache_key_component_matches() {
    let recorded = TypeSignalsDocument::new(
        Timestamp::new("2026-08-30T00:00:00Z").expect("test timestamp is valid"),
        cache_key(0),
        vec![],
    );
    assert_eq!(
        decide_reuse_for_recorded_document(Some(&recorded), &cache_key(0), true),
        TypeSignalsReuseDecision::SkipEvaluation
    );
    for changed_component in 1..=6 {
        assert_eq!(
            decide_reuse_for_recorded_document(
                Some(&recorded),
                &cache_key(changed_component),
                true,
            ),
            TypeSignalsReuseDecision::ReextractAndEvaluate,
            "component {changed_component} must invalidate reuse"
        );
    }
}

#[test]
fn property_evaluator_rejects_external_path_and_build_script_inputs() {
    super::with_process_environment_lock(|| {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let root = workspace.path();
        fs::create_dir_all(root.join("crate/src")).expect("crate directory");
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crate\"]\nresolver = \"2\"\n",
        )
        .expect("workspace manifest");
        fs::write(
            root.join("crate/Cargo.toml"),
            "[package]\nname = \"property-crate\"\nversion = \"0.1.0\"\nedition = \"2024\"\noutside = { path = \"../../outside-dependency\" }\n",
        )
        .expect("crate manifest");
        fs::write(root.join("crate/src/lib.rs"), "pub struct Fixture;\n").expect("crate source");
        fs::create_dir_all(root.parent().expect("temp parent").join("outside-dependency/src"))
            .expect("outside dependency directory");
        let outside = root.parent().expect("temp parent").join("outside-dependency");
        fs::write(
            outside.join("Cargo.toml"),
            "[package]\nname = \"outside-dependency\"\nversion = \"0.1.0\"\nedition = \"2024\"\nbuild = \"build.rs\"\n",
        )
        .expect("outside manifest");
        fs::write(outside.join("src/lib.rs"), "pub struct Outside;\n").expect("outside source");
        fs::write(outside.join("build.rs"), "fn main() {}\n").expect("outside build script");
        let error = freshness::rustdoc_input_fingerprint(root)
            .expect_err("unidentified Cargo input must fail closed");
        assert!(
            error.to_string().contains("external") || error.to_string().contains("build-script")
        );

        let member = tempfile::tempdir().expect("workspace-member build script");
        let member_root = member.path();
        write_fingerprint_fixture(member_root);
        fs::write(member_root.join("build.rs"), "fn main() {}\n").expect("member build script");
        freshness::rustdoc_input_fingerprint(member_root)
            .expect("workspace-member build.rs is source in the fingerprint walk");
    });
}

fn write_fingerprint_fixture(root: &std::path::Path) {
    fs::create_dir_all(root.join("src")).expect("fixture source directory");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"fingerprint-property\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("fixture manifest");
    fs::write(root.join("Cargo.lock"), "version = 4\n").expect("fixture lockfile");
    fs::write(root.join("src/lib.rs"), "pub struct Fixture;\n").expect("fixture source");
}

#[test]
fn property_evaluator_fingerprints_resolved_tool_contents_and_rejects_untrusted_tools() {
    super::with_process_environment_lock(|| {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        write_fingerprint_fixture(workspace.path());
        let tool = workspace.path().join("target/tools/rustc");
        fs::create_dir_all(tool.parent().expect("tool directory")).expect("tool directory");
        fs::write(&tool, b"compiler-generation-a").expect("tool bytes");

        temp_env::with_vars(
            [("RUSTC", Some(tool.as_os_str())), ("RUSTDOC", Some(tool.as_os_str()))],
            || {
                let first = freshness::rustdoc_input_fingerprint(workspace.path())
                    .expect("trusted tool paths must fingerprint");
                fs::write(&tool, b"compiler-generation-b").expect("changed tool bytes");
                let second = freshness::rustdoc_input_fingerprint(workspace.path())
                    .expect("changed trusted tool must fingerprint");
                assert_ne!(first, second, "tool contents are part of implementation identity");

                let outside = tempfile::tempdir().expect("outside tempdir");
                let outside_tool = outside.path().join("rustc");
                fs::write(&outside_tool, b"untrusted compiler").expect("outside tool bytes");
                let outside_hash = temp_env::with_var(
                    "RUSTC",
                    Some(outside_tool.as_os_str()),
                    || {
                        freshness::rustdoc_input_fingerprint(workspace.path())
                        .expect("absolute tool bytes are identity even when they live outside the workspace")
                    },
                );
                assert_ne!(second, outside_hash, "absolute tool contents remain part of identity");

                let escaped = workspace.path().join("..").join("escaped-rustc");
                fs::write(&escaped, b"escaped compiler").expect("escaped tool bytes");
                let error = temp_env::with_var("RUSTC", Some("../escaped-rustc"), || {
                    freshness::rustdoc_input_fingerprint(workspace.path())
                        .expect_err("relative tools that escape the workspace must fail closed")
                });
                assert!(
                    error.to_string().contains("outside the trusted workspace"),
                    "got: {error}"
                );
            },
        );
    });
}

#[test]
fn property_evaluator_fingerprints_parent_cargo_config_and_resolved_home() {
    super::with_process_environment_lock(|| {
        let outer = tempfile::tempdir().expect("outer tempdir");
        let workspace = outer.path().join("workspace");
        let home = outer.path().join("home");
        fs::create_dir_all(&workspace).expect("workspace directory");
        write_fingerprint_fixture(&workspace);
        fs::create_dir_all(outer.path().join(".cargo")).expect("parent Cargo config directory");
        fs::create_dir_all(home.join(".cargo")).expect("Cargo home config directory");
        let parent_config = outer.path().join(".cargo/config.toml");
        let home_config = home.join(".cargo/config.toml");
        fs::write(&parent_config, "[build]\nrustflags = []\n").expect("parent config");
        fs::write(&home_config, "[term]\nverbose = false\n").expect("home config");

        let prior_rustup_home = std::env::var_os("RUSTUP_HOME").or_else(|| {
            std::env::var_os("HOME")
                .map(|value| std::path::PathBuf::from(value).join(".rustup").into_os_string())
        });
        temp_env::with_vars(
            [
                ("HOME", Some(home.as_os_str())),
                ("CARGO_HOME", None::<&std::ffi::OsStr>),
                ("CARGO_TARGET_DIR", None::<&std::ffi::OsStr>),
                ("RUSTUP_HOME", prior_rustup_home.as_deref()),
            ],
            || {
                let first = freshness::rustdoc_input_fingerprint(&workspace)
                    .expect("Cargo config hierarchy must fingerprint");
                fs::write(&parent_config, "[build]\nrustflags = [\"--cfg=parent_b\"]\n")
                    .expect("changed parent config");
                let second = freshness::rustdoc_input_fingerprint(&workspace)
                    .expect("changed parent config must fingerprint");
                assert_ne!(first, second, "Cargo parent config is a rustdoc input");
                fs::write(&home_config, "[term]\nverbose = true\n").expect("changed home config");
                let third = freshness::rustdoc_input_fingerprint(&workspace)
                    .expect("changed Cargo home config must fingerprint");
                assert_ne!(second, third, "resolved Cargo home config is a rustdoc input");
            },
        );
    });
}

#[test]
fn property_evaluator_excludes_only_the_resolved_target_directory() {
    super::with_process_environment_lock(|| {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        write_fingerprint_fixture(workspace.path());
        let target_dir = workspace.path().join("build-cache");
        let generated = target_dir.join("generated.json");
        fs::create_dir_all(&target_dir).expect("custom target directory");
        fs::write(&generated, b"cargo output a").expect("generated output");
        let nested_source = workspace.path().join("src/target/semantic.rs");
        fs::create_dir_all(nested_source.parent().expect("nested source directory"))
            .expect("nested source directory");
        fs::write(&nested_source, b"semantic input a").expect("nested source");

        temp_env::with_var("CARGO_TARGET_DIR", Some(target_dir.as_os_str()), || {
            let first = freshness::rustdoc_input_fingerprint(workspace.path())
                .expect("custom target directory must be excluded");
            fs::write(&generated, b"cargo output b").expect("changed generated output");
            let second = freshness::rustdoc_input_fingerprint(workspace.path())
                .expect("generated output remains excluded");
            assert_eq!(first, second, "only the exact Cargo target directory is excluded");
            fs::write(&nested_source, b"semantic input b").expect("changed nested source");
            let third = freshness::rustdoc_input_fingerprint(workspace.path())
                .expect("nested source remains authoritative");
            assert_ne!(second, third, "a nested source directory named target is not excluded");
        });
    });
}

#[test]
fn property_evaluator_rejects_the_65th_required_layer_before_export() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let items = workspace.path().join("track/items");
    fs::create_dir_all(&items).expect("items directory");
    let layers = (0..65)
        .map(|index| {
            serde_json::json!({
                "crate": format!("layer_{index}"),
                "path": format!("libs/layer_{index}"),
                "may_depend_on": [],
                "tddd": {
                    "enabled": true,
                    "catalogue_file": format!("layer_{index}-types.json"),
                    "schema_export": {
                        "method": "rustdoc",
                        "targets": [format!("layer_{index}")]
                    }
                }
            })
        })
        .collect::<Vec<_>>();
    fs::write(
        workspace.path().join("architecture-rules.json"),
        serde_json::json!({ "version": 2, "layers": layers }).to_string(),
    )
    .expect("architecture rules");
    let observer =
        crate::tddd::type_signals_executor_adapter::RustdocLaunchObserver::using_json_path(
            workspace.path().join("unused.json"),
        );
    let error = super::rustdoc_contexts::load_authoritative_inputs(
        workspace.path(),
        &items,
        &items,
        &observer,
    )
    .expect_err("the 65th configured context must fail before export");
    assert!(error.to_string().contains("64"));
}

#[cfg(unix)]
#[test]
fn property_evaluator_reads_output_only_through_the_owned_lock() {
    let target = tempfile::tempdir().expect("target tempdir");
    let output = target.path().join("doc/infrastructure.json");
    fs::create_dir_all(output.parent().expect("output parent")).expect("output directory");
    fs::write(&output, b"generation-a").expect("output bytes");
    let lock = RustdocOutputLock::acquire(target.path()).expect("lock acquisition");
    assert_eq!(lock.read_bytes(&output, 1024).expect("locked read"), b"generation-a");
}

#[cfg(unix)]
#[test]
fn property_evaluator_generates_lock_contention_generation_and_uncooperative_writer_states() {
    use std::time::Duration;

    assert_eq!(
        RUSTDOC_OUTPUT_LOCK_TIMEOUT,
        Duration::from_secs(120),
        "the lock timeout is part of the rustdoc execution contract"
    );

    let mut generator = PropertyGenerator::new(0x10cc_2026);
    for case in 0..8 {
        let workspace = tempfile::tempdir().expect("target workspace");
        let parent = workspace.path().join(format!("cargo-target-{}", generator.next_u32()));
        let target = parent.join("selected");
        let output = target.join(format!("doc/output-{}.json", generator.next_u32() % 5));
        fs::create_dir_all(output.parent().expect("output parent")).expect("output directory");
        fs::write(&output, format!("generation-a-{case}")).expect("initial output");

        let first = RustdocOutputLock::acquire(&target).expect("first writer owns the target");
        let contender_target = target.clone();
        let contender = std::thread::spawn(move || {
            RustdocOutputLock::acquire_for_test(&contender_target, Duration::from_millis(25))
        });
        let contention = contender.join().expect("contender thread must finish");
        let contention_error = contention.expect_err("second writer must not enter the lock");
        assert!(
            contention_error.to_string().contains("timed out"),
            "second writer must fail closed at its bounded wait: {contention_error}"
        );
        drop(first);

        // Replace an ancestor and recreate the old pathname. The held
        // descriptor remains pinned to the original generation and must not
        // read the replacement target directory.
        let pinned = RustdocOutputLock::acquire_for_test(&target, Duration::from_millis(25))
            .expect("the target becomes available after the first writer releases it");
        assert_eq!(
            pinned.read_bytes(&output, 1024).expect("descriptor-relative read"),
            format!("generation-a-{case}").as_bytes()
        );
        let moved_parent = workspace.path().join(format!("moved-{}", generator.next_u32()));
        fs::rename(&parent, &moved_parent).expect("rename target ancestor");
        fs::create_dir_all(output.parent().expect("replacement output parent"))
            .expect("replacement output directory");
        fs::write(&output, b"generation-b").expect("replacement output");
        let generation_error = pinned
            .read_bytes(&output, 1024)
            .expect_err("target ancestor replacement must be detected");
        assert!(
            generation_error.to_string().contains("generation")
                || generation_error.to_string().contains("replaced"),
            "target replacement must fail closed: {generation_error}"
        );
        drop(pinned);

        // A non-regular output is the observable fail-closed case for a writer
        // that ignores the lock. The no-follow/nonblocking open must return an
        // error rather than waiting on or reading an uncooperative node.
        let replacement_target = parent.join("selected");
        let replacement_output = replacement_target
            .join(output.strip_prefix(&target).expect("output is relative to selected target"));
        let uncooperative =
            RustdocOutputLock::acquire_for_test(&replacement_target, Duration::from_millis(25))
                .expect("replacement target lock");
        let writer_output = replacement_output.clone();
        let writer = std::thread::spawn(move || {
            fs::remove_file(&writer_output).expect("remove regular output");
            fs::create_dir(&writer_output).expect("create uncooperative output node");
        });
        writer.join().expect("uncooperative writer must finish");
        let uncooperative_error = uncooperative
            .read_bytes(&replacement_output, 1024)
            .expect_err("non-regular output must be rejected");
        assert!(
            uncooperative_error.to_string().contains("regular file"),
            "uncooperative output must fail closed: {uncooperative_error}"
        );
    }
}

#[cfg(not(unix))]
#[test]
fn test_non_unix_lock_acquisition_fails_closed() {
    let target = tempfile::tempdir().expect("target tempdir");
    let error =
        RustdocOutputLock::acquire_for_test(target.path(), std::time::Duration::from_millis(25))
            .expect_err("descriptor-relative rustdoc locks must be unsupported on non-Unix");
    assert!(
        error.to_string().contains("supported only on Unix"),
        "non-Unix lock acquisition must fail closed: {error}"
    );
}

#[test]
fn property_execute_type_signals_requires_exclusive_target_before_capture() {
    let evaluator = include_str!("type_signals_evaluator.rs");
    let identities = evaluator
        .find("resolve_execution_identities(")
        .expect("evaluator resolves rustdoc execution identities");
    let capture = evaluator
        .find("rustdoc.capture_current(&target_crate_name, target_features)")
        .expect("evaluator captures current rustdoc through the port");
    assert!(
        identities < capture,
        "exclusive identity admission must precede current rustdoc capture"
    );

    let feature_selection = include_str!("type_signals_evaluator/feature_selection.rs");
    let identity = feature_selection
        .find("rustdoc.execution_identity(&target, features)")
        .expect("identities come from the rustdoc provider");
    let exclusive = feature_selection
        .find("require_exclusive_rustdoc_target(")
        .expect("each identity target must be exclusive");
    assert!(
        identity < exclusive,
        "exclusive target ownership must be required after identity resolution"
    );
}

#[test]
fn property_capture_current_rechecks_workspace_fingerprint_around_locked_export() {
    let source = include_str!("rustdoc_crate_adapter.rs");
    let start = source
        .find("let start_fingerprint = workspace_input_fingerprint(&self.workspace_root, crate_name)?;")
        .expect("capture starts with a workspace fingerprint");
    let export = source
        .find(".capture_rustdoc_snapshot(crate_name, features, decode_rustdoc_bytes)")
        .expect("capture delegates to the locked exporter");
    let exclusive = source
        .find("require_exclusive_snapshot_target(crate_name, &snapshot)?;")
        .expect("captured snapshot must remain on an exclusive target");
    let end = source
        .find(
            "let end_fingerprint = workspace_input_fingerprint(&self.workspace_root, crate_name)?;",
        )
        .expect("capture rechecks the workspace fingerprint");
    let reject = source
        .find("reject_changed_workspace_fingerprint(crate_name, &start_fingerprint, &end_fingerprint)?;")
        .expect("changed fingerprints discard the capture");
    assert!(
        start < export && export < exclusive && exclusive < end && end < reject,
        "capture_current must fingerprint, lock-export, require exclusive ownership, then discard on fingerprint change"
    );
}

#[test]
fn property_export_source_holds_lock_from_path_selection_through_byte_read() {
    let source = include_str!("../schema_export.rs");
    let acquire = source
        .find("let lock = RustdocOutputLock::acquire(&target_directory)?;")
        .expect("export acquires the common lock");
    let identity = source
        .find(
            "self.rustdoc_execution_identity_for_target(crate_name, features, &target_directory)?;",
        )
        .expect("expected path is selected after lock acquisition");
    let export = source
        .find("bin_target::run_rustdoc_with_features(")
        .expect("export runs rustdoc while the lock is in scope");
    let read = source
        .find("lock.read_bytes(&expected_path, 64 * 1024 * 1024)?;")
        .expect("bytes are read through the held lock");
    assert!(
        acquire < identity && identity < export && export < read,
        "lock must remain held from expected-path selection through export and JSON-byte read"
    );
    let path_check = source
        .find("if output_path != expected_path")
        .expect("export verifies the expected JSON path before reading");
    assert!(
        export < path_check && path_check < read,
        "expected-path validation must stay inside the lock interval before byte copy"
    );
}

#[cfg(unix)]
#[test]
fn property_evaluator_fails_closed_on_lock_operation_without_retry_or_fallback() {
    use std::time::Duration;

    let workspace = tempfile::tempdir().expect("lock-op workspace");
    let not_a_directory = workspace.path().join("regular-file-target");
    fs::write(&not_a_directory, b"not-a-directory").expect("regular file target");

    let first = RustdocOutputLock::acquire_for_test(&not_a_directory, Duration::from_millis(25));
    let first_error = first.expect_err("locking a regular file must fail closed");
    assert!(
        first_error.to_string().contains("lock")
            || first_error.to_string().contains("directory")
            || first_error.to_string().contains("cannot open"),
        "lock-operation failure must be reported: {first_error}"
    );

    let retry = RustdocOutputLock::acquire_for_test(&not_a_directory, Duration::from_millis(25));
    retry.expect_err("a failed lock operation must not retry into lockless success");
}

#[test]
fn property_evaluator_rejects_an_io_aba_generation() {
    super::with_process_environment_lock(|| {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        fs::write(
            workspace.path().join("Cargo.toml"),
            "[package]\nname = \"fingerprint-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("fixture manifest");
        fs::write(workspace.path().join("Cargo.lock"), "version = 4\n").expect("fixture lockfile");
        fs::create_dir_all(workspace.path().join("src")).expect("fixture source directory");
        fs::write(workspace.path().join("src/lib.rs"), "pub struct Fixture;\n")
            .expect("fixture source");
        let source = workspace.path().join("source.rs");
        fs::write(&source, b"generation-a").expect("initial source");
        let first =
            freshness::rustdoc_input_fingerprint(workspace.path()).expect("first fingerprint");

        let replacement = workspace.path().join("replacement.rs");
        fs::write(&replacement, b"generation-b").expect("replacement source");
        fs::rename(&replacement, &source).expect("replace source");
        let middle_generation =
            freshness::rustdoc_input_fingerprint(workspace.path()).expect("changed fingerprint");
        assert_ne!(
            first, middle_generation,
            "the intermediate generation must invalidate the input identity"
        );

        fs::write(&replacement, b"generation-a").expect("restored source");
        fs::rename(&replacement, &source).expect("restore source");
        let final_generation =
            freshness::rustdoc_input_fingerprint(workspace.path()).expect("restored fingerprint");
        assert_ne!(
            first, final_generation,
            "an A-B-A replacement must not reproduce the original input identity"
        );
    });
}
