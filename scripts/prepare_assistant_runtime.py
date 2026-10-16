"""Explicit build-time provisioning of the pinned CPU runtime. Never downloads weights."""
from pathlib import Path
import hashlib
import json
import urllib.request
import zipfile

repo = Path(__file__).resolve().parent.parent
manifest = json.loads((repo / "crates/cleaner-assistant/runtime-manifest.json").read_text(encoding="utf-8"))
root = repo / "runtime/llama-b11429"
if not root.resolve().is_relative_to(repo):
    raise SystemExit("Runtime directory must remain inside the workspace")
if all((root / name).is_file() and hashlib.sha256((root / name).read_bytes()).hexdigest() == checksum for name, checksum in manifest.items()):
    print("Pinned llama.cpp b11429 verified")
    raise SystemExit(0)
cache = repo / "artifacts/stage8"
cache.mkdir(parents=True, exist_ok=True)
archive = cache / "llama-b11429.zip"
url = "https://github.com/ggml-org/llama.cpp/releases/download/b11429/llama-b11429-bin-win-cpu-x64.zip"
if not archive.exists():
    urllib.request.urlretrieve(url, archive)
if hashlib.sha256(archive.read_bytes()).hexdigest() != "1283323272b04cd07905816a597a0da810918102de958f4ff6f7bbaa70ed2efe":
    raise SystemExit("llama.cpp archive integrity mismatch")
root.mkdir(parents=True, exist_ok=True)
with zipfile.ZipFile(archive) as package:
    for name, checksum in manifest.items():
        if name != Path(name).name or ".." in name or ":" in name:
            raise SystemExit("Unsafe manifest path")
        if name == "LICENSE-llama.cpp":
            data = urllib.request.urlopen("https://raw.githubusercontent.com/ggml-org/llama.cpp/d81235049384534c167caea52b85a694f6103d14/LICENSE", timeout=30).read(65536)
        else:
            data = package.read(name)
        if hashlib.sha256(data).hexdigest() != checksum:
            raise SystemExit(f"Runtime file integrity mismatch: {name}")
        (root / name).write_bytes(data)
print(f"Provisioned {len(manifest)} pinned runtime files; no model downloaded")
