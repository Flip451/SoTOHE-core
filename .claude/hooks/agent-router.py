#!/usr/bin/env python3
"""
UserPromptSubmit hook: Route to appropriate agent based on user intent.

Routing rules (Rust project):
- Multimodal files (PDF/video/audio/image) -> multimodal_reader capability
- Codebase understanding / large analysis  -> researcher capability
- External research / crate survey         -> researcher capability
- Planning / design                        -> planner capability
- Debugging / error diagnosis              -> debugger capability
- Reviews                                  -> reviewer capability
- Implementation / refactor work           -> implementer capability
- takt / track related                     -> suggest appropriate workflow tool
"""

import json
import re
import sys
from pathlib import Path

from _agent_profiles import provider_label, render_provider_example
from _shared import load_stdin_json, print_hook_error

HOOKS_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = HOOKS_DIR.parent.parent
# Temporarily prepend to ensure our scripts package is found, then remove
# to avoid shadowing stdlib modules for the rest of the process.
_project_root_str = str(PROJECT_ROOT)
sys.path.insert(0, _project_root_str)
import scripts.external_guides as external_guides

if _project_root_str in sys.path:
    sys.path.remove(_project_root_str)

MULTIMODAL_EXTENSIONS = [
    ".pdf",
    ".mp4",
    ".mov",
    ".avi",
    ".mkv",
    ".webm",
    ".mp3",
    ".wav",
    ".m4a",
    ".flac",
    ".ogg",
    ".png",
    ".jpg",
    ".jpeg",
    ".gif",
    ".webp",
    ".svg",
]

_MULTIMODAL_EXT_GROUP = "|".join(ext.lstrip(".") for ext in MULTIMODAL_EXTENSIONS)
_MULTIMODAL_EXT_RE = r"\.(?:" + _MULTIMODAL_EXT_GROUP + r")"
# Match file paths that may contain spaces when quoted or escaped.
MULTIMODAL_PATTERN = re.compile(
    r"(?:"
    # Double-quoted path (may contain apostrophes): "Bob's Notes.pdf"
    r'"(?P<dqpath>[^"]+' + _MULTIMODAL_EXT_RE + r')"'
    r"|"
    # Single-quoted path (may contain double quotes): '/path/to/file.pdf'
    r"'(?P<sqpath>[^']+" + _MULTIMODAL_EXT_RE + r")'"
    r"|"
    # Unquoted path (no spaces): /path/to/file.pdf
    r"(?P<upath>[\w./\\~-]+" + _MULTIMODAL_EXT_RE + r")"
    r")(?:\s|$|[.,;:!?)])",
    re.IGNORECASE,
)

# Strong debugger signals: checked after researcher but BEFORE planner so that
# concrete error-diagnosis patterns beat planner triggers like "借用" / "ownership".
# Only include patterns that are unambiguously error diagnosis — generic phrases
# like "compiler error" or "lifetime mismatch" stay in DEBUGGER_TRIGGERS only,
# because they can appear in research/implementation contexts.
STRONG_DEBUGGER_PATTERN = re.compile(
    r"(?<![a-z0-9_])e0\d{3}(?![a-z0-9_])"  # Rust E-codes: e0382, e0505, etc.
    r"|コンパイル通らない"
    r"|コンパイルエラー"
    r"|(?<![a-z0-9_])moved value(?![a-z0-9_])"
    r"|(?<![a-z0-9_])borrow conflict(?![a-z0-9_])"
    r"|(?<![a-z0-9_])cannot borrow(?![a-z0-9_])",
    re.IGNORECASE,
)

# Explicit review intent words: when present, reviewer is checked before planner
# so that topic signals like "method signature" don't steal review requests.
EXPLICIT_REVIEW_WORDS = re.compile(
    r"(?<![a-z0-9_])review(?![a-z0-9_])"
    r"|レビュー"
    r"|コードレビュー"
    r"|見てほしい"
    r"|確認してほしい",
    re.IGNORECASE,
)

PLANNER_TRIGGERS = {
    "ja": [
        "設計",
        "どう設計",
        "アーキテクチャ",
        "計画",
        "計画を立てて",
        "実装計画",
        "どちらがいい",
        "比較して",
        "トレードオフ",
        "検討して",
        "考えて",
        "分析して",
        "深く",
        "迷ってる",
        "所有権",
        "ライフタイム",
        "借用",
        "トレイト設計",
        "ジェネリクス",
        "設計したい",
    ],
    "en": [
        "design",
        "architecture",
        "architect",
        "plan",
        "planning",
        "compare",
        "trade-off",
        "tradeoff",
        "which is better",
        "think",
        "analyze",
        "deeply",
        "ownership",
        "lifetime",
        "borrow",
        "trait design",
        "generics",
        "tdd",
        "red green refactor",
        "hexagonal",
        "domain layer",
        "usecase layer",
        "ddd",
        "command pattern",
        "query pattern",
        "arc vs",
        "box vs",
        "rc vs",
        "async-trait vs",
        "rpitit",
        "method signature",
        "return type",
        "option vs",
        "clone vs borrow",
        "owned vs borrowed",
    ],
}

# NOTE: Entries also in STRONG_DEBUGGER_PATTERN are duplicated intentionally
# so they still surface a debugger hint when the strong pre-scan is refactored.
DEBUGGER_TRIGGERS = {
    "ja": [
        "なぜ動かない",
        "エラー",
        "バグ",
        "デバッグ",
        "コンパイル通らない",
        "コンパイルエラー",
    ],
    "en": [
        "debug",
        "error",
        "bug",
        "not working",
        "fails",
        "e0382",
        "e0505",
        "e0507",
        "e0277",
        "e0308",
        "moved value",
        "borrow conflict",
        "compiler error",
        "lifetime mismatch",
    ],
}

REVIEWER_TRIGGERS = {
    "ja": [
        "レビュー",
        "コードレビュー",
        "イディオマティック",
        "正しさ",
        "見てほしい",
        "確認してほしい",
    ],
    "en": [
        "review",
        "idiomatic",
        "rust patterns",
    ],
}

IMPLEMENTER_TRIGGERS = {
    "ja": [
        "実装",
        "実装して",
        "テスト",
        "テストして",
        "実装方法",
        "どう実装",
        "リファクタリング",
        "リファクタ",
        "最適化",
        "パフォーマンス",
    ],
    "en": [
        "implement",
        "test",
        "write tests",
        "how to implement",
        "implementation",
        "complex",
        "refactor",
        "simplify",
        "optimize",
        "performance",
    ],
}

RESEARCHER_TRIGGERS = {
    "ja": [
        "調べて",
        "リサーチ",
        "調査",
        "サーベイ",
        "最新",
        "ドキュメント",
        "クレート",
        "ライブラリ",
        "パッケージ",
        "コードベース",
        "リポジトリ",
        "全体構造",
        "理解して",
        "把握して",
    ],
    "en": [
        "research",
        "investigate",
        "look up",
        "find out",
        "survey",
        "latest",
        "documentation",
        "docs",
        "crate",
        "library",
        "package",
        "framework",
        "codebase",
        "repository",
        "project structure",
        "understand",
        "analyze the code",
    ],
}

CAPABILITY_TRIGGERS = {
    "researcher": RESEARCHER_TRIGGERS,
    "planner": PLANNER_TRIGGERS,
    "debugger": DEBUGGER_TRIGGERS,
    "reviewer": REVIEWER_TRIGGERS,
    "implementer": IMPLEMENTER_TRIGGERS,
}
WORKFLOW_TRIGGERS = {
    "ja": [
        "仕様",
        "スペック",
        "spec",
        "takt",
        "ワークフロー実行",
        "track",
        "トラック",
    ],
    "en": [
        "spec",
        "specification",
        "takt",
        "workflow",
        "track",
        "new track",
        "newtrack",
    ],
}

# ================================================================
# Weighted scoring constants and cue classification
# ================================================================

INTENT_WEIGHT = 4
INTERROGATIVE_WEIGHT = 3
DOMAIN_WEIGHT = 1
CLEAR_MIN_SCORE = 4
CLEAR_MIN_MARGIN = 2

# Interrogative patterns that signal planner intent even without explicit planning verbs.
PLANNER_INTENT_PATTERNS = [
    # English interrogative patterns
    re.compile(r"\bshould\s+(?:we|i|you)\b", re.IGNORECASE),
    re.compile(r"\bhow\s+should\b", re.IGNORECASE),
    re.compile(r"\bcould\s+(?:we|i)\b", re.IGNORECASE),
    re.compile(r"\bwould\s+(?:it|we)\s+be\s+better\b", re.IGNORECASE),
    # Japanese interrogative patterns
    re.compile(r"べきか"),
    re.compile(r"どうすべき"),
    re.compile(r"したほうがいい"),
    re.compile(r"していいか"),
]

# Scored cues: intent cues express action type (+4), domain cues express topic (+1).
# Per-capability, only the higher family scores (intent XOR domain, not both).
SCORED_CUES: dict[str, dict[str, list[str]]] = {
    "planner": {
        "intent": [
            "設計",
            "どう設計",
            "計画",
            "計画を立てて",
            "計画して",
            "実装計画",
            "どちらがいい",
            "比較して",
            "トレードオフ",
            "検討して",
            "迷ってる",
            "設計したい",
            "compare",
            "trade-off",
            "tradeoff",
            "which is better",
            "red green refactor",
        ],
        "domain": [
            "アーキテクチャ",
            "深く",
            "所有権",
            "ライフタイム",
            "借用",
            "トレイト設計",
            "ジェネリクス",
            "design",
            "architecture",
            "architect",
            "plan",
            "planning",
            "ownership",
            "lifetime",
            "borrow",
            "trait design",
            "generics",
            "tdd",
            "hexagonal",
            "domain layer",
            "usecase layer",
            "ddd",
            "command pattern",
            "query pattern",
            "arc vs",
            "box vs",
            "rc vs",
            "async-trait vs",
            "rpitit",
            "method signature",
            "return type",
            "option vs",
            "clone vs borrow",
            "owned vs borrowed",
            "deeply",
            "think",
            "analyze",
            "考えて",
            "分析して",
        ],
    },
    "implementer": {
        "intent": [
            "実装して",
            "テストして",
            "どう実装",
            "リファクタリング",
            "リファクタ",
            "implement",
            "write tests",
            "how to implement",
            "refactor",
            "simplify",
        ],
        "domain": [
            "実装",
            "テスト",
            "実装方法",
            "パフォーマンス",
            "最適化",
            "test",
            "implementation",
            "complex",
            "performance",
            "optimize",
        ],
    },
    "debugger": {
        "intent": [
            "なぜ動かない",
            "デバッグ",
            "debug",
            "not working",
        ],
        "domain": [
            "エラー",
            "バグ",
            "error",
            "bug",
            "fails",
            "compile",
            "compiler error",
            "lifetime mismatch",
            "コンパイル通らない",
            "コンパイルエラー",
            "コンパイル",
            "e0382",
            "e0505",
            "e0507",
            "e0277",
            "e0308",
            "moved value",
            "borrow conflict",
            "cannot borrow",
        ],
    },
    "reviewer": {
        "intent": [
            "レビュー",
            "コードレビュー",
            "見てほしい",
            "確認してほしい",
            "review",
        ],
        "domain": [
            "正しさ",
            "rust patterns",
            "イディオマティック",
            "idiomatic",
        ],
    },
    "researcher": {
        "intent": [
            "調べて",
            "リサーチ",
            "調査",
            "サーベイ",
            "research",
            "investigate",
            "survey",
            "look up",
            "find out",
        ],
        "domain": [
            "最新",
            "ドキュメント",
            "クレート",
            "ライブラリ",
            "パッケージ",
            "コードベース",
            "リポジトリ",
            "全体構造",
            "理解して",
            "把握して",
            "latest",
            "documentation",
            "docs",
            "crate",
            "library",
            "package",
            "framework",
            "codebase",
            "repository",
            "project structure",
            "understand",
            "analyze the code",
        ],
    },
}

# Priority order for tie-breaking: planner is the safest default.
SCORING_TIEBREAK_ORDER = (
    "planner",
    "debugger",
    "reviewer",
    "implementer",
    "researcher",
)


def _has_planner_interrogative(prompt_lower: str) -> bool:
    """Check if prompt contains a planner interrogative pattern."""
    return any(p.search(prompt_lower) for p in PLANNER_INTENT_PATTERNS)


def score_keywords(prompt_lower: str) -> dict[str, int]:
    """Score prompt against each capability using intent/domain cue weights.

    Per capability, only the higher family scores (intent beats domain).
    Each family is capped at 1 hit.
    """
    scores: dict[str, int] = {cap: 0 for cap in SCORED_CUES}
    has_interrogative = _has_planner_interrogative(prompt_lower)
    for capability, families in SCORED_CUES.items():
        intent_hit = any(
            trigger_matches(prompt_lower, t) for t in families.get("intent", [])
        )
        # Planner also checks interrogative regex patterns (lower weight).
        interrogative_hit = False
        if capability == "planner" and not intent_hit and has_interrogative:
            interrogative_hit = True
        if intent_hit:
            scores[capability] = INTENT_WEIGHT
        elif interrogative_hit:
            scores[capability] = INTERROGATIVE_WEIGHT
        else:
            domain_hit = any(
                trigger_matches(prompt_lower, t) for t in families.get("domain", [])
            )
            if domain_hit:
                scores[capability] = DOMAIN_WEIGHT
    # When a planner interrogative is present ("should we", "could we", etc.),
    # demote implementer intent to domain — the user is asking for advice, not action.
    if has_interrogative and scores.get("implementer", 0) >= INTENT_WEIGHT:
        scores["implementer"] = DOMAIN_WEIGHT
    return scores


def is_clear(scores: dict[str, int]) -> bool:
    """Return True when the top scorer has a decisive margin."""
    sorted_scores = sorted(scores.values(), reverse=True)
    top = sorted_scores[0]
    second = sorted_scores[1] if len(sorted_scores) > 1 else 0
    return top >= CLEAR_MIN_SCORE and (top - second) >= CLEAR_MIN_MARGIN


def _top_capability(scores: dict[str, int]) -> str | None:
    """Return highest-scoring capability, using tiebreak order on ties."""
    max_score = max(scores.values())
    if max_score <= 0:
        return None
    for cap in SCORING_TIEBREAK_ORDER:
        if scores.get(cap) == max_score:
            return cap
    return None


def _find_scored_trigger(prompt_lower: str, capability: str) -> str:
    """Find the first matching trigger string for a scored capability."""
    families = SCORED_CUES.get(capability, {})
    for trigger in families.get("intent", []):
        if trigger_matches(prompt_lower, trigger):
            return trigger
    if capability == "planner":
        for pattern in PLANNER_INTENT_PATTERNS:
            m = pattern.search(prompt_lower)
            if m:
                return m.group(0)
    for trigger in families.get("domain", []):
        if trigger_matches(prompt_lower, trigger):
            return trigger
    return capability


MULTIMODAL_PREFIX = "[Multimodal Routing]"
MULTIMODAL_TEMPLATE = (
    "{prefix} Found '{trigger}' in prompt. "
    "**MUST** route the `multimodal_reader` capability to {provider_label}. "
    "Example: `{provider_example}`"
)
CAPABILITY_PREFIX = "[Agent Routing]"
RESEARCH_PREFIX = "[Research Routing]"
CAPABILITY_TEMPLATE = (
    "{prefix} Detected '{trigger}' -- this task maps to the `{capability}` capability. "
    "Use {provider_label} for {capability_description}. "
    "Example: `{provider_example}`"
)
WORKFLOW_PREFIX = "[Workflow Hint]"
WORKFLOW_TEMPLATE = (
    "{prefix} Detected '{trigger}' -- consider using the appropriate tool: "
    "For planning + track creation: `/track:plan <feature>`. "
    "For planning-only kickoff: `/track:plan-only <feature>`. "
    "For activating a planning-only track: `/track:activate <track-id>`. "
    "For adding an external guide index entry: `/guide:add`. "
    "For a new project convention doc: `/conventions:add <name>`. "
    "For autonomous full cycle in Claude Code: `/track:full-cycle <task>`. "
    "For interactive parallel implementation/review: `/track:implement` / `/track:review`. "
    "For validation and commit: `/track:ci` / `/track:commit <message>`. "
    "For PR lifecycle helpers after a branch is pushed: "
    "`cargo make track-pr-status` / `cargo make track-pr-review` / `cargo make track-pr-merge`."
)
EXTERNAL_GUIDES_PREFIX = "[External Guide Context]"
EXTERNAL_GUIDES_COMMAND_TRIGGERS = [
    "/track:plan",
    "/track:plan-only",
    "/track:activate",
    "/track:implement",
    "/track:review",
    "/track:full-cycle",
]
CAPABILITY_DESCRIPTIONS = {
    "planner": "planning, design, ownership/lifetime analysis, and architecture decisions",
    "researcher": "large-context analysis, crate surveys, documentation lookup, and version research",
    "implementer": "complex Rust implementation, refactoring, and performance-oriented edits",
    "reviewer": "code review, correctness checks, idiomatic Rust validation, and performance review",
    "debugger": "compiler error diagnosis, failing test analysis, and root-cause debugging",
}
CAPABILITY_EXAMPLE_TASKS = {
    "planner": "Review this Rust design: {description}",
    "researcher": "Research Rust crate: {name}. Latest version, features, idiomatic usage, known issues, alternatives",
    "implementer": "Implement this Rust task: {task description}",
    "reviewer": "Review this Rust implementation: $(git diff)",
    "debugger": "Debug this Rust error: <full error output>",
}
CAPABILITY_PREFIXES = {
    "researcher": RESEARCH_PREFIX,
    "planner": CAPABILITY_PREFIX,
    "implementer": CAPABILITY_PREFIX,
    "reviewer": CAPABILITY_PREFIX,
    "debugger": CAPABILITY_PREFIX,
}


def detect_multimodal_file(prompt: str) -> str | None:
    match = MULTIMODAL_PATTERN.search(prompt)
    if match:
        # Prefer quoted path (may contain spaces), fall back to unquoted path.
        return match.group("dqpath") or match.group("sqpath") or match.group("upath")
    return None


def trigger_matches(prompt_lower: str, trigger: str) -> bool:
    if re.search(r"[a-z0-9]", trigger):
        pattern = rf"(?<![a-z0-9_]){re.escape(trigger)}(?![a-z0-9_])"
        return re.search(pattern, prompt_lower) is not None
    return trigger in prompt_lower


def detect_agent(prompt: str) -> tuple[str | None, str, bool]:
    prompt_lower = prompt.lower()

    multimodal_file = detect_multimodal_file(prompt)
    if multimodal_file:
        return "multimodal_reader", multimodal_file, True

    # Explicit slash commands should always surface workflow guidance first.
    if "/architecture-customizer" in prompt_lower:
        return "workflow", "/architecture-customizer", False
    if "/guide:add" in prompt_lower:
        return "workflow", "/guide:add", False
    if "/conventions:add" in prompt_lower:
        return "workflow", "/conventions:add", False
    if "/track:" in prompt_lower:
        return "workflow", "track", False

    # Phase 1: Explicit review intent beats all other capabilities.
    if EXPLICIT_REVIEW_WORDS.search(prompt_lower):
        for triggers in CAPABILITY_TRIGGERS["reviewer"].values():
            for trigger in triggers:
                if trigger_matches(prompt_lower, trigger):
                    return "reviewer", trigger, False

    # Phase 2: Researcher has high priority, but yields to strong debugger
    # when no researcher intent cue is present (domain-only researcher cues
    # like "最新" or "codebase" should not steal from concrete error prompts).
    researcher_has_intent = any(
        trigger_matches(prompt_lower, t)
        for t in SCORED_CUES["researcher"].get("intent", [])
    )
    strong_debugger_early = STRONG_DEBUGGER_PATTERN.search(prompt_lower)
    if not (strong_debugger_early and not researcher_has_intent):
        for triggers in CAPABILITY_TRIGGERS["researcher"].values():
            for trigger in triggers:
                if trigger_matches(prompt_lower, trigger):
                    return "researcher", trigger, False

    # Phase 3: Weighted scoring for remaining capabilities (reviewer, planner,
    # implementer, debugger — researcher domain-only cues also land here when
    # a strong debugger signal suppressed the Phase 2 early-exit).
    scores = score_keywords(prompt_lower)

    # Phase 3a: Detect planner interrogative (shared helper, consistent with scoring).
    has_interrogative = _has_planner_interrogative(prompt_lower)
    planner_score = scores.get("planner", 0)

    # Phase 3b: If implementer has intent, skip strong debugger (let scoring decide).
    has_implementer_intent = scores.get("implementer", 0) >= INTENT_WEIGHT

    # Phase 3c: Strong debugger signals beat planner triggers like "借用" / "ownership",
    # but skip when:
    # - implementer has intent (scoring is more accurate)
    # - planner has intent-level score (+4) — explicit planning verbs beat error tokens
    # - planner interrogative + demoted implementer verb (user asking advice about action)
    # Note: pure interrogative + debugger domain (no planner intent/action) → debugger wins.
    # Ambiguous cases like "Should we use Arc to avoid E0505?" are deferred to Phase 2 LLM.
    has_demoted_implementer = has_interrogative and any(
        trigger_matches(prompt_lower, t)
        for t in SCORED_CUES.get("implementer", {}).get("intent", [])
    )
    suppress_debugger = (
        has_implementer_intent
        or planner_score >= INTENT_WEIGHT
        or (has_interrogative and has_demoted_implementer)
    )
    if not suppress_debugger:
        strong_match = STRONG_DEBUGGER_PATTERN.search(prompt_lower)
        if strong_match:
            return "debugger", strong_match.group(0), False

    # Phase 3c: Return top scorer (planner wins ties).
    top_cap = _top_capability(scores)
    if top_cap and scores[top_cap] > 0:
        trigger = _find_scored_trigger(prompt_lower, top_cap)
        return top_cap, trigger, False

    # Phase 4: Workflow triggers (last resort).
    for triggers in WORKFLOW_TRIGGERS.values():
        for trigger in triggers:
            if trigger_matches(prompt_lower, trigger):
                return "workflow", trigger, False

    return None, "", False


def build_multimodal_message(trigger: str) -> str:
    return MULTIMODAL_TEMPLATE.format(
        prefix=MULTIMODAL_PREFIX,
        trigger=trigger,
        provider_label=provider_label("multimodal_reader"),
        provider_example=render_provider_example(
            "multimodal_reader",
            task="{what to extract}",
            file_path=trigger,
        ),
    )


def build_capability_message(capability: str, trigger: str) -> str:
    return CAPABILITY_TEMPLATE.format(
        prefix=CAPABILITY_PREFIXES[capability],
        trigger=trigger,
        capability=capability,
        provider_label=provider_label(capability),
        capability_description=CAPABILITY_DESCRIPTIONS[capability],
        provider_example=render_provider_example(
            capability,
            task=CAPABILITY_EXAMPLE_TASKS[capability],
        ),
    )


def build_workflow_message(trigger: str) -> str:
    return WORKFLOW_TEMPLATE.format(prefix=WORKFLOW_PREFIX, trigger=trigger)


def should_inject_external_guides(prompt: str) -> bool:
    prompt_lower = prompt.lower()
    return any(trigger in prompt_lower for trigger in EXTERNAL_GUIDES_COMMAND_TRIGGERS)


def find_external_guide_matches(prompt: str) -> list[tuple[dict, str]]:
    if not should_inject_external_guides(prompt):
        return []
    try:
        return external_guides.find_relevant_guides_for_track_workflow(prompt)
    except Exception:
        return []


def build_external_guides_message(matches: list[tuple[dict, str]]) -> str:
    if not matches:
        return ""

    lines = [
        f"{EXTERNAL_GUIDES_PREFIX} Relevant guide summaries for this track workflow:"
    ]
    for guide, trigger in matches:
        summary = " ".join(guide.get("summary", [])) or "Summary not recorded."
        usage = (
            " ".join(guide.get("project_usage", [])) or "Project usage not recorded."
        )
        cache_path = guide.get("cache_path", "")
        lines.append(f"- {guide['id']} (trigger: {trigger}): {summary}")
        lines.append(f"  project usage: {usage}")
        if cache_path:
            lines.append(f"  cache path: {cache_path}")
    lines.append(
        f"Avoid reading the full cached guide with the Read tool. "
        f"Use Grep for specific sections; if full-document analysis is needed, "
        f"delegate to the `researcher` capability ({provider_label('researcher')})."
    )
    return "\n".join(lines)


def build_user_prompt_context(prompt: str) -> str | None:
    agent, trigger, is_multimodal = detect_agent(prompt)
    messages: list[str] = []

    if is_multimodal:
        messages.append(build_multimodal_message(trigger))
    elif agent in CAPABILITY_DESCRIPTIONS:
        messages.append(build_capability_message(agent, trigger))
    elif agent == "workflow":
        messages.append(build_workflow_message(trigger))

    guide_message = build_external_guides_message(find_external_guide_matches(prompt))
    if guide_message:
        messages.append(guide_message)

    return "\n\n".join(messages) if messages else None


def main() -> None:
    try:
        data = load_stdin_json()
        prompt = data.get("prompt", "")

        context = build_user_prompt_context(prompt)
        if context:
            output = {
                "hookSpecificOutput": {
                    "hookEventName": "UserPromptSubmit",
                    "additionalContext": context,
                }
            }
            print(json.dumps(output))

        sys.exit(0)

    except Exception as err:
        print_hook_error(err)
        sys.exit(0)


if __name__ == "__main__":
    main()
