"""Regenerate native app icons from the shared FrameFetch vector master."""
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parent.parent
subprocess.run(
    ["node", str(root / "node_modules/@tauri-apps/cli/tauri.js"),
     "icon", str(root / "public-brand/framefetch.svg")],
    cwd=root,
    check=True,
)
