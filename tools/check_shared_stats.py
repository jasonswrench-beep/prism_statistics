"""Says whether the shared statistics crate has moved on since the tag PrismStatistics pins (decision D03).

PrismStatistics takes its core statistics from `prismqda-stats`, published from openstage-core and pinned in
Cargo.toml to a tag there (`prismqda-stats-v1.0.0`), so a change in the core never changes PrismStatistics's numbers
by itself. Run this before each release (tools/release.py does). When it reports a newer tag, read the changes, move
the tag in Cargo.toml, run `cargo update -p prismqda-stats` (here and in app/src-tauri) and `python tools/validate.py`.

    python tools/check_shared_stats.py            # exit 1 when a newer tag exists or main has untagged changes

It asks GitHub through the `gh` command (signed in; the core repository is private for now).
"""

import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CRATE = "crates/prismqda-stats/"
PREFIX = "prismqda-stats-v"


def pinned():
    text = open(os.path.join(ROOT, "Cargo.toml"), encoding="utf-8").read()
    m = re.search(r'prismqda-stats\s*=\s*\{\s*git\s*=\s*"https://github\.com/([^"]+?)(?:\.git)?"\s*,\s*tag\s*=\s*"([^"]+)"', text)
    if not m:
        sys.exit("Cargo.toml does not pin prismqda-stats to a git tag; nothing to compare.")
    return m.group(1), m.group(2)


def gh(path):
    try:
        out = subprocess.run(["gh", "api", path], capture_output=True, text=True, encoding="utf-8", check=True).stdout
    except (OSError, subprocess.CalledProcessError) as e:
        sys.exit(f"Could not ask GitHub (is `gh` installed and signed in?): {e}")
    return json.loads(out)


def version(tag):
    return tuple(int(x) for x in tag[len(PREFIX):].split(".") if x.isdigit())


def main():
    repo, tag = pinned()
    tags = sorted((t["name"] for t in gh(f"repos/{repo}/tags?per_page=100") if t["name"].startswith(PREFIX)), key=version)
    print(f"Pinned: {repo} at {tag}. Tags of the crate there: {', '.join(tags) or 'none'}.")
    status = 0
    newer = [t for t in tags if version(t) > version(tag)]
    if newer:
        print(f"Newer: {', '.join(newer)}. Read the changes, move the tag in Cargo.toml and validate.")
        status = 1
    cmp = gh(f"repos/{repo}/compare/{tag}...main")
    files = [f for f in cmp.get("files", []) if f["filename"].startswith(CRATE)]
    if files:
        print(f"{len(files)} file(s) in {CRATE} changed on main since {tag}, not yet tagged:")
        for f in files:
            print(f"  {f['status']:<9} {f['filename']} (+{f['additions']} -{f['deletions']})")
        status = 1
    if not status:
        print("No change since the pinned tag: PrismStatistics computes with the core's current crate.")
    return status


if __name__ == "__main__":
    sys.exit(main())
