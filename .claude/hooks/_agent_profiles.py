#!/usr/bin/env python3
"""
Capability-to-provider profile helpers for Claude hooks.
"""

from __future__ import annotations

import json
import os
import shlex
from pathlib import Path
from typing import Any

PROFILE_VERSION = 1
CONFIG_ENV_VAR = "CLAUDE_AGENT_PROFILES_PATH"
DEFAULT_CONFIG_PATH = Path(__file__).resolve().parent.parent / "agent-profiles.json"
REQUIRED_CAPABILITIES = (
    "orchestrator",
    "planner",
    "researcher",
    "implementer",
    "reviewer",
    "debugger",
    "multimodal_reader",
)
ORCHESTRATOR_PROVIDER = "claude"
WORKFLOW_HOST_PROVIDER_KEY = "workflow_host_provider"
WORKFLOW_HOST_MODEL_KEY = "workflow_host_model"
WORKFLOW_HOST_PROVIDER_ALIASES = (
    WORKFLOW_HOST_PROVIDER_KEY,
)
WORKFLOW_HOST_MODEL_ALIASES = (
    WORKFLOW_HOST_MODEL_KEY,
)
SUPPORTED_WORKFLOW_HOST_PROVIDERS = ("claude", "codex")
PLACEHOLDER_KEYS = ("task", "path", "model", "briefing_file")


class AgentProfilesError(ValueError):
    """Raised when the agent profile config is missing or invalid."""


def _looks_like_shell_command_template(template: str) -> bool:
    stripped = template.strip()
    if not stripped:
        return False
    if stripped.startswith("/"):
        return False
    if stripped.lower().startswith("continue "):
        return False
    return True


def _quote_for_single_quoted_shell(value: str) -> str:
    return value.replace("'", "'\"'\"'")


def _quote_for_double_quoted_shell(value: str) -> str:
    return (
        value.replace("\\", "\\\\")
        .replace('"', '\\"')
        .replace("$", "\\$")
        .replace("`", "\\`")
    )


def _placeholder_quote_context(template: str, start_index: int) -> str:
    in_single = False
    in_double = False
    escaped = False
    for char in template[:start_index]:
        if escaped:
            escaped = False
            continue
        if char == "\\" and not in_single:
            escaped = True
            continue
        if char == "'" and not in_double:
            in_single = not in_single
            continue
        if char == '"' and not in_single:
            in_double = not in_double
            continue
    if in_single:
        return "single"
    if in_double:
        return "double"
    return "unquoted"


def _sanitize_placeholder_value(value: str) -> str:
    return value.replace("\r", " ").replace("\n", " ")


def _render_shell_template(template: str, values: dict[str, str]) -> str:
    rendered_parts: list[str] = []
    index = 0
    while index < len(template):
        matched_key = None
        matched_placeholder = ""
        for key in PLACEHOLDER_KEYS:
            placeholder = "{" + key + "}"
            if template.startswith(placeholder, index):
                matched_key = key
                matched_placeholder = placeholder
                break

        if matched_key is None:
            rendered_parts.append(template[index])
            index += 1
            continue

        replacement_source = _sanitize_placeholder_value(values[matched_key])
        context = _placeholder_quote_context(template, index)
        if context == "double":
            replacement = _quote_for_double_quoted_shell(replacement_source)
        elif context == "single":
            replacement = _quote_for_single_quoted_shell(replacement_source)
        else:
            replacement = shlex.quote(replacement_source)
        rendered_parts.append(replacement)
        index += len(matched_placeholder)

    return "".join(rendered_parts)


def _render_plain_template(template: str, values: dict[str, str]) -> str:
    rendered = template
    for key in PLACEHOLDER_KEYS:
        rendered = rendered.replace("{" + key + "}", values[key])
    return rendered


def declared_provider_capabilities(
    provider_name: str, provider: dict[str, Any]
) -> set[str]:
    invoke_examples = provider.get("invoke_examples", {})
    if not isinstance(invoke_examples, dict):
        raise AgentProfilesError(
            f"Provider '{provider_name}' invoke_examples must be an object"
        )

    declared = {capability for capability in invoke_examples if capability != "default"}
    supported_capabilities = provider.get("supported_capabilities")
    if supported_capabilities is not None:
        if not isinstance(supported_capabilities, list) or not supported_capabilities:
            raise AgentProfilesError(
                f"Provider '{provider_name}' supported_capabilities must be a non-empty list"
            )
        for capability in supported_capabilities:
            if (
                not isinstance(capability, str)
                or capability not in REQUIRED_CAPABILITIES
            ):
                raise AgentProfilesError(
                    f"Provider '{provider_name}' declares unsupported capability '{capability}'"
                )
            declared.add(capability)

    if not declared:
        raise AgentProfilesError(
            f"Provider '{provider_name}' must declare at least one supported capability"
        )

    for capability in declared:
        if capability not in REQUIRED_CAPABILITIES:
            raise AgentProfilesError(
                f"Provider '{provider_name}' declares unsupported capability '{capability}'"
            )
        if capability not in invoke_examples and "default" not in invoke_examples:
            raise AgentProfilesError(
                f"Provider '{provider_name}' must define an example or default for capability "
                f"'{capability}'"
            )

    return declared


def config_path(path: str | Path | None = None) -> Path:
    if path is not None:
        return Path(path)
    env_path = os.environ.get(CONFIG_ENV_VAR)
    if env_path:
        return Path(env_path)
    return DEFAULT_CONFIG_PATH


def load_profiles(path: str | Path | None = None) -> dict[str, Any]:
    resolved_path = config_path(path)
    try:
        with resolved_path.open(encoding="utf-8") as handle:
            profiles = json.load(handle)
    except FileNotFoundError as err:
        raise AgentProfilesError(
            f"Missing agent profile config: {resolved_path}"
        ) from err
    except json.JSONDecodeError as err:
        raise AgentProfilesError(
            f"Invalid JSON in agent profile config {resolved_path}: line {err.lineno}"
        ) from err

    validate_profiles(profiles)
    return profiles


def validate_profiles(profiles: dict[str, Any]) -> None:
    if not isinstance(profiles, dict):
        raise AgentProfilesError("Agent profile config must be a JSON object")

    if profiles.get("version") != PROFILE_VERSION:
        raise AgentProfilesError(
            f"Unsupported agent profile config version: {profiles.get('version')}"
        )

    providers = profiles.get("providers")
    if not isinstance(providers, dict) or not providers:
        raise AgentProfilesError(
            "Agent profile config must define at least one provider"
        )

    provider_capability_map: dict[str, set[str]] = {}
    for provider_name, provider in providers.items():
        if not isinstance(provider, dict):
            raise AgentProfilesError(f"Provider '{provider_name}' must be an object")
        label = provider.get("label")
        if not isinstance(label, str) or not label.strip():
            raise AgentProfilesError(
                f"Provider '{provider_name}' must define a non-empty label"
            )
        default_model = provider.get("default_model")
        if default_model is not None:
            if not isinstance(default_model, str):
                raise AgentProfilesError(
                    f"Provider '{provider_name}' default_model must be a string"
                )
            if not default_model.strip():
                raise AgentProfilesError(
                    f"Provider '{provider_name}' default_model must not be empty"
                )
        invoke_examples = provider.get("invoke_examples", {})
        if not isinstance(invoke_examples, dict):
            raise AgentProfilesError(
                f"Provider '{provider_name}' invoke_examples must be an object"
            )
        for capability, example in invoke_examples.items():
            if not isinstance(capability, str) or not capability:
                raise AgentProfilesError(
                    f"Provider '{provider_name}' has an invalid example capability key"
                )
            if not isinstance(example, str) or not example.strip():
                raise AgentProfilesError(
                    f"Provider '{provider_name}' example '{capability}' must be a non-empty string"
                )
        provider_capability_map[provider_name] = declared_provider_capabilities(
            provider_name, provider
        )

    all_profiles = profiles.get("profiles")
    if not isinstance(all_profiles, dict) or not all_profiles:
        raise AgentProfilesError(
            "Agent profile config must define at least one profile"
        )

    active_name = profiles.get("active_profile")
    if not isinstance(active_name, str) or active_name not in all_profiles:
        raise AgentProfilesError(
            f"Active profile '{active_name}' is not defined in the profile map"
        )

    for profile_name, mapping in all_profiles.items():
        if not isinstance(mapping, dict):
            raise AgentProfilesError(f"Profile '{profile_name}' must be an object")

        if mapping.get("orchestrator") != ORCHESTRATOR_PROVIDER:
            raise AgentProfilesError(
                f"Profile '{profile_name}' must use '{ORCHESTRATOR_PROVIDER}' for the "
                "orchestrator capability in v1"
            )

        missing = [
            capability
            for capability in REQUIRED_CAPABILITIES
            if capability not in mapping
        ]
        if missing:
            missing_list = ", ".join(missing)
            raise AgentProfilesError(
                f"Profile '{profile_name}' is missing required capabilities: {missing_list}"
            )

        for capability in REQUIRED_CAPABILITIES:
            provider_name = mapping.get(capability)
            if not isinstance(provider_name, str) or not provider_name:
                raise AgentProfilesError(
                    f"Profile '{profile_name}' capability '{capability}' must map to a provider name"
                )
            if provider_name not in providers:
                raise AgentProfilesError(
                    f"Profile '{profile_name}' capability '{capability}' references unknown provider "
                    f"'{provider_name}'"
                )
            if capability not in provider_capability_map[provider_name]:
                raise AgentProfilesError(
                    f"Profile '{profile_name}' capability '{capability}' uses provider "
                    f"'{provider_name}', but that provider does not support capability "
                    f"'{capability}'"
                )

        # Validate {model} placeholder resolvability for each active capability
        model_overrides = mapping.get("provider_model_overrides", {})
        if not isinstance(model_overrides, dict):
            model_overrides = {}
        for capability in REQUIRED_CAPABILITIES:
            prov_name = mapping.get(capability)
            if not isinstance(prov_name, str) or prov_name not in providers:
                continue
            prov = providers[prov_name]
            invoke_examples = prov.get("invoke_examples", {})
            if not isinstance(invoke_examples, dict):
                continue
            template = invoke_examples.get(capability) or invoke_examples.get(
                "default", ""
            )
            if "{model}" in template:
                has_override = (
                    isinstance(model_overrides.get(prov_name), str)
                    and model_overrides[prov_name].strip()
                )
                has_default = (
                    isinstance(prov.get("default_model"), str)
                    and prov["default_model"].strip()
                )
                if not has_override and not has_default:
                    raise AgentProfilesError(
                        f"Profile '{profile_name}' capability '{capability}' uses provider "
                        f"'{prov_name}' whose template contains {{model}} but no model is "
                        f"configured (set default_model on provider or "
                        f"provider_model_overrides in profile)"
                    )

        host_provider_name = _profile_string_value(
            mapping, WORKFLOW_HOST_PROVIDER_ALIASES
        )
        if not isinstance(host_provider_name, str) or not host_provider_name:
            raise AgentProfilesError(
                f"Profile '{profile_name}' must define a non-empty "
                f"{WORKFLOW_HOST_PROVIDER_KEY}"
            )
        if host_provider_name not in SUPPORTED_WORKFLOW_HOST_PROVIDERS:
            supported_hosts = ", ".join(SUPPORTED_WORKFLOW_HOST_PROVIDERS)
            raise AgentProfilesError(
                f"Profile '{profile_name}' {WORKFLOW_HOST_PROVIDER_KEY} must be one of: "
                f"{supported_hosts}"
            )
        if host_provider_name not in providers:
            raise AgentProfilesError(
                f"Profile '{profile_name}' {WORKFLOW_HOST_PROVIDER_KEY} references unknown provider "
                f"'{host_provider_name}'"
            )

        host_model = _profile_string_value(mapping, WORKFLOW_HOST_MODEL_ALIASES)
        if not isinstance(host_model, str) or not host_model.strip():
            raise AgentProfilesError(
                f"Profile '{profile_name}' must define a non-empty "
                f"{WORKFLOW_HOST_MODEL_KEY}"
            )

        model_overrides = mapping.get("provider_model_overrides")
        if model_overrides is not None:
            if not isinstance(model_overrides, dict):
                raise AgentProfilesError(
                    f"Profile '{profile_name}' provider_model_overrides must be a dict"
                )
            for override_provider, override_model in model_overrides.items():
                if not isinstance(override_model, str) or not override_model.strip():
                    raise AgentProfilesError(
                        f"Profile '{profile_name}' provider_model_overrides values must be "
                        f"non-empty strings, got {override_model!r} for '{override_provider}'"
                    )
                if override_provider not in providers:
                    raise AgentProfilesError(
                        f"Profile '{profile_name}' provider_model_overrides references "
                        f"unknown provider '{override_provider}'"
                    )


def active_profile_name(
    profiles: dict[str, Any] | None = None, path: str | Path | None = None
) -> str:
    resolved_profiles = profiles if profiles is not None else load_profiles(path)
    active_name = resolved_profiles.get("active_profile")
    if not isinstance(active_name, str) or not active_name:
        raise AgentProfilesError(
            "Agent profile config does not define a valid active_profile"
        )
    return active_name


def active_profile(
    profiles: dict[str, Any] | None = None, path: str | Path | None = None
) -> dict[str, Any]:
    resolved_profiles = profiles if profiles is not None else load_profiles(path)
    profile_name = active_profile_name(resolved_profiles)
    profile = resolved_profiles["profiles"].get(profile_name)
    if not isinstance(profile, dict):
        raise AgentProfilesError(
            f"Active profile '{profile_name}' is not a valid object"
        )
    return profile


def resolve_provider(
    capability: str,
    profiles: dict[str, Any] | None = None,
    path: str | Path | None = None,
) -> str:
    if capability not in REQUIRED_CAPABILITIES:
        raise AgentProfilesError(f"Unknown capability '{capability}'")
    profile = active_profile(profiles=profiles, path=path)
    provider_name = profile.get(capability)
    if not isinstance(provider_name, str) or not provider_name:
        raise AgentProfilesError(
            f"Capability '{capability}' is not mapped in the active profile"
        )
    return provider_name


def provider_definition(
    provider_name: str,
    profiles: dict[str, Any] | None = None,
    path: str | Path | None = None,
) -> dict[str, Any]:
    resolved_profiles = profiles if profiles is not None else load_profiles(path)
    providers = resolved_profiles.get("providers", {})
    provider = providers.get(provider_name)
    if not isinstance(provider, dict):
        raise AgentProfilesError(f"Unknown provider '{provider_name}'")
    return provider


def provider_label_for_name(
    provider_name: str,
    profiles: dict[str, Any] | None = None,
    path: str | Path | None = None,
) -> str:
    provider = provider_definition(provider_name, profiles=profiles, path=path)
    label = provider.get("label")
    if not isinstance(label, str) or not label:
        raise AgentProfilesError(
            f"Provider '{provider_name}' does not define a valid label"
        )
    return label


def provider_label(
    capability: str,
    profiles: dict[str, Any] | None = None,
    path: str | Path | None = None,
) -> str:
    provider_name = resolve_provider(capability, profiles=profiles, path=path)
    return provider_label_for_name(provider_name, profiles=profiles, path=path)


def resolve_provider_model(
    provider_name: str,
    profiles: dict[str, Any] | None = None,
    path: str | Path | None = None,
) -> str | None:
    """Resolve the model for a provider: profile override > provider default_model > None.

    Returned value is always stripped (no leading/trailing whitespace).
    """
    resolved_profiles = profiles if profiles is not None else load_profiles(path)
    profile = active_profile(profiles=resolved_profiles)
    model_overrides = profile.get("provider_model_overrides")
    if isinstance(model_overrides, dict):
        override = model_overrides.get(provider_name)
        if isinstance(override, str) and override.strip():
            return override.strip()
    provider = provider_definition(provider_name, profiles=resolved_profiles)
    default_model = provider.get("default_model")
    if isinstance(default_model, str) and default_model.strip():
        return default_model.strip()
    return None


def provider_example(
    capability: str,
    profiles: dict[str, Any] | None = None,
    path: str | Path | None = None,
) -> str:
    provider_name = resolve_provider(capability, profiles=profiles, path=path)
    provider = provider_definition(provider_name, profiles=profiles, path=path)
    invoke_examples = provider.get("invoke_examples", {})
    if not isinstance(invoke_examples, dict):
        raise AgentProfilesError(
            f"Provider '{provider_name}' does not define invoke_examples"
        )
    example = invoke_examples.get(capability) or invoke_examples.get("default")
    if not isinstance(example, str) or not example:
        raise AgentProfilesError(
            f"Provider '{provider_name}' does not define an example for capability '{capability}'"
        )
    return example


def render_provider_example(
    capability: str,
    task: str = "{task}",
    file_path: str = "{path}",
    briefing_file: str = "{briefing_file}",
    profiles: dict[str, Any] | None = None,
    path: str | Path | None = None,
) -> str:
    resolved_profiles = profiles if profiles is not None else load_profiles(path)
    provider_name = resolve_provider(capability, profiles=resolved_profiles)
    example = provider_example(capability, profiles=resolved_profiles)
    model = resolve_provider_model(provider_name, profiles=resolved_profiles)
    if "{model}" in example and model is None:
        raise AgentProfilesError(
            f"Provider '{provider_name}' invoke_example uses {{model}} placeholder "
            f"but model is not configured (set default_model on provider or "
            f"provider_model_overrides in profile)"
        )
    values = {
        "task": task,
        "path": file_path,
        "model": model or "",
        "briefing_file": briefing_file,
    }
    if _looks_like_shell_command_template(example):
        return _render_shell_template(example, values)
    return _render_plain_template(example, values)


def profile_value(
    key: str, profiles: dict[str, Any] | None = None, path: str | Path | None = None
) -> str:
    profile = active_profile(profiles=profiles, path=path)
    value = profile.get(key)
    if not isinstance(value, str) or not value.strip():
        raise AgentProfilesError(
            f"Profile key '{key}' is not configured in the active profile"
        )
    return value


def _profile_string_value(mapping: dict[str, Any], keys: tuple[str, ...]) -> str | None:
    for key in keys:
        value = mapping.get(key)
        if isinstance(value, str) and value.strip():
            return value.strip()
    return None


def workflow_host_provider(
    profiles: dict[str, Any] | None = None, path: str | Path | None = None
) -> str:
    profile = active_profile(profiles=profiles, path=path)
    provider_name = _profile_string_value(profile, WORKFLOW_HOST_PROVIDER_ALIASES)
    if provider_name is None:
        raise AgentProfilesError(
            f"Profile key '{WORKFLOW_HOST_PROVIDER_KEY}' is not configured in the active profile"
        )
    if provider_name not in SUPPORTED_WORKFLOW_HOST_PROVIDERS:
        supported_hosts = ", ".join(SUPPORTED_WORKFLOW_HOST_PROVIDERS)
        raise AgentProfilesError(
            f"Active profile {WORKFLOW_HOST_PROVIDER_KEY} must be one of: {supported_hosts}"
        )
    return provider_name


def workflow_host_model(
    profiles: dict[str, Any] | None = None, path: str | Path | None = None
) -> str:
    profile = active_profile(profiles=profiles, path=path)
    model = _profile_string_value(profile, WORKFLOW_HOST_MODEL_ALIASES)
    if model is None:
        raise AgentProfilesError(
            f"Profile key '{WORKFLOW_HOST_MODEL_KEY}' is not configured in the active profile"
        )
    return model


def workflow_host_label(
    profiles: dict[str, Any] | None = None, path: str | Path | None = None
) -> str:
    provider_name = workflow_host_provider(profiles=profiles, path=path)
    return provider_label_for_name(provider_name, profiles=profiles, path=path)


def provider_command_prefixes(
    profiles: dict[str, Any] | None = None, path: str | Path | None = None
) -> tuple[str, ...]:
    resolved_profiles = profiles if profiles is not None else load_profiles(path)
    active_providers = {
        active_profile(profiles=resolved_profiles).get(capability)
        for capability in REQUIRED_CAPABILITIES
    }
    prefixes: set[str] = set()
    for provider_name in active_providers:
        if not isinstance(provider_name, str) or not provider_name:
            continue
        provider = provider_definition(provider_name, profiles=resolved_profiles)
        invoke_examples = provider.get("invoke_examples", {})
        if not isinstance(invoke_examples, dict):
            continue
        for example in invoke_examples.values():
            if not isinstance(example, str):
                continue
            stripped = example.strip()
            if not stripped:
                continue
            token = stripped.split()[0]
            if token.startswith(("/", "./", "../")) or (
                token[0].islower() and token[0].isascii()
            ):
                prefixes.add(token)
    return tuple(sorted(prefixes))
