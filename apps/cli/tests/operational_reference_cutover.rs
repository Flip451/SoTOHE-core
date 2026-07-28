#![allow(clippy::expect_used, clippy::panic)]

const RETIRED_PLANNER_TOKENS: &[&str] = &["plan codex-local", "PlanCodexLocal"];
const RETIRED_SIGNAL_EXECUTION_TOKENS: &[&str] = &["SignalServiceImpl", "signal::shim"];

const LIVE_OPERATIONAL_SURFACES: &[(&str, &str)] = &[
    (
        ".claude/rules/dev-environment.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../.claude/rules/dev-environment.md"
        )),
    ),
    (
        ".claude/rules/guardrails.md",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.claude/rules/guardrails.md")),
    ),
    (
        ".claude/commands/track/plan.md",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.claude/commands/track/plan.md")),
    ),
    (
        ".claude/commands/track/impl-plan.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../.claude/commands/track/impl-plan.md"
        )),
    ),
    (
        ".claude/commands/track/obligation-fulfillment.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../.claude/commands/track/obligation-fulfillment.md"
        )),
    ),
    (
        ".claude/commands/track/diagnose.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../.claude/commands/track/diagnose.md"
        )),
    ),
    (
        ".claude/settings.json",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.claude/settings.json")),
    ),
    (
        ".codex/rules/default.rules",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.codex/rules/default.rules")),
    ),
    (
        ".claude/skills/codex-system/SKILL.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../.claude/skills/codex-system/SKILL.md"
        )),
    ),
];

const LIVE_SIGNAL_EXECUTION_SOURCES: &[(&str, &str)] = &[
    (
        "apps/cli/src/commands/signal/mod.rs",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/commands/signal/mod.rs")),
    ),
    (
        "apps/cli/src/commands/signal/calc_adr_user.rs",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/commands/signal/calc_adr_user.rs")),
    ),
    (
        "apps/cli/src/commands/signal/calc_spec_adr.rs",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/commands/signal/calc_spec_adr.rs")),
    ),
    (
        "apps/cli/src/commands/signal/calc_catalog_spec.rs",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/commands/signal/calc_catalog_spec.rs"
        )),
    ),
    (
        "apps/cli/src/commands/signal/calc_impl_catalog.rs",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/commands/signal/calc_impl_catalog.rs"
        )),
    ),
    (
        "apps/cli/src/commands/signal/check_adr_user.rs",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/commands/signal/check_adr_user.rs")),
    ),
    (
        "apps/cli/src/commands/signal/check_spec_adr.rs",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/commands/signal/check_spec_adr.rs")),
    ),
    (
        "apps/cli/src/commands/signal/check_catalog_spec.rs",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/commands/signal/check_catalog_spec.rs"
        )),
    ),
    (
        "apps/cli/src/commands/signal/check_impl_catalog.rs",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/commands/signal/check_impl_catalog.rs"
        )),
    ),
    (
        "apps/cli-composition/src/signal/mod.rs",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../cli-composition/src/signal/mod.rs")),
    ),
    (
        "apps/cli-driver/src/signal.rs",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../cli-driver/src/signal.rs")),
    ),
    (
        "libs/usecase/src/signal_service.rs",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../libs/usecase/src/signal_service.rs"
        )),
    ),
    (
        "libs/infrastructure/src/signal.rs",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../libs/infrastructure/src/signal.rs"
        )),
    ),
];

const SIGNAL_ENTRY_POINT_ROUTES: &[(&str, &str, &str)] = &[
    ("apps/cli/src/commands/signal/calc_adr_user.rs", "CalcAdrUser", "calc_adr_user"),
    ("apps/cli/src/commands/signal/calc_spec_adr.rs", "CalcSpecAdr", "calc_spec_adr"),
    ("apps/cli/src/commands/signal/calc_catalog_spec.rs", "CalcCatalogSpec", "calc_catalog_spec"),
    ("apps/cli/src/commands/signal/calc_impl_catalog.rs", "CalcImplCatalog", "calc_impl_catalog"),
    ("apps/cli/src/commands/signal/check_adr_user.rs", "CheckAdrUser", "check_adr_user"),
    ("apps/cli/src/commands/signal/check_spec_adr.rs", "CheckSpecAdr", "check_spec_adr"),
    (
        "apps/cli/src/commands/signal/check_catalog_spec.rs",
        "CheckCatalogSpec",
        "check_catalog_spec",
    ),
    (
        "apps/cli/src/commands/signal/check_impl_catalog.rs",
        "CheckImplCatalog",
        "check_impl_catalog",
    ),
];

fn expected_dispatch_markers(path: &str) -> &[&str] {
    match path {
        ".claude/settings.json" => &["bin/sotp capability exec", "Bash(bin/sotp capability:*)"],
        ".codex/rules/default.rules" => &["[\"bin/sotp\", \"capability\", \"exec\"]"],
        _ => &["capability exec"],
    }
}

#[test]
fn test_live_operational_reference_surfaces_retired_planner_not_present() {
    let mut stale_references = Vec::new();

    for (path, content) in LIVE_OPERATIONAL_SURFACES {
        for dispatch_marker in expected_dispatch_markers(path) {
            assert!(
                content.contains(dispatch_marker),
                "{path} does not contain expected dispatch marker {dispatch_marker:?}"
            );
        }
        for retired_token in RETIRED_PLANNER_TOKENS {
            for (line_number, line) in content.lines().enumerate() {
                if line.contains(retired_token) {
                    stale_references.push(format!(
                        "{path}:{} still contains retired planner token {retired_token:?}",
                        line_number + 1
                    ));
                }
            }
        }
    }

    assert!(
        stale_references.is_empty(),
        "retired planner references remain:\n{}",
        stale_references.join("\n")
    );

    let mut retired_signal_references = Vec::new();
    for (path, content) in LIVE_SIGNAL_EXECUTION_SOURCES {
        for retired_token in RETIRED_SIGNAL_EXECUTION_TOKENS {
            for (line_number, line) in content.lines().enumerate() {
                if line.contains(retired_token) {
                    retired_signal_references.push(format!(
                        "{path}:{} still contains retired signal execution token {retired_token:?}",
                        line_number + 1
                    ));
                }
            }
        }
    }
    assert!(
        retired_signal_references.is_empty(),
        "retired signal execution references remain:\n{}",
        retired_signal_references.join("\n")
    );

    let driver = LIVE_SIGNAL_EXECUTION_SOURCES
        .iter()
        .find_map(|(path, content)| (*path == "apps/cli-driver/src/signal.rs").then_some(*content))
        .expect("the live Signal sources must include the driver");
    let usecase = LIVE_SIGNAL_EXECUTION_SOURCES
        .iter()
        .find_map(|(path, content)| {
            (*path == "libs/usecase/src/signal_service.rs").then_some(*content)
        })
        .expect("the live Signal sources must include the interactor");
    let composition = LIVE_SIGNAL_EXECUTION_SOURCES
        .iter()
        .find_map(|(path, content)| {
            (*path == "apps/cli-composition/src/signal/mod.rs").then_some(*content)
        })
        .expect("the live Signal sources must include composition wiring");
    let infrastructure = LIVE_SIGNAL_EXECUTION_SOURCES
        .iter()
        .find_map(|(path, content)| {
            (*path == "libs/infrastructure/src/signal.rs").then_some(*content)
        })
        .expect("the live Signal sources must include the port adapter");

    assert!(
        composition.contains("SignalCommandInteractor::new"),
        "composition must wire the Signal interactor"
    );
    assert!(
        usecase.contains("impl SignalService for SignalCommandInteractor")
            && usecase.contains("self.port")
            && usecase.contains(".execute(command)"),
        "the Signal interactor must delegate through its typed port"
    );
    assert!(
        infrastructure.contains("impl SignalCommandPort for SystemSignalCommandAdapter"),
        "the Signal infrastructure adapter must implement the typed execution port"
    );

    for (path, input, interactor_method) in SIGNAL_ENTRY_POINT_ROUTES {
        let entry_point = LIVE_SIGNAL_EXECUTION_SOURCES
            .iter()
            .find_map(|(candidate_path, content)| (*candidate_path == *path).then_some(*content))
            .unwrap_or_else(|| panic!("missing Signal entry point {path}"));
        assert!(
            entry_point.contains(&format!("driver.handle(SignalInput::{input}")),
            "{path} must dispatch directly to SignalDriver via SignalInput::{input}"
        );
        assert!(
            driver.contains(&format!("SignalInput::{input}"))
                && driver.contains(&format!("self.service.{interactor_method}(")),
            "{path} must continue through SignalDriver to SignalService::{interactor_method}"
        );
    }

    let aggregate_entry_point = LIVE_SIGNAL_EXECUTION_SOURCES
        .iter()
        .find_map(|(path, content)| {
            (*path == "apps/cli/src/commands/signal/mod.rs").then_some(*content)
        })
        .expect("the live Signal sources must include the aggregate entry point");
    assert!(
        aggregate_entry_point
            .contains("driver.handle(cli_driver::signal::SignalInput::CheckGate {"),
        "the aggregate Signal entry point must dispatch directly to SignalDriver via CheckGate"
    );
    assert!(
        driver.contains("SignalInput::CheckGate") && driver.contains("self.service.check_gate("),
        "the aggregate Signal entry point must continue through SignalDriver to SignalService::check_gate"
    );
    assert!(
        usecase.contains("fn check_gate(")
            && usecase.contains("self.port.execute(ResolvedSignalChainCommand::CheckAdrUser"),
        "SignalService::check_gate must execute aggregate chains through the typed port"
    );
}
