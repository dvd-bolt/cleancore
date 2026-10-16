"""Prepare the frontend and license notices before every desktop bundle."""
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parent.parent
out = root / "artifacts" / "bundle-input"
out.mkdir(parents=True, exist_ok=True)

def pnpm(*args, **kwargs):
    command = ["cmd.exe", "/d", "/c", "pnpm", *args] if sys.platform == "win32" else ["pnpm", *args]
    return subprocess.run(command, cwd=root, check=True, **kwargs)

subprocess.run([sys.executable, str(root / "scripts/prepare_assistant_runtime.py")], cwd=root, check=True)
pnpm("build")
metadata = subprocess.run(["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", "x86_64-pc-windows-msvc"], cwd=root, check=True, stdout=subprocess.PIPE)
(out / "cargo-metadata.json").write_bytes(metadata.stdout)
licenses = pnpm("licenses", "list", "--json", stdout=subprocess.PIPE)
(out / "frontend-licenses.json").write_bytes(licenses.stdout)
subprocess.run([sys.executable, str(root / "scripts/prepare_notices.py"), "--rust-metadata", str(out / "cargo-metadata.json"), "--frontend-licenses", str(out / "frontend-licenses.json")], cwd=root, check=True)
