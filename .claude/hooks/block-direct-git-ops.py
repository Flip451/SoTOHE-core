#!/usr/bin/env python3
"""
PreToolUse hook: Block direct git add/commit/push and git branch delete commands.
"""

import os
import re
import shlex
import sys

from _shared import load_stdin_json, print_hook_error, tool_input

GIT_POLICY_PREFIX = "[Git Policy]"
GIT_COMMIT_MESSAGE = (
    f"{GIT_POLICY_PREFIX} Direct `git commit` is blocked.\n"
    "Use `/track:commit`, or write the message to `tmp/track-commit/commit-message.txt` "
    "and run `cargo make track-commit-message`."
)
GIT_ADD_MESSAGE = (
    f"{GIT_POLICY_PREFIX} Direct `git add` is blocked.\n"
    "For selective staging, write repo-relative paths to `tmp/track-commit/add-paths.txt` "
    "and run `cargo make track-add-paths`. Use `cargo make add-all` only when staging the whole worktree is intended."
)
GIT_BRANCH_DELETE_MESSAGE = (
    f"{GIT_POLICY_PREFIX} Direct `git branch -d/-D/--delete` is blocked.\n"
    "Branch create/rename is allowed, but branch deletion must be done manually by the user."
)
GIT_PUSH_MESSAGE = (
    f"{GIT_POLICY_PREFIX} Direct `git push` is blocked.\n"
    "Pushing must be done manually by the user, not by AI agents."
)
SHELL_COMMAND_FLAGS = {"-c", "-lc"}
KNOWN_SHELLS = {"bash", "sh", "zsh", "dash", "ksh"}
KNOWN_PYTHONS = {"python", "python3"}
# Common command launchers that pass remaining args as a command to execute.
_COMMAND_LAUNCHERS = {
    "nohup",
    "nice",
    "timeout",
    "stdbuf",
    "setsid",
    "chronic",
    "ionice",
    "chrt",
    "taskset",
    "command",
    "time",
    "exec",
}
# Launchers that take a mandatory positional argument before the command.
# timeout DURATION CMD..., chrt PRIORITY CMD..., taskset MASK CMD..., ionice CMD...
_LAUNCHER_POSITIONAL_ARGS = {"timeout": 1, "chrt": 1, "taskset": 1}
_LAUNCHER_OPTIONS_WITH_ARG = {
    # nice / ionice (-n is shared)
    "-n",
    "--adjustment",
    # timeout
    "-k",
    "--kill-after",
    "-s",
    "--signal",
    # stdbuf
    "-i",
    "-o",
    "-e",
    # ionice
    "-c",
    # chrt / taskset (-p is shared)
    "-p",
    # /usr/bin/time
    "-f",
    "--format",
    "--output",
    # exec (bash builtin)
    "-a",
}
_FIND_EXEC_FLAGS = {"-exec", "-execdir"}
_XARGS_OPTIONS_NO_ARG = {
    "-0",
    "--null",
    "-r",
    "--no-run-if-empty",
    "-t",
    "--verbose",
    "-p",
    "--interactive",
    "-x",
    "--exit",
}
_XARGS_OPTIONS_WITH_ARG = {
    "-L",
    "-n",
    "-P",
    "-I",
    "-E",
    "-s",
    "-a",
    "-d",
    "--max-lines",
    "--max-args",
    "--max-procs",
    "--replace",
    "--eof",
    "--max-chars",
    "--arg-file",
    "--delimiter",
}
GIT_VARIABLE_BYPASS_MESSAGE = (
    f"{GIT_POLICY_PREFIX} Shell variable or command substitution in command position is blocked.\n"
    "Potential bypass of git operation guardrails detected. Use literal `git` commands through "
    "approved cargo-make wrappers."
)
_PROTECTED_GIT_SUBCOMMANDS = {"add", "commit", "push"}
# Regex fallback for when bashlex is unavailable.
# Match expansion tokens ($VAR, ${VAR}, $(cmd), `cmd`) followed by protected git subcommands.
# Anchored to command position: start of string or after shell control operators (;, &&, ||, |).
# Allows optional prefixes like `env`, `VAR=val`, or combinations thereof.
_VARIABLE_BYPASS_FALLBACK_RE = re.compile(
    r"(?:^|(?:&&|\|\||[;|])\s*)"
    r"""(?:(?:env\s+|[A-Za-z_][A-Za-z0-9_]*=(?:[^\s]*|"[^"]*"|'[^']*')\s+)*)"""
    r"""(?:"?\$\([^)]*\)"?|"?`[^`]*`"?|"?\$[A-Za-z_{][^\s"]*"?)"""
    r"""\s+"?(add|commit|push)\b"?"""
)

# bashlex (GPLv3+) is optional — install manually for higher-accuracy AST detection.
# Without it, the regex fallback above is used instead.
try:
    import bashlex as _bashlex
except ImportError:
    _bashlex = None  # type: ignore[assignment]


def _check_variable_git_bypass(command: str) -> bool:
    """Return True if a shell expansion in command position is followed by a protected git subcommand.

    Uses bashlex AST when available; falls back to regex heuristic.
    """
    if _bashlex is not None:
        return _check_variable_git_bypass_ast(command)
    return _check_variable_git_bypass_regex(command)


def _check_variable_git_bypass_ast(command: str) -> bool:
    """AST-based detection using bashlex."""
    try:
        parts = _bashlex.parse(command)
    except Exception:
        # Parse failure on a command with expansion markers is suspicious — block it.
        return bool(re.search(r"\$[A-Za-z_{(]|`", command))

    for node in parts:
        if node.kind == "command":
            words = [p for p in node.parts if p.kind == "word"]
            if not words:
                continue
            # Find the command-position word (skip assignments like VAR=val)
            cmd_word = None
            remaining_words: list[str] = []
            for i, w in enumerate(words):
                if "=" in w.word and not w.word.startswith("="):
                    # Looks like VAR=val assignment — skip
                    continue
                cmd_word = w
                remaining_words = [rw.word.lower() for rw in words[i + 1 :]]
                break
            if cmd_word is None:
                continue

            def _word_has_expansion(word_node) -> bool:
                return hasattr(word_node, "parts") and any(
                    p.kind in ("parameter", "commandsubstitution")
                    for p in word_node.parts
                )

            # Check if the command word has expansion sub-parts
            if _word_has_expansion(cmd_word) and any(
                rw in _PROTECTED_GIT_SUBCOMMANDS for rw in remaining_words
            ):
                return True

            # Handle `env $VAR subcommand` — if cmd is `env`, scan remaining words
            if cmd_word.word.lower() == "env":
                for j, rw in enumerate(words[words.index(cmd_word) + 1 :]):
                    if _word_has_expansion(rw) and any(
                        w2.word.lower() in _PROTECTED_GIT_SUBCOMMANDS
                        for w2 in words[words.index(cmd_word) + 2 + j :]
                    ):
                        return True
        elif node.kind == "list":
            # Recurse into list/pipeline parts
            for part in node.parts:
                cmd_str = command[part.pos[0] : part.pos[1]]
                if _check_variable_git_bypass_ast(cmd_str):
                    return True
    return False


def _check_variable_git_bypass_regex(command: str) -> bool:
    """Regex fallback when bashlex is not available."""
    return bool(_VARIABLE_BYPASS_FALLBACK_RE.search(command))


SHELL_CONTROL_OPERATORS = {"&&", "||", ";", "&", "|", "(", ")", "\n"}
PYTHON_GIT_COMMAND_PATTERNS = (
    re.compile(r"\bgit\s+(add|commit|push)\b", re.IGNORECASE),
    re.compile(r"\bgit\s+branch\s+(-d|-D|--delete)\b", re.IGNORECASE),
    # Bare name: subprocess.run(["git", "commit", ...])
    re.compile(r"['\"]git['\"]\s*,\s*['\"](add|commit|push)['\"]", re.IGNORECASE),
    # Absolute/relative path: subprocess.run(["/usr/bin/git", "commit", ...])
    re.compile(
        r"['\"][^'\"]*[/\\]git['\"]\s*,\s*['\"](add|commit|push)['\"]", re.IGNORECASE
    ),
    # List-form branch delete: ["git", "branch", "-D", ...]
    re.compile(
        r"['\"]git['\"]\s*,\s*['\"]branch['\"]\s*,\s*['\"](-d|-D|--delete)['\"]",
        re.IGNORECASE,
    ),
    # Absolute path branch delete: ["/usr/bin/git", "branch", "-D", ...]
    re.compile(
        r"['\"][^'\"]*[/\\]git['\"]\s*,\s*['\"]branch['\"]\s*,\s*['\"](-d|-D|--delete)['\"]",
        re.IGNORECASE,
    ),
)


def _tokenize(command: str) -> list[str]:
    try:
        return shlex.split(command, posix=True)
    except ValueError:
        return []


def _split_shell_control_segments(command: str) -> list[str]:
    segments: list[str] = []
    current: list[str] = []
    saw_control_operator = False
    quote_char = ""
    escape_next = False
    index = 0

    while index < len(command):
        char = command[index]

        if escape_next:
            current.append(char)
            escape_next = False
            index += 1
            continue

        if char == "\\" and quote_char != "'":
            current.append(char)
            escape_next = True
            index += 1
            continue

        if quote_char:
            current.append(char)
            if char == quote_char:
                quote_char = ""
            index += 1
            continue

        if char in {"'", '"'}:
            quote_char = char
            current.append(char)
            index += 1
            continue

        # Skip $(...) command substitution — parentheses inside are not control operators.
        if command.startswith("$(", index) and not command.startswith("$((", index):
            nested, next_index = _extract_balanced_command_substitution(command, index)
            if nested is not None:
                current.append(command[index:next_index])
                index = next_index
                continue

        # Skip backtick command substitution.
        if char == "`":
            nested, next_index = _extract_backtick_command_substitution(command, index)
            if nested is not None:
                current.append(command[index:next_index])
                index = next_index
                continue

        operator = ""
        if command.startswith("&&", index):
            operator = "&&"
        elif command.startswith("||", index):
            operator = "||"
        elif char in {";", "&", "|", "(", ")", "\n"}:
            operator = char

        if operator:
            saw_control_operator = True
            segment = "".join(current).strip()
            if segment:
                segments.append(segment)
            current = []
            index += len(operator)
            continue

        current.append(char)
        index += 1

    if current:
        segment = "".join(current).strip()
        if segment:
            segments.append(segment)

    if not saw_control_operator:
        return [command]

    return segments or ([command] if command.strip() else [])


def _extract_balanced_command_substitution(
    command: str, start: int
) -> tuple[str | None, int]:
    if not command.startswith("$(", start) or command.startswith("$((", start):
        return None, start

    depth = 1
    index = start + 2
    current: list[str] = []
    quote_char = ""
    escape_next = False

    while index < len(command):
        char = command[index]

        if escape_next:
            current.append(char)
            escape_next = False
            index += 1
            continue

        if char == "\\" and quote_char != "'":
            current.append(char)
            escape_next = True
            index += 1
            continue

        if quote_char == "'":
            current.append(char)
            if char == "'":
                quote_char = ""
            index += 1
            continue

        if char == '"':
            current.append(char)
            quote_char = "" if quote_char == '"' else '"'
            index += 1
            continue

        if char == "'" and not quote_char:
            current.append(char)
            quote_char = "'"
            index += 1
            continue

        if char == "`":
            current.append(char)
            nested, next_index = _extract_backtick_command_substitution(command, index)
            if nested is not None:
                current.append(nested)
                current.append("`")
                index = next_index
                continue
            index += 1
            continue

        if command.startswith("$(", index) and not command.startswith("$((", index):
            nested, next_index = _extract_balanced_command_substitution(command, index)
            if nested is None:
                return None, start
            current.append("$(")
            current.append(nested)
            current.append(")")
            index = next_index
            continue

        if char == "(":
            depth += 1
            current.append(char)
            index += 1
            continue

        if char == ")":
            depth -= 1
            if depth == 0:
                return "".join(current), index + 1
            current.append(char)
            index += 1
            continue

        current.append(char)
        index += 1

    return None, start


def _extract_backtick_command_substitution(
    command: str, start: int
) -> tuple[str | None, int]:
    if start >= len(command) or command[start] != "`":
        return None, start

    index = start + 1
    current: list[str] = []
    escape_next = False

    while index < len(command):
        char = command[index]

        if escape_next:
            current.append(char)
            escape_next = False
            index += 1
            continue

        if char == "\\":
            current.append(char)
            escape_next = True
            index += 1
            continue

        if char == "`":
            return "".join(current), index + 1

        current.append(char)
        index += 1

    return None, start


def _nested_command_substitutions(command: str) -> list[str]:
    nested: list[str] = []
    quote_char = ""
    escape_next = False
    index = 0

    while index < len(command):
        char = command[index]

        if escape_next:
            escape_next = False
            index += 1
            continue

        if char == "\\" and quote_char != "'":
            escape_next = True
            index += 1
            continue

        if quote_char == "'":
            if char == "'":
                quote_char = ""
            index += 1
            continue

        if char == "'" and not quote_char:
            quote_char = "'"
            index += 1
            continue

        if char == '"' and quote_char != "'":
            quote_char = "" if quote_char == '"' else '"'
            index += 1
            continue

        if char == "`":
            nested_command, next_index = _extract_backtick_command_substitution(
                command, index
            )
            if nested_command is not None:
                nested.append(nested_command)
                index = next_index
                continue

        if (
            command.startswith("$(", index)
            and quote_char != "'"
            and not command.startswith("$((", index)
        ):
            nested_command, next_index = _extract_balanced_command_substitution(
                command, index
            )
            if nested_command is not None:
                nested.append(nested_command)
                index = next_index
                continue

        index += 1

    return nested


# env options that consume the next token as their argument.
_ENV_OPTIONS_WITH_ARG = {"-u", "--unset", "-S", "--split-string", "-C", "--chdir"}


def _skip_env_prefix(tokens: list[str], start: int = 0) -> int:
    index = start
    # Skip inline VAR=val assignments before any command
    while index < len(tokens) and re.fullmatch(
        r"[A-Za-z_][A-Za-z0-9_]*=.*", tokens[index]
    ):
        index += 1

    if index < len(tokens) and os.path.basename(tokens[index]) == "env":
        index += 1
        # Skip env options and their arguments
        while index < len(tokens):
            token = tokens[index]
            if token in {"--"}:
                index += 1
                break
            if token in _ENV_OPTIONS_WITH_ARG:
                index += 2
                continue
            if token in {"-i", "--ignore-environment", "-0", "--null"}:
                index += 1
                continue
            if token.startswith("-") and not re.fullmatch(
                r"[A-Za-z_][A-Za-z0-9_]*=.*", token
            ):
                # Combined short flags may end with an option that takes an argument
                # e.g. -iC means -i + -C (chdir), so the next token is the arg for -C
                if len(token) > 2 and token[-1] in {"C", "u", "S"}:
                    index += 2  # skip this token + its argument
                else:
                    index += 1
                continue
            # VAR=val after env
            if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*=.*", token):
                index += 1
                continue
            break

    return index


def _skip_command_launcher(tokens: list[str], start: int) -> int:
    """Skip known command launchers (nohup, nice, timeout, etc.) and their options."""
    index = start
    while (
        index < len(tokens)
        and os.path.basename(tokens[index]).lower() in _COMMAND_LAUNCHERS
    ):
        launcher = os.path.basename(tokens[index]).lower()
        index += 1
        # Skip launcher options
        while index < len(tokens) and tokens[index].startswith("-"):
            # time's -p is a no-arg flag (portable output), unlike chrt/taskset
            if tokens[index] == "-p" and launcher == "time":
                index += 1
            elif tokens[index] in _LAUNCHER_OPTIONS_WITH_ARG:
                index += 2
            else:
                index += 1
        # Skip mandatory positional arguments (e.g. timeout DURATION)
        positional_count = _LAUNCHER_POSITIONAL_ARGS.get(launcher, 0)
        index += positional_count
        # After launcher, skip env prefix if present
        index = _skip_env_prefix(tokens, index)
    return index


def _is_python_binary(token: str) -> bool:
    basename = os.path.basename(token).lower()
    return basename in KNOWN_PYTHONS or basename.startswith("python")


# Shell options that consume the next token as their argument.
_SHELL_OPTIONS_WITH_ARG = {"-O", "+O", "-o", "+o"}


def _nested_shell_commands(command: str) -> list[str]:
    tokens = _tokenize(command)
    if not tokens:
        return []

    nested: list[str] = []
    for index, token in enumerate(tokens[:-1]):
        if os.path.basename(token).lower() not in KNOWN_SHELLS:
            continue

        # Scan forward through shell options to find -c
        flag_index = index + 1
        while flag_index < len(tokens):
            flag = tokens[flag_index]
            is_c_flag = flag in SHELL_COMMAND_FLAGS or (
                flag.startswith("-") and not flag.startswith("--") and "c" in flag[1:]
            )
            if is_c_flag:
                if flag_index + 1 < len(tokens):
                    nested.append(tokens[flag_index + 1])
                break
            if flag == "--":
                break
            if flag in _SHELL_OPTIONS_WITH_ARG:
                flag_index += 2
                continue
            if flag.startswith("-") or flag.startswith("+"):
                flag_index += 1
                continue
            break

    return nested


def _nested_python_commands(command: str) -> list[str]:
    tokens = _tokenize(command)
    if not tokens:
        return []

    nested: list[str] = []
    for index, token in enumerate(tokens[:-1]):
        if not _is_python_binary(token):
            continue

        flag_index = index + 1
        while flag_index < len(tokens):
            flag = tokens[flag_index]
            if flag == "-c":
                if flag_index + 1 < len(tokens):
                    nested.extend(_python_git_commands(tokens[flag_index + 1]))
                break
            # Handle -c<code> concatenated form (e.g. -c'import os; ...')
            if flag.startswith("-c") and len(flag) > 2:
                nested.extend(_python_git_commands(flag[2:]))
                break
            if flag == "--":
                break
            if flag == "-m":
                break
            # Options that take a separate argument: -W, -X, -Q
            # Also handle -Wfoo concatenated form
            if flag in {"-W", "-X", "-Q"}:
                flag_index += 2
                continue
            if len(flag) > 2 and flag[:2] in {"-W", "-X", "-Q"}:
                flag_index += 1
                continue
            if flag.startswith("-"):
                flag_index += 1
                continue
            break

    return nested


_BRANCH_DELETE_FLAGS = {"-d", "--delete"}


def _python_git_commands(code: str) -> list[str]:
    nested: list[str] = []
    for pattern in PYTHON_GIT_COMMAND_PATTERNS:
        for match in pattern.finditer(code):
            if not match.lastindex or match.lastindex < 1:
                continue
            group1 = match.group(1)
            if group1.lower() in _BRANCH_DELETE_FLAGS:
                nested.append(f"git branch {group1}")
            else:
                nested.append(f"git {group1.lower()}")
    return nested


# git top-level options that consume the next token as their argument.
# Reference: git(1) — these flags take a mandatory <path>, <name>, or <value> argument.
_GIT_OPTIONS_WITH_ARG = {
    "-C",
    "-c",
    "--git-dir",
    "--work-tree",
    "--namespace",
    "--super-prefix",
    "--config-env",
    "--exec-path",
}


def _git_subcommand_from_tokens(tokens: list[str], start: int) -> str | None:
    index = start
    if index >= len(tokens) or os.path.basename(tokens[index]).lower() != "git":
        return None

    index += 1
    while index < len(tokens):
        token = tokens[index]
        if token == "--":
            index += 1
            break
        if token in _GIT_OPTIONS_WITH_ARG:
            index += 2  # skip option + its argument
            continue
        if token.startswith("-"):
            index += 1
            continue
        return token.lower()

    if index < len(tokens):
        return tokens[index].lower()
    return None


def _git_subcommand(command: str) -> str | None:
    tokens = _tokenize(command)
    if not tokens:
        return None

    direct_index = _skip_env_prefix(tokens)
    direct_index = _skip_command_launcher(tokens, direct_index)
    if (
        direct_index < len(tokens)
        and os.path.basename(tokens[direct_index]).lower() == "git"
    ):
        return _git_subcommand_from_tokens(tokens, direct_index)

    # Check for find -exec / -execdir patterns
    first_cmd = (
        os.path.basename(tokens[direct_index]).lower()
        if direct_index < len(tokens)
        else ""
    )
    if first_cmd == "find":
        for index in range(direct_index + 1, len(tokens)):
            if tokens[index] not in _FIND_EXEC_FLAGS:
                continue
            after_exec = _skip_env_prefix(tokens, index + 1)
            if (
                after_exec < len(tokens)
                and os.path.basename(tokens[after_exec]).lower() == "git"
            ):
                return _git_subcommand_from_tokens(tokens, after_exec)

    # Check for xargs [options] [env ...] git patterns
    if first_cmd == "xargs":
        index = direct_index + 1
        while index < len(tokens) and tokens[index].startswith("-"):
            if tokens[index] in _XARGS_OPTIONS_WITH_ARG:
                index += 2  # skip option + its argument
            else:
                index += 1
        after_xargs = _skip_env_prefix(tokens, index)
        if (
            after_xargs < len(tokens)
            and os.path.basename(tokens[after_xargs]).lower() == "git"
        ):
            return _git_subcommand_from_tokens(tokens, after_xargs)

    return None


def _is_git_branch_delete_from_tokens(tokens: list[str], start: int) -> bool:
    """Check if git branch delete starting from the git token at `start`."""
    if start >= len(tokens) or os.path.basename(tokens[start]).lower() != "git":
        return False

    index = start + 1
    while index < len(tokens):
        token = tokens[index]
        if token == "--":
            index += 1
            break
        if token in _GIT_OPTIONS_WITH_ARG:
            index += 2
            continue
        if token.startswith("-"):
            index += 1
            continue
        break

    if index >= len(tokens) or tokens[index].lower() != "branch":
        return False

    index += 1
    while index < len(tokens):
        token = tokens[index]
        if token in {"-d", "-D", "--delete"}:
            return True
        index += 1
    return False


def _is_git_branch_delete(command: str) -> bool:
    tokens = _tokenize(command)
    if not tokens:
        return False

    direct_index = _skip_env_prefix(tokens)
    direct_index = _skip_command_launcher(tokens, direct_index)

    # Direct git command
    if (
        direct_index < len(tokens)
        and os.path.basename(tokens[direct_index]).lower() == "git"
    ):
        return _is_git_branch_delete_from_tokens(tokens, direct_index)

    first_cmd = (
        os.path.basename(tokens[direct_index]).lower()
        if direct_index < len(tokens)
        else ""
    )

    # find -exec / -execdir
    if first_cmd == "find":
        for index in range(direct_index + 1, len(tokens)):
            if tokens[index] not in _FIND_EXEC_FLAGS:
                continue
            after_exec = _skip_env_prefix(tokens, index + 1)
            if (
                after_exec < len(tokens)
                and os.path.basename(tokens[after_exec]).lower() == "git"
            ):
                return _is_git_branch_delete_from_tokens(tokens, after_exec)

    # xargs [options] [env ...] git
    if first_cmd == "xargs":
        index = direct_index + 1
        while index < len(tokens) and tokens[index].startswith("-"):
            if tokens[index] in _XARGS_OPTIONS_WITH_ARG:
                index += 2
            else:
                index += 1
        after_xargs = _skip_env_prefix(tokens, index)
        if (
            after_xargs < len(tokens)
            and os.path.basename(tokens[after_xargs]).lower() == "git"
        ):
            return _is_git_branch_delete_from_tokens(tokens, after_xargs)

    return False


def check_command(command: str) -> tuple[bool, str]:
    if not command:
        return False, ""

    segments = _split_shell_control_segments(command)
    if len(segments) != 1 or (segments and segments[0] != command):
        for segment in segments:
            should_block, message = check_command(segment)
            if should_block:
                return True, message
        return False, ""

    for nested_command in _nested_command_substitutions(command):
        should_block, message = check_command(nested_command)
        if should_block:
            return True, message

    for nested_command in _nested_shell_commands(command):
        should_block, message = check_command(nested_command)
        if should_block:
            return True, message

    for nested_command in _nested_python_commands(command):
        should_block, message = check_command(nested_command)
        if should_block:
            return True, message

    # Check for shell variable/command substitution bypass before literal git detection.
    if _check_variable_git_bypass(command):
        return True, GIT_VARIABLE_BYPASS_MESSAGE

    subcommand = _git_subcommand(command)

    if subcommand == "commit":
        return True, GIT_COMMIT_MESSAGE

    if subcommand == "add":
        return True, GIT_ADD_MESSAGE

    if subcommand == "push":
        return True, GIT_PUSH_MESSAGE

    if subcommand == "branch" and _is_git_branch_delete(command):
        return True, GIT_BRANCH_DELETE_MESSAGE

    return False, ""


CLI_BINARY_VAR = "SOTP_CLI_BINARY"


def _cli_binary() -> str:
    return os.environ.get(CLI_BINARY_VAR, "sotp")


def _try_cli_guard(command: str) -> tuple[bool, str] | None:
    """Try to check the command via the Rust CLI guard.

    Returns (should_block, message) on success, or None if CLI is unavailable.
    """
    import subprocess as _subprocess

    cli = _cli_binary()
    try:
        result = _subprocess.run(
            [cli, "guard", "check", "--command", command],
            capture_output=True,
            text=True,
            timeout=5,
        )
    except FileNotFoundError:
        return None  # CLI binary not found — fall back to Python
    except _subprocess.TimeoutExpired:
        return None  # CLI hung — fall back to Python
    except Exception:
        return None  # Any other error — fall back to Python

    try:
        import json as _json

        verdict = _json.loads(result.stdout)
    except Exception:
        return None  # Malformed output — fall back to Python

    if verdict.get("decision") == "block":
        reason = verdict.get("reason", "blocked by guard CLI")
        return True, reason
    return False, ""


def main() -> None:
    try:
        data = load_stdin_json()
        if data.get("tool_name", "") != "Bash":
            sys.exit(0)

        command = tool_input(data).get("command", "")

        # Try Rust CLI first; fall back to Python check_command if unavailable
        cli_result = _try_cli_guard(command)
        if cli_result is not None:
            should_block, message = cli_result
        else:
            should_block, message = check_command(command)

        if not should_block:
            sys.exit(0)

        print(message)
        sys.exit(2)

    except Exception as err:
        print_hook_error(err)
        sys.exit(0)


if __name__ == "__main__":
    main()
