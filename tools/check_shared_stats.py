"""Says whether PrismQDA's statistics crate has changed since the commit PrismStatistics pins (decision D03).

PrismStatistics takes its core statistics from `prismqda-stats`, pinned in Cargo.toml to one PrismQDA commit, so a
change in PrismQDA never changes PrismStatistics's numbers by itself. The cost is that a fix made there does not reach
here. Until the crate is shared from openstage-core (CORE-REQUESTS.md, R5), this check is the bridge: run it before
each release, and when it reports changes, read them, move the pin and run `python tools/validate.py`.

    python tools/check_shared_stats.py            # list changes since the pin (exit 1 if there are any)
    python tools/check_shared_stats.py --branch main

It asks GitHub through the `gh` command (signed in), reading only the public PrismQDA repository.
"""

import argparse
import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CRATE = "crates/prismqda-stats/"


def pin():
    text = open(os.path.join(ROOT, "Cargo.toml"), encoding="utf-8").read()
    m = re.search(r'prismqda-stats\s*=\s*\{\s*git\s*=\s*"https://github\.com/([^"]+?)(?:\.git)?"\s*,\s*rev\s*=\s*"([0-9a-f]{7,40})"', text)
    if not m:
        sys.exit("Cargo.toml does not pin prismqda-stats to a git commit; nothing to compare.")
    return m.group(1), m.group(2)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--branch", default="main")
    a = ap.parse_args()
    repo, rev = pin()
    try:
        out = subprocess.run(["gh", "api", f"repos/{repo}/compare/{rev}...{a.branch}"], capture_output=True, text=True, encoding="utf-8", check=True).stdout
    except (OSError, subprocess.CalledProcessError) as e:
        sys.exit(f"Could not ask GitHub (is `gh` installed and signed in?): {e}")
    cmp = json.loads(out)
    files = [f for f in cmp.get("files", []) if f["filename"].startswith(CRATE)]
    print(f"Pinned: {repo} at {rev[:10]}; {a.branch} is {cmp.get('ahead_by', 0)} commit(s) ahead.")
    if not files:
        print(f"No change to {CRATE} since the pin: PrismStatistics and PrismQDA compute with the same code.")
        return 0
    print(f"{len(files)} file(s) in {CRATE} changed since the pin:")
    for f in files:
        print(f"  {f['status']:<9} {f['filename']} (+{f['additions']} -{f['deletions']})")
    commits = [c for c in cmp.get("commits", [])]
    if commits:
        print("Commits on the branch since the pin (newest last):")
        for c in commits[-20:]:
            print(f"  {c['sha'][:10]} {c['commit']['message'].splitlines()[0]}")
    print("Read the changes, move the rev in Cargo.toml, then run python tools/validate.py.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
