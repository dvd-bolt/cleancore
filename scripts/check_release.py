"""Fail closed until the release acceptance matrix has been recorded."""
import json
from pathlib import Path
import sys

root = Path(__file__).resolve().parent.parent
versions = [json.loads((root / "package.json").read_text(encoding="utf-8"))["version"], json.loads((root / "src-tauri/tauri.conf.json").read_text(encoding="utf-8"))["version"]]
import tomllib
versions.append(tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"])
if len(set(versions)) != 1:
    raise SystemExit(f"Version mismatch: {versions}")
if "--versions-only" in sys.argv:
    print(f"Version consistency: {versions[0]}")
    raise SystemExit(0)
acceptance = json.loads((root / "docs/release-acceptance.json").read_text(encoding="utf-8"))
required = {"safe_recycle", "windows10_22h2_x64", "windows11_x64", "offline_install", "upgrade_preserves_data", "uninstall", "keyboard_and_scale", "soak_test", "dependency_audit", "license_review", "local_assistant_real_model"}
if set(acceptance) != required:
    raise SystemExit("Release acceptance keys do not match the mandatory matrix")
missing = [key for key in sorted(required) if acceptance[key].get("passed") is not True or not acceptance[key].get("evidence")]
if missing:
    raise SystemExit("Release blocked: " + ", ".join(missing))
print("All release acceptance evidence recorded")
