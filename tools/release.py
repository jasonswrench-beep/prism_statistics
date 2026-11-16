"""Release steps for PrismStatistics: version checks, the full check run, and an UNSIGNED Windows installer.

    python tools/release.py check             # the version is the same everywhere it is written
    python tools/release.py bump 0.2.0        # set a new version everywhere
    python tools/release.py build             # tests, validation report, UI build, then the unsigned installer
    python tools/release.py build --sign      # the same, with the program and installer signed and timestamped
    python tools/release.py build --core pstat-v1.2.0            # also run the shared core's checks on the build
    python tools/release.py build --core <openstage-core clone>  # the same, from a clone
    python tools/release.py build --sign --updates   # also sign the installer for Help > Check for Updates and write latest.json

`--updates` (docs/UPDATES.md) needs the update key that tools/make-update-key.bat made: %USERPROFILE%\.prismstatistics-update.key
(or TAURI_SIGNING_PRIVATE_KEY_PATH), with its password from TAURI_SIGNING_PRIVATE_KEY_PASSWORD or asked for. It stops before
building when the key is missing or when PUBKEY in app/src-tauri/src/updates.rs is empty or is not the key's public half (such a
build could never update anything). After the code signature, it signs the installer with the update key (the update signature
covers the exact bytes people download) and writes release/<version>/latest.json, plus previous.json (the outgoing release's
latest.json, for Return to the previous version). Attach both to the release v<version> with the installer.

`--core` names a release tag of the public openstage-core repository (its source is fetched from GitHub into a
temporary folder) or a clone of it. With it, the build also runs the core's .pstat reader conformance check
(specs/pstat/conformance/check_reader.mjs) and its item-analysis check (testsets/item-analysis/check/
check_prismstatistics.mjs) against the program just built, without changing it. Without it those two are reported as
skipped; the item-analysis test set itself always runs, from the copy in the Rust reference tests (c3_reference).

`build` stops at the first failure. It maps the build folders out of the program (--remap-path-prefix, /d1trimfile) and
fails if the program still holds the user profile or repository path. It writes the installer, its SHA-256 checksum and the release notes for this version
(from CHANGELOG.md) to release/<version>/. Signing is a deliberate choice: only `--sign` signs, through
tools/sign_windows.py (Azure Artifact Signing, every signature timestamped). Its account names come from the
OPENSTAGE_SIGN_* environment variables and the sign-in from `az login`, so nothing about the account is in the
repository; tauri.conf.json itself never holds a signing setting, and the build refuses to run if it does.
"""

import hashlib
import io
import json
import tarfile
import tempfile
import urllib.request
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


SIGN_SCRIPT = os.path.join(ROOT, "tools", "sign_windows.py")


def tauri_config(sign):
    """The build's Tauri settings: tools/tauri-release.json, plus, for a signed build, a sign command that Tauri runs for
    each file it signs (the program, the installer's plugins, the uninstaller and the setup file)."""
    cfg = json.loads(read(os.path.join(ROOT, "tools", "tauri-release.json")))
    if sign:
        cfg.setdefault("bundle", {}).setdefault("windows", {})["signCommand"] = {"cmd": sys.executable, "args": ["-I", SIGN_SCRIPT, "--description", "PrismStatistics", "%1"]}
    path = os.path.join(ROOT, "target", "release-config", "tauri-signed.json" if sign else "tauri-unsigned.json")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    write(path, json.dumps(cfg, indent=2) + "\n")
    return path


CORE_REPO = "jasonswrench-beep/openstage-core"


def core_at_tag(tag):
    """The core's source at a release tag, unpacked into a temporary folder; returns the folder."""
    if not re.fullmatch(r"[a-z][a-z0-9-]*-v\d+\.\d+\.\d+", tag):
        sys.exit(f"{tag} is neither a folder nor a core release tag (such as pstat-v1.2.0).")
    url = f"https://github.com/{CORE_REPO}/archive/refs/tags/{tag}.tar.gz"
    print(f"\nFetching the shared core at {tag}")
    with urllib.request.urlopen(url, timeout=120) as r:
        data = r.read()
    dest = tempfile.mkdtemp(prefix="openstage-core-")
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as t:
        t.extractall(dest, filter="data")
    tops = [d for d in os.listdir(dest) if os.path.isdir(os.path.join(dest, d))]
    if len(tops) != 1 or not os.path.isfile(os.path.join(dest, tops[0], "CORE-NOTES.md")):
        sys.exit(f"The archive for {tag} is not the core's source.")
    return os.path.join(dest, tops[0])


UPDATES_RS = os.path.join(ROOT, "app", "src-tauri", "src", "updates.rs")
PUBLIC_REPO = "jasonswrench-beep/prism_statistics"


def update_key():
    """The update key's file, checked against the public key built into the program; stops the build when anything is off."""
    key = os.environ.get("TAURI_SIGNING_PRIVATE_KEY_PATH") or os.path.join(os.path.expanduser("~"), ".prismstatistics-update.key")
    if not os.path.isfile(key):
        sys.exit(f"--updates: there is no update key at {key} (tools/make-update-key.bat makes it).")
    if not os.path.isfile(key + ".pub"):
        sys.exit(f"--updates: the public half of the update key is not beside it ({key}.pub).")
    m = re.search(r'pub const PUBKEY: &str = "([^"]*)";', read(UPDATES_RS))
    if not m or not m.group(1):
        sys.exit("--updates: PUBKEY in app/src-tauri/src/updates.rs is empty, so the program would not check for updates (tools/make-update-key.ps1 puts it there).")
    if m.group(1) != read(key + ".pub").strip():
        sys.exit("--updates: PUBKEY in updates.rs is not the public half of the update key; a program built like that would refuse every update.")
    if not os.environ.get("TAURI_SIGNING_PRIVATE_KEY_PASSWORD"):
        import getpass
        os.environ["TAURI_SIGNING_PRIVATE_KEY_PASSWORD"] = getpass.getpass("Password for the update key (Enter if it has none): ")
    return key


def update_feed(exe, key, version, release_notes):
    """Signs the finished installer with the update key and writes latest.json beside it."""
    cli = os.path.join(ROOT, "app", "ui", "node_modules", ".bin", "tauri.cmd" if os.name == "nt" else "tauri")
    step("Update signature", [cli, "signer", "sign", "-f", key, exe])
    sig = exe + ".sig"
    if not os.path.isfile(sig):
        sys.exit("The update signature was not written.")
    manifest = {
        "version": version,
        "notes": release_notes.strip(),
        "pub_date": __import__("datetime").datetime.now(__import__("datetime").timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": {"windows-x86_64": {"signature": read(sig).strip(), "url": f"https://github.com/{PUBLIC_REPO}/releases/download/v{version}/{os.path.basename(exe)}"}},
    }
    feed = os.path.join(os.path.dirname(exe), "latest.json")
    write(feed, json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")
    # The outgoing release's feed becomes previous.json, which Return to the previous version reads.
    previous = os.path.join(os.path.dirname(exe), "previous.json")
    if os.path.exists(previous):
        os.remove(previous)
    try:
        with urllib.request.urlopen(f"https://github.com/{PUBLIC_REPO}/releases/latest/download/latest.json", timeout=30) as r:
            old = json.loads(r.read().decode("utf-8"))
        if old.get("version") and old["version"] != version:
            write(previous, json.dumps(old, indent=2, ensure_ascii=False) + "\n")
            print(f"previous.json points at version {old['version']}.")
    except Exception:
        print("No published latest.json yet, so no previous.json (normal for the first release with updates).")
    print(f"Wrote {feed}: attach it (and previous.json, if written) to the release v{version} with the installer (docs/UPDATES.md).")


def build(sign=False, core=None, updates=False):
    version = check()
    conf = json.loads(read(TAURI_CONF))
    win = conf.get("bundle", {}).get("windows", {})
    if win.get("certificateThumbprint") or win.get("signCommand"):
        sys.exit("tauri.conf.json has a signing setting. Signing is chosen per build with --sign, not kept in the configuration.")
    if sign:
        step("Signing: checking everything it needs", [sys.executable, "-I", SIGN_SCRIPT, "--ready"])
    key = update_key() if updates else None
    if updates and not sign:
        sys.exit("--updates needs --sign: an update installer is code-signed too.")
    release_notes = notes(version)
    # Not a stop: it needs GitHub, and a change there is read and adopted by hand (decision D03).
    drift = subprocess.run([sys.executable, os.path.join(ROOT, "tools", "check_shared_stats.py")])
    if drift.returncode:
        print("\nNote: PrismQDA's statistics crate has changed since the pin (above). The build goes on with the pinned copy.")
    step("Rust tests", ["cargo", "test", "--workspace", "-q"])
    step("Interface tests", "npm test", cwd=os.path.join(ROOT, "app", "ui"), shell=True)
    step("Validation report", [sys.executable, os.path.join(ROOT, "tools", "validate.py")])
    step("Interface build", "npm run build", cwd=os.path.join(ROOT, "app", "ui"), shell=True)
    # The interface is built above, so Tauri's own before-build step (which resolves ../ui from the folder it was
    # started in) is switched off by tools/tauri-release.json.
    cfg = tauri_config(sign)
    step("Installer (signed)" if sign else "Installer (unsigned)", f'npm --prefix ../ui exec -- tauri build --config "{cfg}"', cwd=os.path.join(ROOT, "app", "src-tauri"), shell=True, env=path_free_env())
    program = os.path.join(ROOT, "app", "src-tauri", "target", "release", "prismstatistics-app.exe")
    check_no_private_paths(program)
    if core:
        step("Shared core: .pstat reader conformance", ["node", os.path.join(core, "specs", "pstat", "conformance", "check_reader.mjs"), "--prism", program], cwd=core)
        step("Shared core: item-analysis test set", ["node", os.path.join(core, "testsets", "item-analysis", "check", "check_prismstatistics.mjs"), "--prism", program], cwd=core)
    else:
        print("\nSkipped: the shared core's conformance and item-analysis checks on the build (give --core <release tag such as pstat-v1.2.0, or a clone>).")
    bundle = os.path.join(ROOT, "app", "src-tauri", "target", "release", "bundle", "nsis")
    found = [f for f in os.listdir(bundle) if f.endswith("-setup.exe") and f"_{version}_" in f]
    if not found:
        sys.exit(f"No installer for {version} in {bundle}")
    out = os.path.join(ROOT, "release", version)
    os.makedirs(out, exist_ok=True)
    exe = os.path.join(out, found[0])
    shutil.copy2(os.path.join(bundle, found[0]), exe)
    if sign:
        step("Signing: checking the installer", [sys.executable, "-I", SIGN_SCRIPT, "--check", exe])
    if key:
        update_feed(exe, key, version, release_notes)
    digest = hashlib.sha256(open(exe, "rb").read()).hexdigest()
    write(os.path.join(out, "SHA256SUMS.txt"), f"{digest}  {found[0]}\n")
    signed_note = "The installer and the program are code-signed and timestamped." if sign else "This build is not code-signed: Windows shows \"Windows protected your PC\"; choose More info, then Run anyway."
    write(os.path.join(out, "RELEASE-NOTES.md"), release_notes + "\n" + signed_note + "\n")
    shutil.copy2(os.path.join(ROOT, "docs", "validation-report.md"), os.path.join(out, "validation-report.md"))
    print(f"\n{'Signed' if sign else 'Unsigned'} installer: {os.path.relpath(exe, ROOT)}\nSHA-256: {digest}")


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
        extra = sys.argv[2:]
        sign = "--sign" in extra
        updates = "--updates" in extra
        core = None
        rest = [a for a in extra if a not in ("--sign", "--updates")]
        if rest[:1] == ["--core"] and len(rest) == 2:
            if os.path.isdir(rest[1]):
                core = os.path.abspath(rest[1])
                if not os.path.isfile(os.path.join(core, "CORE-NOTES.md")):
                    sys.exit(f"{core} is not a clone of openstage-core (no CORE-NOTES.md).")
            else:
                core = core_at_tag(rest[1])
        elif rest:
            sys.exit("build takes --sign, --updates and --core <core release tag or clone>, e.g. python tools/release.py build --sign --updates --core pstat-v1.2.0")
        build(sign=sign, core=core, updates=updates)
