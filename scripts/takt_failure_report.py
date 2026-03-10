#!/usr/bin/env python3
"""
Generate a `.takt/debug-report.md` file from failing command output using the
same diagnostic heuristics as the Claude hook analyzers.
"""

from __future__ import annotations

import argparse
import importlib.util
import sys
from pathlib import Path
from types import ModuleType

MAX_PATH_LENGTH = 4096


def project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def hooks_dir() -> Path:
    return project_root() / ".claude" / "hooks"


def load_hook_module(stem: str) -> ModuleType:
    hooks_path = hooks_dir()
    if str(hooks_path) not in sys.path:
        sys.path.insert(0, str(hooks_path))

    path = hooks_path / f"{stem}.py"
    spec = importlib.util.spec_from_file_location(stem.replace("-", "_"), path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"failed to load hook module: {path}")

    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


post_test_analysis = load_hook_module("post-test-analysis")
error_to_codex = load_hook_module("error-to-codex")


def read_output(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def resolve_project_path(raw_path: str) -> Path:
    if not raw_path or len(raw_path) > MAX_PATH_LENGTH:
        raise ValueError("Path is missing or too long")

    path = Path(raw_path)
    if not path.is_absolute():
        path = project_root() / path

    resolved = path.resolve()
    try:
        resolved.relative_to(project_root())
    except ValueError as err:
        raise ValueError(f"Path must stay within project root: {raw_path}") from err

    return resolved


def display_path(path: Path) -> str:
    return path.relative_to(project_root()).as_posix()


def output_excerpt(output: str, max_lines: int = 20) -> str:
    lines = output.strip().splitlines()
    if not lines:
        return "(no output captured)"
    if len(lines) <= max_lines:
        return "\n".join(lines)
    excerpt = "\n".join(lines[-max_lines:])
    return f"... (showing last {max_lines} of {len(lines)} lines)\n{excerpt}"


def primary_guidance(command: str, output: str) -> tuple[str, str, str]:
    if post_test_analysis.is_test_or_build_command(command):
        has_failure, reason = post_test_analysis.has_complex_failure(output)
        if has_failure:
            return (
                "post-test-analysis",
                reason,
                post_test_analysis.build_debug_message(reason),
            )

    if not error_to_codex.should_ignore_command(command):
        errors = error_to_codex.detect_errors(output)
        if errors:
            summary = f"{len(errors)} error pattern(s) detected"
            return (
                "error-to-codex",
                summary,
                error_to_codex.build_error_message(len(errors), output),
            )

    return (
        "none",
        "No hook-derived failure classification matched",
        "Inspect the full output manually and record the blocker before deciding whether to continue or abort.",
    )


def build_report(command: str, output_path: Path, output: str) -> str:
    analyzer, summary, guidance = primary_guidance(command, output)
    return (
        "\n".join(
            [
                "# Takt Debug Report",
                "",
                f"- failing command: `{command}`",
                f"- captured output: `{output_path.as_posix()}`",
                f"- analyzer: `{analyzer}`",
                f"- summary: {summary}",
                "",
                "## Hook-Derived Guidance",
                "",
                guidance,
                "",
                "## Output Excerpt",
                "",
                "```text",
                output_excerpt(output),
                "```",
                "",
                "## Next Step",
                "",
                "- Update this report with the root cause after inspecting the full output.",
                "- Decide whether the work should return to implementation, go back to planning, or ABORT safely.",
            ]
        )
        + "\n"
    )


def write_report(report_path: Path, content: str) -> None:
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(content, encoding="utf-8")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Generate a takt debug report from failing command output."
    )
    parser.add_argument(
        "--command", required=True, help="The command that produced the failure output."
    )
    parser.add_argument(
        "--output-file",
        default=".takt/last-failure.log",
        help="Path to the captured stderr/stdout output file.",
    )
    parser.add_argument(
        "--report-file",
        default=".takt/debug-report.md",
        help="Path to the markdown report file to write.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    raw_args = argv[1:] if argv is not None else None
    if raw_args and raw_args[0] == "--":
        raw_args = raw_args[1:]
    args = parser.parse_args(raw_args)

    try:
        output_path = resolve_project_path(args.output_file)
        report_path = resolve_project_path(args.report_file)
    except ValueError as err:
        print(f"[ERROR] {err}", file=sys.stderr)
        return 1

    if not output_path.exists():
        print(
            f"[ERROR] Output file not found: {display_path(output_path)}",
            file=sys.stderr,
        )
        return 1

    output = read_output(output_path)
    write_report(
        report_path, build_report(args.command, Path(display_path(output_path)), output)
    )
    print(f"[OK] Wrote debug report: {display_path(report_path)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
