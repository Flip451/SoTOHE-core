import importlib.util
import json
import sys
from copy import deepcopy
from pathlib import Path


def load_hook_module(module_name: str):
    hooks_dir = Path(__file__).resolve().parent
    module_path = hooks_dir / f"{module_name}.py"
    if str(hooks_dir) not in sys.path:
        sys.path.insert(0, str(hooks_dir))
    spec = importlib.util.spec_from_file_location(
        module_name.replace("-", "_"), module_path
    )
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def load_agent_profiles_data() -> dict:
    config_path = Path(__file__).resolve().parent.parent / "agent-profiles.json"
    return json.loads(config_path.read_text(encoding="utf-8"))


def write_agent_profiles(
    target_path: str | Path, active_profile: str, mutator=None
) -> Path:
    profiles = deepcopy(load_agent_profiles_data())
    profiles["active_profile"] = active_profile
    if mutator is not None:
        mutator(profiles)
    destination = Path(target_path)
    destination.write_text(json.dumps(profiles), encoding="utf-8")
    return destination
