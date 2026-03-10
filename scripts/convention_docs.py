#!/usr/bin/env python3
"""
Manage project-specific convention documents and the README index.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

INDEX_START = "<!-- convention-docs:start -->"
INDEX_END = "<!-- convention-docs:end -->"
UPPERCASE_WORDS = {
    "api",
    "cli",
    "cpu",
    "css",
    "db",
    "gpu",
    "grpc",
    "html",
    "http",
    "https",
    "id",
    "io",
    "json",
    "jwt",
    "oauth",
    "sdk",
    "sql",
    "ui",
    "uri",
    "url",
    "ux",
}
FILE_ORDER = {
    "architecture": 10,
    "domain-model": 20,
    "data-model": 30,
    "api-design": 40,
    "error-handling": 50,
    "instrumentation": 60,
    "testing": 70,
    "naming": 80,
    "generated-code": 90,
}


def project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def conventions_dir() -> Path:
    return project_root() / "project-docs" / "conventions"


def readme_path() -> Path:
    return conventions_dir() / "README.md"


def slugify(value: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", value.strip().lower())
    slug = re.sub(r"-{2,}", "-", slug).strip("-")
    return slug


def validate_slug(value: str) -> str:
    slug = slugify(value)
    if not slug:
        raise ValueError("slug must contain at least one ASCII letter or digit")
    if slug != value:
        raise ValueError("slug must be kebab-case ASCII")
    return slug


def default_title(slug: str) -> str:
    words = []
    for part in slug.split("-"):
        if part in UPPERCASE_WORDS:
            words.append(part.upper())
        else:
            words.append(part.capitalize())
    return " ".join(words)


def extract_heading(path: Path) -> str:
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("# "):
            return line[2:].strip()
    return path.stem


def sort_key(path: Path) -> tuple[int, str]:
    stem = path.stem
    return (FILE_ORDER.get(stem, 100), stem)


def render_index_block() -> str:
    entries = []
    for path in sorted(conventions_dir().glob("*.md"), key=sort_key):
        if path.name == "README.md":
            continue
        entries.append(f"- `{path.name}`: {extract_heading(path)}")

    body = (
        "\n".join(entries)
        if entries
        else "- No convention documents yet. Add one with `/conventions:add <name>`."
    )
    return f"{INDEX_START}\n{body}\n{INDEX_END}"


def load_readme_content() -> str:
    path = readme_path()
    if not path.exists():
        raise ValueError(
            f"README index target is missing: {path.relative_to(project_root())}"
        )
    return path.read_text(encoding="utf-8")


def ensure_readme_markers(content: str) -> None:
    pattern = re.compile(
        rf"{re.escape(INDEX_START)}.*?{re.escape(INDEX_END)}", re.DOTALL
    )
    if pattern.search(content) is None:
        raise ValueError(f"README index markers not found in {readme_path()}")


def update_readme_index() -> None:
    path = readme_path()
    content = load_readme_content()
    ensure_readme_markers(content)
    pattern = re.compile(
        rf"{re.escape(INDEX_START)}.*?{re.escape(INDEX_END)}", re.DOTALL
    )
    replacement = render_index_block()
    updated, count = pattern.subn(replacement, content, count=1)
    if count != 1:
        raise ValueError(f"README index markers not found in {path}")
    path.write_text(updated, encoding="utf-8")


def update_index() -> int:
    try:
        update_readme_index()
    except (OSError, ValueError) as exc:
        print(f"[ERROR] {exc}", file=sys.stderr)
        return 1

    print(
        f"[OK] Updated convention README index: {readme_path().relative_to(project_root())}"
    )
    return 0


def build_template(title: str, summary: str | None) -> str:
    summary_text = summary or "この規約の目的と適用範囲をここに書く。"
    return (
        f"# {title}\n\n"
        "## Purpose\n\n"
        f"{summary_text}\n\n"
        "## Scope\n\n"
        "- Applies to: `TODO:` この規約が適用されるレイヤ、機能、ファイル、状況を書く\n"
        "- Does not apply to: `TODO:` 適用外や境界条件を書く\n\n"
        "## Rules\n\n"
        "- `TODO:` 守るべきルールを書く\n"
        "- `TODO:` 禁止事項や避ける実装を書く\n"
        "- `TODO:` 境界での変換、命名、エラー処理など具体条件を書く\n\n"
        "## Examples\n\n"
        "- Good: `TODO:` 推奨される実装例を書く\n"
        "- Bad: `TODO:` 避けるべき実装例を書く\n\n"
        "## Exceptions\n\n"
        "- `TODO:` 例外を認める条件、承認方法、記録方法を書く\n\n"
        "## Review Checklist\n\n"
        "- `TODO:` レビュー時に確認する観点を書く\n\n"
        "## Related Documents\n\n"
        "- `TODO:` 関連する spec / plan / external guide / rule を書く\n"
    )


def resolve_slug(name: str, provided_slug: str | None) -> str:
    if provided_slug is not None:
        return validate_slug(provided_slug)

    derived = slugify(name)
    if derived:
        return derived

    raise ValueError(
        "non-ASCII or free-form names require --slug with a kebab-case ASCII file name"
    )


def resolve_title(name: str, slug: str, provided_title: str | None) -> str:
    if provided_title is not None:
        return provided_title

    if slug == name:
        return default_title(slug)

    return name.strip()


def add_document(
    name: str, title: str | None, summary: str | None, slug: str | None
) -> int:
    try:
        resolved_slug = resolve_slug(name, slug)
        resolved_title = resolve_title(name, resolved_slug, title)
        ensure_readme_markers(load_readme_content())
    except ValueError as exc:
        print(f"[ERROR] {exc}", file=sys.stderr)
        return 1

    path = conventions_dir() / f"{resolved_slug}.md"
    if path.exists():
        print(
            f"[ERROR] Convention document already exists: {path.relative_to(project_root())}",
            file=sys.stderr,
        )
        return 1

    conventions_dir().mkdir(parents=True, exist_ok=True)
    path.write_text(build_template(resolved_title, summary), encoding="utf-8")
    update_readme_index()

    relative = path.relative_to(project_root())
    print(f"[OK] Added convention document: {relative}")
    print("Updated:")
    print(f"- {readme_path().relative_to(project_root())}")
    print(f"- {relative}")
    return 0


def verify_index() -> int:
    try:
        content = load_readme_content()
        ensure_readme_markers(content)
    except (OSError, ValueError) as exc:
        print(f"[ERROR] {exc}", file=sys.stderr)
        return 1

    match = re.search(
        rf"{re.escape(INDEX_START)}.*?{re.escape(INDEX_END)}", content, re.DOTALL
    )
    if match is None:
        print(
            f"[ERROR] README index markers not found in {readme_path()}",
            file=sys.stderr,
        )
        return 1

    expected = render_index_block()
    actual = match.group(0)
    if actual != expected:
        print("[ERROR] Convention README index is out of sync.", file=sys.stderr)
        print("To fix: run `cargo make conventions-update-index`.", file=sys.stderr)
        print(
            "If you just added a convention document, re-run `cargo make conventions-add ...`.",
            file=sys.stderr,
        )
        return 1
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Manage project convention documents.")
    subparsers = parser.add_subparsers(dest="command")

    add_parser = subparsers.add_parser(
        "add", help="Add a convention document and update the README index."
    )
    add_parser.add_argument(
        "name", help="Convention name or title. Free-form names may require --slug."
    )
    add_parser.add_argument(
        "--slug",
        help="ASCII kebab-case file name to use under project-docs/conventions/.",
    )
    add_parser.add_argument(
        "--title",
        help="Document title. Defaults to the free-form name or a titleized slug.",
    )
    add_parser.add_argument(
        "--summary", help="One-line purpose text to prefill the document."
    )

    subparsers.add_parser(
        "update-index",
        help="Regenerate README.md index from current convention documents.",
    )
    subparsers.add_parser(
        "verify-index", help="Verify that README.md indexes all convention documents."
    )
    return parser


def main(argv: list[str]) -> int:
    parser = build_parser()
    cli_args = argv[1:]
    # `cargo make <task> -- ...` may pass `--` between subcommand and its args.
    if len(cli_args) >= 2 and cli_args[1] == "--":
        cli_args = [cli_args[0], *cli_args[2:]]
    args = parser.parse_args(cli_args)

    if args.command == "add":
        return add_document(args.name, args.title, args.summary, args.slug)
    if args.command == "update-index":
        return update_index()
    if args.command == "verify-index":
        return verify_index()

    parser.print_help()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
