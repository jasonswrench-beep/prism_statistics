"""Release steps for PrismStatistics: version checks, the full check run, and an UNSIGNED Windows installer.

    python tools/release.py check             # the version is the same everywhere it is written
    python tools/release.py bump 0.2.0        # set a new version everywhere
    python tools/release.py build             # tests, validation report, UI build, then the unsigned installer

`build` stops at the first failure. It maps the build folders out of the program (--remap-path-prefix, /d1trimfile) and
fails if the program still holds the user profile or repository path. It writes the installer, its SHA-256 checksum and the release notes for this version
(from CHANGELOG.md) to release/<version>/. Code signing is a separate, deliberate step that the product owner starts:
this script never signs, and it refuses to run while a signing setting is present in tauri.conf.json.
"""

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CARGO = os.path.join(ROOT, "Cargo.toml")
TAURI_CARGO = os.path.join(ROOT, "app", "src-tauri", "Cargo.toml")
TAURI_CONF = os.path.join(ROOT, "app", "src-tauri", "tauri.conf.json")
PACKAGE = os.path.join(ROOT, "app", "ui", "package.json")
CHANGELOG = os.path.join(ROOT, "CHANGELOG.md")
VERSION_RE = re.compile(r'^(version = ")([^"]+)(")', re.M)


def read(p):
    return open(p, encoding="utf-8").read()


def write(p, s):
    open(p, "w", encoding="utf-8", newline="").write(s)


def versions():
    return {
        "Cargo.toml (workspace)": VERSION_RE.search(read(CARGO)).group(2),
        "app/src-tauri/Cargo.toml": VERSION_RE.search(read(TAURI_CARGO)).group(2),
        "app/src-tauri/tauri.conf.json": json.loads(read(TAURI_CONF))["version"],
        "app/ui/package.json": json.loads(read(PACKAGE))["version"],
    }


def check():
    v = versions()
    if len(set(v.values())) != 1:
        for k, x in v.items():
            print(f"  {k}: {x}")
        sys.exit("The version differs between files; run: python tools/release.py bump <version>")
    version = next(iter(v.values()))
    print(f"Version {version} everywhere.")
    return version


def bump(new):
    if not re.fullmatch(r"\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?", new):
        sys.exit(f"{new} isn't a version like 1.2.3 or 1.2.3-beta.1")
    for p in (CARGO, TAURI_CARGO):
        write(p, VERSION_RE.sub(lambda m: m.group(1) + new + m.group(3), read(p), count=1))
    for p in (TAURI_CONF, PACKAGE):
        write(p, re.sub(r'("version":\s*")[^"]+(")', lambda m: m.group(1) + new + m.group(2), read(p), count=1))
    # Refresh the lock files' own entries.
    subprocess.run(["cargo", "update", "-w", "-q"], cwd=ROOT)
    subprocess.run(["cargo", "update", "-p", "prismstatistics-app", "-q"], cwd=os.path.join(ROOT, "app", "src-tauri"))
    subprocess.run(["npm", "install", "--package-lock-only", "--silent"], cwd=os.path.join(ROOT, "app", "ui"), shell=True)
    check()
    if f"## {new}" not in read(CHANGELOG):
        print(f"Add a '## {new}' section to CHANGELOG.md before building.")


def step(title, cmd, cwd=ROOT, shell=False, env=None):
    print(f"\n== {title}", flush=True)
    r = subprocess.run(cmd, cwd=cwd, shell=shell, env=env)
    if r.returncode != 0:
        sys.exit(f"Stopped: {title} failed.")


def notes(version):
    text = read(CHANGELOG)
    m = re.search(rf"^## {re.escape(version)}\b.*?(?=^## |\Z)", text, re.M | re.S)
    if not m:
        sys.exit(f"CHANGELOG.md has no '## {version}' section.")
    return m.group(0).strip() + "\n"


def private_prefixes():
    """The folders whose names must not end up in the program: the user's profile, Cargo's home and this repository."""
    home = os.path.expanduser("~")
    cargo = os.environ.get("CARGO_HOME") or os.path.join(home, ".cargo")
    return [("home", home), ("cargo", cargo), ("src", ROOT)]


def path_free_env():
    """The build environment with every private folder mapped to a short neutral name: --remap-path-prefix for Rust,
    and /d1trimfile for the C and C++ code (llama.cpp) built with MSVC. Computed here, so no personal path is written
    into a tracked file."""
    env = os.environ.copy()
    # rustc uses the last matching mapping, so the most specific folders come last.
    flags = [f"--remap-path-prefix={path}={name}" for name, path in sorted(private_prefixes(), key=lambda x: len(x[1]))]
    extra = env.get("CARGO_ENCODED_RUSTFLAGS")
    env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(([extra] if extra else []) + flags)
    # CFLAGS is split at spaces, so a folder with a space in its name can't be given here; the check below still runs.
    trim = " ".join(f"/d1trimfile:{path}\\" for _, path in private_prefixes() if " " not in path)
    for var in ("CFLAGS", "CXXFLAGS"):
        env[var] = (env.get(var, "") + " " + trim).strip()
    return env


def check_no_private_paths(exe):
    """Fails the build when the program still holds the user's profile path or this repository's path."""
    data = open(exe, "rb").read().lower()
    found = []
    for name, path in private_prefixes():
        if name == "cargo" and path.lower().startswith(os.path.expanduser("~").lower()):
            continue  # covered by the profile check
        for form in {path, path.replace("\\", "/"), path.replace("/", "\\")}:
            for enc in ("utf-8", "utf-16-le"):
                n = data.count(form.lower().encode(enc))
                if n:
                    found.append(f"{n} x {name} folder ({enc})")
    if found:
        sys.exit(f"Stopped: {os.path.relpath(exe, ROOT)} contains private paths: " + ", ".join(found))
    print("No private paths in the program.")


def build():
    version = check()
    conf = json.loads(read(TAURI_CONF))
    win = conf.get("bundle", {}).get("windows", {})
    if win.get("certificateThumbprint") or win.get("signCommand"):
        sys.exit("tauri.conf.json has a signing setting. This script makes unsigned builds only; signing is the product owner's step.")
    release_notes = notes(version)
    step("Rust tests", ["cargo", "test", "--workspace", "-q"])
    step("Interface tests", "npm test", cwd=os.path.join(ROOT, "app", "ui"), shell=True)
    step("Validation report", [sys.executable, os.path.join(ROOT, "tools", "validate.py")])
    step("Interface build", "npm run build", cwd=os.path.join(ROOT, "app", "ui"), shell=True)
    # The interface is built above, so Tauri's own before-build step (which resolves ../ui from the folder it was
    # started in) is switched off by tools/tauri-release.json.
    cfg = os.path.join(ROOT, "tools", "tauri-release.json")
    step("Installer (unsigned)", f'npm --prefix ../ui exec -- tauri build --config "{cfg}"', cwd=os.path.join(ROOT, "app", "src-tauri"), shell=True, env=path_free_env())
    check_no_private_paths(os.path.join(ROOT, "app", "src-tauri", "target", "release", "prismstatistics-app.exe"))
    bundle = os.path.join(ROOT, "app", "src-tauri", "target", "release", "bundle", "nsis")
    found = [f for f in os.listdir(bundle) if f.endswith("-setup.exe") and f"_{version}_" in f]
    if not found:
        sys.exit(f"No installer for {version} in {bundle}")
    out = os.path.join(ROOT, "release", version)
    os.makedirs(out, exist_ok=True)
    exe = os.path.join(out, found[0])
    shutil.copy2(os.path.join(bundle, found[0]), exe)
    digest = hashlib.sha256(open(exe, "rb").read()).hexdigest()
    write(os.path.join(out, "SHA256SUMS.txt"), f"{digest}  {found[0]}\n")
    write(os.path.join(out, "RELEASE-NOTES.md"), release_notes + "\nThis build is not code-signed: Windows shows \"Windows protected your PC\"; choose More info, then Run anyway.\n")
    shutil.copy2(os.path.join(ROOT, "docs", "validation-report.md"), os.path.join(out, "validation-report.md"))
    print(f"\nUnsigned installer: {os.path.relpath(exe, ROOT)}\nSHA-256: {digest}")


if __name__ == "__main__":
    if len(sys.argv) < 2 or sys.argv[1] not in ("check", "bump", "build"):
        sys.exit(__doc__)
    if sys.argv[1] == "check":
        check()
    elif sys.argv[1] == "bump":
        if len(sys.argv) != 3:
            sys.exit("bump needs a version, e.g. python tools/release.py bump 0.2.0")
        bump(sys.argv[2])
    else:
        build()
