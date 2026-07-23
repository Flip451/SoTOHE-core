const RETIRED_PLANNER_TOKENS: &[&str] = &["plan codex-local", "PlanCodexLocal"];

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
}
