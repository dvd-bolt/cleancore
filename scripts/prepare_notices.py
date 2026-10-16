"""Build a dependency inventory and preserve shipped license/notice texts.
Inputs are produced with cargo metadata and pnpm licenses list (see RELEASE.md).
This does not replace a legal review of SPDX expressions or embedded resources.
"""
import argparse
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--rust-metadata", type=Path, required=True)
parser.add_argument("--frontend-licenses", type=Path, required=True)
parser.add_argument("--output", type=Path, default=Path("artifacts/legal"))
args = parser.parse_args()
rust = json.loads(args.rust_metadata.read_text(encoding="utf-8-sig"))
frontend = json.loads(args.frontend_licenses.read_text(encoding="utf-8-sig"))
nodes = {node["id"]: node for node in rust["resolve"]["nodes"]}
active = set()
pending = list(rust["workspace_members"])
while pending:
    package_id = pending.pop()
    if package_id in active:
        continue
    active.add(package_id)
    pending.extend(dependency["pkg"] for dependency in nodes[package_id]["deps"])
packages = []
for package in rust["packages"]:
    if package["id"] in active and package["source"] is not None:
        packages.append(("Rust", package["name"], package["version"], package.get("license"), Path(package["manifest_path"]).parent))
for license_name, entries in frontend.items():
    for entry in entries:
        packages.append(("Frontend", entry["name"], ", ".join(entry["versions"]), entry.get("license", license_name), Path(entry["paths"][0])))
packages.sort(key=lambda item: (item[0], item[1], item[2]))
args.output.mkdir(parents=True, exist_ok=True)
lines = ["# CleanCore third-party notices", "", "Generated from locked dependencies. Windows x64 Rust graph and frontend package inventory.", "", "Review SPDX alternatives and packages without root license texts before a public release.", ""]
inventory = []
missing = []
for ecosystem, name, version, license_name, folder in packages:
    if not license_name:
        missing.append(f"{name}@{version}: missing license metadata")
    texts = []
    if folder.is_dir():
        for candidate in sorted(folder.iterdir()):
            if candidate.is_file() and candidate.name.lower().startswith(("license", "licence", "copying", "notice", "copyright")):
                texts.append((candidate.name, candidate.read_text(encoding="utf-8", errors="replace")))
    inventory.append({"ecosystem": ecosystem, "name": name, "version": version, "license": license_name, "license_files": [name for name, _ in texts]})
    lines.extend([f"## {ecosystem}: {name} {version}", "", f"License: {license_name or 'UNKNOWN'}", ""])
    if not texts:
        lines.extend(["No root license text found; review the package source before redistribution.", ""])
    for filename, content in texts:
        lines.extend([f"### {filename}", "", "````text", content.rstrip(), "````", ""])
# Native runtime is outside package managers and must remain visible in the inventory.
runtime = Path(__file__).resolve().parent.parent / "runtime/llama-b11429"
if runtime.is_dir():
    licenses = ["LICENSE-llama.cpp", "LICENSE-LLVM-OpenMP"]
    inventory.append({"ecosystem": "Native runtime", "name": "llama.cpp CPU / LLVM OpenMP", "version": "b11429", "license": "MIT; Apache-2.0 WITH LLVM-exception", "license_files": licenses})
    lines.extend(["## Native runtime: llama.cpp b11429 / LLVM OpenMP", "", "Unmodified official CPU runtime; pinned hashes in runtime-manifest.json. Model weights are not included.", ""])
    for filename in licenses:
        lines.extend([f"### {filename}", "", "````text", (runtime / filename).read_text(encoding="utf-8").rstrip(), "````", ""])
(args.output / "THIRD_PARTY_NOTICES.md").write_text("\n".join(lines), encoding="utf-8")
(args.output / "inventory.json").write_text(json.dumps(inventory, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(f"Inventoried {len(inventory)} dependencies; {sum(not p['license_files'] for p in inventory)} require source license-text review.")
if missing:
    raise SystemExit("\n".join(missing))
