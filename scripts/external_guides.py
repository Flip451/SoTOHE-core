#!/usr/bin/env python3
"""
Manage external long-form guide metadata and local cache.
"""

from __future__ import annotations

import json
import re
import sys
import time as time_mod
import urllib.error
import urllib.request
from datetime import UTC, datetime, time
from pathlib import Path
from urllib.parse import urlparse

try:
    from scripts.track_resolution import latest_legacy_track_dir
    from scripts.verify_latest_track_files import (
        latest_track_dir as latest_verified_track_dir,
    )
except ImportError:  # pragma: no cover - script execution path
    from track_resolution import latest_legacy_track_dir
    from verify_latest_track_files import latest_track_dir as latest_verified_track_dir

MAX_DOWNLOAD_BYTES = 2 * 1024 * 1024
USER_AGENT = "TaktAgent/1.0 (+https://github.com/anthropics)"
FETCH_MAX_RETRIES = 3
FETCH_BACKOFF_BASE = 1.0


def project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def registry_path() -> Path:
    return project_root() / "docs" / "external-guides.json"


def load_registry() -> dict:
    with open(registry_path(), encoding="utf-8") as handle:
        return json.load(handle)


def save_registry(registry: dict) -> None:
    from atomic_write import atomic_write_file

    content = json.dumps(registry, ensure_ascii=False, indent=2) + "\n"
    atomic_write_file(registry_path(), content)


def cache_abspath(guide: dict) -> Path:
    return validate_cache_path(guide["cache_path"])


def read_response_body(response, max_bytes: int = MAX_DOWNLOAD_BYTES) -> bytes:
    body = response.read(max_bytes + 1)
    if len(body) > max_bytes:
        raise ValueError(f"response exceeded download limit ({max_bytes} bytes)")
    return body


def is_supported_remote_url(url: str) -> bool:
    return urlparse(url).scheme == "https"


def _is_retryable_error(err: urllib.error.URLError) -> bool:
    """Return True for transient HTTP errors worth retrying."""
    if isinstance(err, urllib.error.HTTPError):
        return err.code in (429, 500, 502, 503, 504)
    return True  # Network-level errors (DNS, timeout) are retryable


def fetch_with_retry(
    url: str,
    *,
    max_retries: int = FETCH_MAX_RETRIES,
    backoff_base: float = FETCH_BACKOFF_BASE,
) -> bytes:
    """Fetch a URL with bounded exponential backoff for transient errors."""
    if max_retries < 1:
        raise ValueError(f"max_retries must be >= 1, got {max_retries}")
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    last_error: Exception | None = None
    for attempt in range(max_retries):
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                return read_response_body(response)
        except urllib.error.URLError as err:
            last_error = err
            if not _is_retryable_error(err):
                raise
            if attempt < max_retries - 1:
                delay = backoff_base * (2**attempt)
                time_mod.sleep(delay)
    raise last_error  # type: ignore[misc]


def derive_raw_url(source_url: str) -> str:
    marker = "github.com/"
    blob = "/blob/"
    if marker in source_url and blob in source_url:
        # Strip query string and fragment before converting so they don't appear
        # in the raw URL (e.g. ?plain=1 or #L10-L20 from GitHub blob links).
        # Re-check after stripping in case marker/blob were only in the query.
        parsed = urlparse(source_url)
        clean_url = parsed._replace(query="", fragment="").geturl()
        if marker in clean_url and blob in clean_url:
            prefix, rest = clean_url.split(marker, 1)
            repo_path = rest.split(blob, 1)
            if len(repo_path) == 2:
                repo, tail = repo_path
                return f"{prefix}raw.githubusercontent.com/{repo}/{tail}"
    return source_url


def derive_cache_path(guide_id: str, source_url: str) -> str:
    suffix = Path(urlparse(source_url).path).suffix or ".md"
    return f".cache/external-guides/{guide_id}{suffix}"


def validate_cache_path(cache_path: str) -> Path:
    cache_root = (project_root() / ".cache" / "external-guides").resolve()
    candidate = Path(cache_path)
    if candidate.is_absolute():
        raise ValueError("cache_path must be relative to .cache/external-guides")

    resolved = (project_root() / candidate).resolve()
    resolved.relative_to(cache_root)
    return resolved


def trigger_matches(prompt_lower: str, trigger: str) -> bool:
    if re.search(r"[a-z0-9]", trigger):
        pattern = rf"(?<![a-z0-9_]){re.escape(trigger.lower())}(?![a-z0-9_])"
        return re.search(pattern, prompt_lower) is not None
    return trigger.lower() in prompt_lower


def _parse_updated_at(raw_value: str) -> datetime:
    value = raw_value.strip()
    if not value:
        raise ValueError("updated_at must be a non-empty string")
    if value.endswith("Z"):
        value = value[:-1] + "+00:00"
    try:
        parsed = datetime.fromisoformat(value)
    except ValueError:
        parsed = datetime.combine(
            datetime.fromisoformat(value + "T00:00:00").date(), time.min
        )
    if parsed.tzinfo is None:
        return parsed.replace(tzinfo=UTC)
    return parsed.astimezone(UTC)


def _track_updated_at(track_dir: Path) -> datetime:
    """Return the updated_at datetime from metadata.json, or datetime.min on any failure."""
    epoch = datetime.min.replace(tzinfo=UTC)
    metadata_file = track_dir / "metadata.json"
    if not metadata_file.is_file():
        return epoch
    try:
        data = json.loads(metadata_file.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError, UnicodeDecodeError):
        return epoch
    if not isinstance(data, dict):
        return epoch
    raw = data.get("updated_at")
    if not isinstance(raw, str):
        return epoch
    try:
        return _parse_updated_at(raw)
    except (ValueError, OverflowError):
        return epoch


def latest_track_dir(root: Path | None = None) -> Path | None:
    repo_root = root or project_root()
    latest_dir, _warnings = latest_legacy_track_dir(repo_root)
    if latest_dir is not None:
        return latest_dir
    latest_dir, errors = latest_verified_track_dir(repo_root)
    if errors:
        return None
    return latest_dir


def latest_track_context(root: Path | None = None) -> str:
    track_dir = latest_track_dir(root)
    if track_dir is None:
        return ""

    parts: list[str] = []
    for name in ("spec.md", "plan.md"):
        path = track_dir / name
        if path.exists():
            parts.append(path.read_text(encoding="utf-8"))
    return "\n".join(parts)


def find_relevant_guides(
    prompt: str, registry: dict | None = None, limit: int = 3
) -> list[tuple[dict, str]]:
    prompt_lower = prompt.lower()
    active_registry = registry if registry is not None else load_registry()
    matches: list[tuple[dict, str]] = []

    for guide in active_registry.get("guides", []):
        for trigger in guide.get("trigger_keywords", []):
            if trigger_matches(prompt_lower, trigger):
                matches.append((guide, trigger))
                break

    return matches[:limit]


def find_relevant_guides_for_track_workflow(
    prompt: str,
    registry: dict | None = None,
    track_context: str | None = None,
    limit: int = 3,
) -> list[tuple[dict, str]]:
    """
    Match guide triggers against track workflow context.

    `track_context=None` means "load the latest track context from disk".
    `track_context=""` means "skip track file context and use only the prompt".
    """
    active_registry = registry if registry is not None else load_registry()
    context_parts = [prompt]
    if track_context is None:
        context_parts.append(latest_track_context())
    elif track_context:
        context_parts.append(track_context)

    combined_context = "\n".join(part for part in context_parts if part)
    return find_relevant_guides(combined_context, active_registry, limit=limit)


def strip_make_separator(args: list[str]) -> list[str]:
    # `cargo make <task> -- ...` forwards a literal `--` token. Ignore it.
    if args and args[0] == "--":
        return args[1:]
    return args


def add_guide(registry: dict, argv: list[str]) -> int:
    args = strip_make_separator(argv[2:])
    entry = {
        "trigger_keywords": [],
        "summary": [],
        "project_usage": [],
    }
    current_key: str | None = None

    flag_map = {
        "--id": "id",
        "--title": "title",
        "--source-url": "source_url",
        "--raw-url": "raw_url",
        "--license": "license",
        "--cache-path": "cache_path",
        "--trigger": "trigger_keywords",
        "--summary": "summary",
        "--project-usage": "project_usage",
    }

    for token in args:
        if token in flag_map:
            current_key = flag_map[token]
            if current_key in {"trigger_keywords", "summary", "project_usage"}:
                continue
            entry.setdefault(current_key, "")
            continue

        if current_key is None:
            print(f"[ERROR] Unexpected argument: {token}", file=sys.stderr)
            return 1

        if current_key in {"trigger_keywords", "summary", "project_usage"}:
            entry[current_key].append(token)
        else:
            entry[current_key] = token
            current_key = None

    required = ["id", "title", "source_url", "license"]
    missing = [name for name in required if not entry.get(name)]
    if missing:
        print(f"[ERROR] Missing required fields: {', '.join(missing)}", file=sys.stderr)
        return 1

    if any(guide["id"] == entry["id"] for guide in registry["guides"]):
        print(f"[ERROR] Guide id already exists: {entry['id']}", file=sys.stderr)
        return 1

    entry["raw_url"] = entry.get("raw_url") or derive_raw_url(entry["source_url"])
    entry["cache_path"] = entry.get("cache_path") or derive_cache_path(
        entry["id"], entry["source_url"]
    )

    invalid_urls = [
        name
        for name in ("source_url", "raw_url")
        if not is_supported_remote_url(entry[name])
    ]
    if invalid_urls:
        print(
            f"[ERROR] Unsupported URL scheme for: {', '.join(invalid_urls)}. Only https URLs are allowed.",
            file=sys.stderr,
        )
        return 1

    try:
        validate_cache_path(entry["cache_path"])
    except ValueError as err:
        print(f"[ERROR] Invalid cache_path: {err}", file=sys.stderr)
        return 1

    registry["guides"].append(
        {
            "id": entry["id"],
            "title": entry["title"],
            "source_url": entry["source_url"],
            "raw_url": entry["raw_url"],
            "license": entry["license"],
            "cache_path": entry["cache_path"],
            "trigger_keywords": entry["trigger_keywords"],
            "summary": entry["summary"],
            "project_usage": entry["project_usage"],
        }
    )
    registry["guides"].sort(key=lambda guide: guide["id"])
    save_registry(registry)

    print(f"[OK] Added guide entry: {entry['id']}")
    print("Next steps:")
    print("1. cargo make guides-list")
    print(f"2. cargo make guides-fetch {entry['id']}")
    return 0


def list_guides(registry: dict) -> int:
    print("External Guide Registry")
    print("=======================")
    for guide in registry["guides"]:
        try:
            status = "cached" if cache_abspath(guide).exists() else "missing"
        except ValueError:
            status = "invalid-cache-path"
        print(f"- {guide['id']}")
        print(f"  title: {guide['title']}")
        print(f"  status: {status}")
        print(f"  license: {guide['license']}")
        print(f"  source: {guide['source_url']}")
        print(f"  cache: {guide['cache_path']}")
        print(f"  triggers: {', '.join(guide['trigger_keywords'])}")
    return 0


def fetch_guides(registry: dict, ids: list[str]) -> int:
    guides = {guide["id"]: guide for guide in registry["guides"]}
    selected: list[dict]
    if ids:
        unknown = [guide_id for guide_id in ids if guide_id not in guides]
        if unknown:
            print(f"[ERROR] Unknown guide ids: {', '.join(unknown)}", file=sys.stderr)
            return 1
        selected = [guides[guide_id] for guide_id in ids]
    else:
        selected = registry["guides"]

    failed = 0
    for guide in selected:
        if not is_supported_remote_url(guide["raw_url"]):
            print(
                f"[ERROR] Unsupported URL scheme for {guide['id']}: {guide['raw_url']}",
                file=sys.stderr,
            )
            failed = 1
            continue
        try:
            target = cache_abspath(guide)
        except ValueError as err:
            print(
                f"[ERROR] Invalid cache_path for {guide['id']}: {err}", file=sys.stderr
            )
            failed = 1
            continue
        try:
            body = fetch_with_retry(guide["raw_url"])
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(body)
            print(f"[OK] Cached {guide['id']} -> {guide['cache_path']}")
        except (urllib.error.URLError, OSError, ValueError) as err:
            print(f"[ERROR] Failed to fetch {guide['id']}: {err}", file=sys.stderr)
            failed = 1
    return failed


def clean_guides(registry: dict, ids: list[str]) -> int:
    guides = {guide["id"]: guide for guide in registry["guides"]}
    selected: list[dict]
    if ids:
        unknown = [guide_id for guide_id in ids if guide_id not in guides]
        if unknown:
            print(f"[ERROR] Unknown guide ids: {', '.join(unknown)}", file=sys.stderr)
            return 1
        selected = [guides[guide_id] for guide_id in ids]
    else:
        selected = registry["guides"]

    removed = 0
    for guide in selected:
        try:
            target = cache_abspath(guide)
        except ValueError as err:
            print(
                f"[WARN] Invalid cache_path for {guide['id']}: {err}", file=sys.stderr
            )
            continue
        if target.exists():
            try:
                target.unlink()
            except OSError as err:
                print(
                    f"[WARN] Could not remove cache for {guide['id']}: {err}",
                    file=sys.stderr,
                )
                continue
            print(f"[OK] Removed cache for {guide['id']}: {guide['cache_path']}")
            removed += 1
        else:
            print(f"[SKIP] No cache file for {guide['id']}")
    print(f"\nRemoved {removed} cached file(s).")
    return 0


def show_usage(registry: dict) -> int:
    policy = registry["usage_policy"]
    print("Usage Policy")
    print("============")
    print(policy["goal"])
    print()
    print("Read order:")
    for item in policy["read_order"]:
        print(f"- {item}")
    print()
    print("Copyright notes:")
    for item in policy["copyright_notes"]:
        print(f"- {item}")
    return 0


def show_setup(registry: dict) -> int:
    print("External Guide Setup")
    print("====================")
    print("1. List available guides")
    print("   cargo make guides-list")
    print()
    print("2. Review the minimal-context usage policy")
    print("   cargo make guides-usage")
    print()
    print("3. Fetch only the guide you need into local cache")
    print("   cargo make guides-fetch <guide-id>")
    print()
    print("4. Re-run the list command to confirm cache status")
    print("   cargo make guides-list")
    print()
    print("Current guide ids:")
    for guide in registry["guides"]:
        print(f"- {guide['id']}")
    return 0


def show_help() -> int:
    print("Usage:")
    print("  cargo make guides-list")
    print("  cargo make guides-fetch <guide-id ...>")
    print("  cargo make guides-clean [<guide-id ...>]")
    print("  cargo make guides-usage")
    print("  cargo make guides-setup")
    print(
        "  cargo make guides-add -- --id <id> --title <title> --source-url <url> --license <license> [--raw-url <url>] [--cache-path <path>] [--trigger <kw>]... [--summary <text>]... [--project-usage <text>]..."
    )
    print()
    print("Example:")
    print(
        '  cargo make guides-add -- --id pg-guide --title "PostgreSQL Guide" --source-url "https://github.com/example/repo/blob/main/docs/postgres.md" --license "CC-BY-4.0" --trigger postgres --summary "Use for schema review" --project-usage "Check before changing SQL conventions"'
    )
    return 0


def main(argv: list[str]) -> int:
    command = argv[1] if len(argv) > 1 else "help"
    command_args = strip_make_separator(argv[2:])

    if command in {"help", "--help", "-h"}:
        return show_help()

    registry = load_registry()

    if command == "list":
        return list_guides(registry)
    if command == "fetch":
        return fetch_guides(registry, command_args)
    if command == "clean":
        return clean_guides(registry, command_args)
    if command == "add":
        return add_guide(registry, argv)
    if command == "usage":
        return show_usage(registry)
    if command == "setup":
        return show_setup(registry)

    print(f"[ERROR] Unknown command: {command}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
