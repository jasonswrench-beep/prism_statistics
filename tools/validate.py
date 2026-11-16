"""The validation suite: runs every reference test of the engine and writes docs/validation-report.md.

Each reference test (crates/prism-engine/tests/*_reference.rs and the other integration tests there) compares the engine
with values made by a reference script in crates/prism-engine/tests/reference: R scripts (make_*_reference.R) and, for
SEM, the product owner's reference packages. The report lists, for each suite, what it checks, the script and R packages
behind it with their installed versions, and every test's outcome. Sign-offs live in docs/validation-signoff.md.

    python tools/validate.py                 # run the tests and write the report
    python tools/validate.py --regenerate    # also re-run the R scripts first and report any reference value that moved
    python tools/validate.py --rscript "C:/Program Files/R/R-4.6.1/bin/Rscript.exe"

Exit status 1 when any test fails (or, with --regenerate, any reference file changed).
"""

import argparse
import datetime
import glob
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TESTS = os.path.join(ROOT, "crates", "prism-engine", "tests")
REF = os.path.join(TESTS, "reference")
REPORT = os.path.join(ROOT, "docs", "validation-report.md")


def find_rscript(given):
    if given:
        return given
    for base in (r"C:\Program Files\R", "/usr/lib/R", "/Library/Frameworks/R.framework/Versions"):
        found = sorted(v for v in glob.glob(os.path.join(base, "R-*")) if "devel" not in v)
        for v in reversed(found):
            exe = os.path.join(v, "bin", "Rscript.exe")
            if os.path.isfile(exe):
                return exe
    for exe in ("Rscript", "Rscript.exe"):
        try:
            subprocess.run([exe, "--version"], capture_output=True, check=True)
            return exe
        except (OSError, subprocess.CalledProcessError):
            pass
    return None


def suite_info(path):
    name = os.path.basename(path)[:-3]
    text = open(path, encoding="utf-8").read()
    doc = " ".join(l[3:].strip() for l in text.splitlines() if l.startswith("//!"))
    scripts = sorted(set(re.findall(r"make_[a-z_]+\.(?:R|ps1|do)", text + doc)))
    guess = f"make_{name}.R"
    if not scripts and os.path.isfile(os.path.join(REF, guess)):
        scripts = [guess]
    packages = set()
    for s in scripts:
        p = os.path.join(REF, s)
        if s.endswith(".R") and os.path.isfile(p):
            src = open(p, encoding="utf-8").read()
            packages |= set(re.findall(r"library\(([A-Za-z0-9.]+)\)", src))
            packages |= set(re.findall(r"requireNamespace\(\"([A-Za-z0-9.]+)\"", src))
            packages |= set(re.findall(r"\b([A-Za-z][A-Za-z0-9.]+)::", src))
    packages.discard("base")
    packages.discard("stats")
    packages.discard("utils")
    return {"name": name, "doc": doc, "scripts": scripts, "packages": sorted(packages)}


def run_suite(name):
    out = subprocess.run(["cargo", "test", "-q", "-p", "prism-engine", "--test", name, "--", "--test-threads=4"], cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace")
    tests = re.findall(r"^test (\S+) \.\.\. (ok|FAILED|ignored)", out.stdout, re.M)
    if not tests:
        # -q prints dots only; run again listing the outcomes.
        out = subprocess.run(["cargo", "test", "-p", "prism-engine", "--test", name, "--", "--test-threads=4"], cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace")
        tests = re.findall(r"^test (\S+) \.\.\. (ok|FAILED|ignored)", out.stdout, re.M)
    if not tests and out.returncode != 0:
        tests = [("(build)", "FAILED")]
    return tests


def r_versions(rscript, packages):
    if not rscript:
        return None, {}
    code = 'cat("R", as.character(getRversion()), "\\n"); for (p in commandArgs(TRUE)) cat(p, tryCatch(as.character(packageVersion(p)), error = function(e) "not installed"), "\\n")'
    out = subprocess.run([rscript, "-e", code] + packages, capture_output=True, text=True)
    versions = dict(l.split(" ", 1) for l in out.stdout.strip().splitlines() if " " in l)
    return versions.pop("R", "").strip() or None, {k: v.strip() for k, v in versions.items()}


def regenerate(rscript, suites):
    if not rscript:
        sys.exit("R isn't installed (or --rscript wasn't given), so the references can't be regenerated.")
    ran = []
    for s in sorted({x for suite in suites for x in suite["scripts"] if x.endswith(".R")}):
        r = subprocess.run([rscript, s], cwd=REF, capture_output=True, text=True)
        ran.append((s, r.returncode == 0, (r.stderr or "").strip().splitlines()[-1:] or [""]))
    diff = subprocess.run(["git", "diff", "--stat", "--", REF], cwd=ROOT, capture_output=True, text=True).stdout.strip()
    return ran, diff


SIGNOFF = os.path.join(ROOT, "docs", "validation-signoff.md")


def signoff(suites):
    """Adds a blank row to the sign-off table for every suite not in it yet."""
    nl = "\n"
    if not os.path.isfile(SIGNOFF):
        head = [
            "# Validation sign-off",
            "",
            "The product owner signs a suite after reviewing its comparison in `docs/validation-report.md` (validation decided "
            "2026-11-11: checked against R and the owner's reference packages, signed by the owner). When a suite's reference "
            "values change, clear its row and sign again.",
            "",
            "| Suite | Signed by | Date | Notes |",
            "|---|---|---|---|",
            "",
        ]
        open(SIGNOFF, "w", encoding="utf-8", newline=nl).write(nl.join(head))
    text = open(SIGNOFF, encoding="utf-8").read()
    have = set(re.findall(r"^\| `([a-z0-9_]+)` \|", text, re.M))
    new = [s["name"] for s in suites if s["name"] not in have]
    if new:
        if not text.endswith(nl):
            text += nl
        text += "".join(f"| `{n}` | | | |{nl}" for n in new)
        open(SIGNOFF, "w", encoding="utf-8", newline=nl).write(text)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--regenerate", action="store_true")
    ap.add_argument("--rscript")
    ap.add_argument("--out", default=REPORT)
    a = ap.parse_args()

    files = sorted(glob.glob(os.path.join(TESTS, "*.rs")))
    suites = [suite_info(f) for f in files]
    rscript = find_rscript(a.rscript)
    regen = regenerate(rscript, suites) if a.regenerate else None
    all_packages = sorted({p for s in suites for p in s["packages"]})
    r_version, versions = r_versions(rscript, all_packages)

    failed = 0
    for s in suites:
        print(f"running {s['name']} ...", flush=True)
        s["tests"] = run_suite(s["name"])
        failed += sum(1 for _, r in s["tests"] if r == "FAILED")

    version = re.search(r'^version = "([^"]+)"', open(os.path.join(ROOT, "Cargo.toml"), encoding="utf-8").read(), re.M)
    commit = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT, capture_output=True, text=True).stdout.strip()
    total = sum(len(s["tests"]) for s in suites)
    passed = sum(1 for s in suites for _, r in s["tests"] if r == "ok")
    ignored = sum(1 for s in suites for _, r in s["tests"] if r == "ignored")

    L = []
    L.append("# Validation report")
    L.append("")
    L.append(f"Generated {datetime.date.today().isoformat()} by `python tools/validate.py` at commit `{commit}` (engine {version.group(1) if version else '?'}).")
    L.append(f"R {r_version}." if r_version else "R was not found, so package versions are not listed.")
    L.append("")
    L.append(f"**{passed} of {total} reference tests passed**" + (f", {ignored} ignored" if ignored else "") + (f", **{failed} failed**." if failed else "."))
    L.append("")
    L.append("Each suite compares the engine with values computed independently: R scripts in `crates/prism-engine/tests/reference` (re-run them with `--regenerate`), and for SEM the product owner's reference packages. Tolerances are written in each test file: exact (to about 1e-8) where the reference is deterministic, and within the spread of repeated reference runs where it depends on random draws. The core methods shared with PrismQDA (t tests, ANOVA, correlation, regression, nonparametric tests and more) are validated against R in the `prismqda-stats` crate's own suite (published from openstage-core, pinned by tag), and the engine's unit tests (`cargo test -p prism-engine --lib`) cover each analysis module.")
    L.append("")
    if regen:
        ran, diff = regen
        L.append("## Reference scripts re-run")
        L.append("")
        for s, ok, last in ran:
            L.append(f"- `{s}`: {'ran' if ok else 'FAILED: ' + last[0]}")
        L.append("")
        L.append("Reference files changed by the re-run:" if diff else "No reference value changed.")
        if diff:
            L.append("")
            L.append("```")
            L.append(diff)
            L.append("```")
        L.append("")
    L.append("## Suites")
    L.append("")
    L.append("| Suite | Reference | R packages | Tests |")
    L.append("|---|---|---|---|")
    for s in suites:
        ok = sum(1 for _, r in s["tests"] if r == "ok")
        bad = sum(1 for _, r in s["tests"] if r == "FAILED")
        pk = ", ".join(f"{p} {versions[p]}" if p in versions else p for p in s["packages"]) or "—"
        ref = ", ".join(f"`{x}`" for x in s["scripts"]) or "—"
        L.append(f"| `{s['name']}` | {ref} | {pk} | {ok}/{len(s['tests'])}{' ✗ ' + str(bad) + ' failed' if bad else ''} |")
    L.append("")
    for s in suites:
        L.append(f"### {s['name']}")
        L.append("")
        if s["doc"]:
            L.append(s["doc"])
            L.append("")
        for t, r in s["tests"]:
            L.append(f"- {'✓' if r == 'ok' else ('–' if r == 'ignored' else '✗')} `{t}`{'' if r == 'ok' else ' (' + r.lower() + ')'}")
        L.append("")
    L.append("## Sign-off")
    L.append("")
    L.append("The product owner signs each suite in `docs/validation-signoff.md` after reviewing its comparison. This script adds a row there for any new suite and never changes a signed row.")
    L.append("")
    signoff(suites)
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    open(a.out, "w", encoding="utf-8", newline="\n").write("\n".join(L))
    print(f"{passed}/{total} passed; report written to {os.path.relpath(a.out, ROOT)}")
    changed = bool(regen and regen[1])
    sys.exit(1 if failed or changed else 0)


if __name__ == "__main__":
    main()
