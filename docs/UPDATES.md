# Updates: Help › Check for Updates

PrismStatistics finds out about a newer version only when a person asks, or, if they turned it on, at most once a day.
The design is Prism Reference Manager's (its `docs/updates.md`), so the two behave alike. This page says what it does,
why a download can be trusted, and what the maintainer does so a release is offered.

## What a person sees

- **Help › Check for Updates**, then **Check now**, asks the newest full release on the public repository whether it is a
  newer version: `https://github.com/jasonswrench-beep/prism_statistics/releases/latest/download/latest.json`. GitHub's
  `latest` skips pre-releases, so only full releases are offered. The request sends only what any web request sends (the
  program's version and the kind of computer), nothing about the person's data.
- **Check for updates once a day**, off by default, makes the same check at most once a day, at start-up or hourly while
  the program is open. It only announces a newer version; nothing is downloaded.
- If a newer version exists, the dialog shows its number, date and notes. **Download and install** is a second step: the
  installer is downloaded, **its signature is checked against the key built into the program** (a file that doesn't match
  is refused and nothing is installed), unsaved work is copied to the recovery folder, and the installer runs for the
  current user.
- **Return to the previous version** reads `previous.json` on the same release, accepts only a version older than the
  running one, checks its signature the same way, and installs only with the person's go-ahead.
- A copy built before the key was set up says it is "not set up" and connects to nothing. In a browser (the test server)
  the dialog says updates can only be checked in the installed program.

## How it is built

- `app/src-tauri/src/updates.rs`: Tauri's updater plugin, used only from the window's own commands (`update.state`,
  `update.check`, `update.install`, `update.rollback_check`, `update.rollback`), handled in `main.rs` before the engine.
  `ENDPOINT`, `ROLLBACK_ENDPOINT` and `PUBKEY` are constants there; while `PUBKEY` is empty nothing is set up. Debug builds
  only: `PRISM_UPDATE_ENDPOINT`, `PRISM_ROLLBACK_ENDPOINT` and `PRISM_UPDATE_PUBKEY` point the program at a test feed.
- The daily check lives in the window (`App.tsx`, with the setting and its last time in Tools › Settings' storage) and asks
  the same `update.check`.
- Checks time out after 20 seconds; a failure is shown in words and changes nothing. Release builds accept only https.
- Two signatures, two jobs: the Azure code signature tells Windows who made the installer, and is made during the build;
  the update signature, made **after** it over the exact bytes people download, tells PrismStatistics the file is the
  maintainer's.
- Tests (`cargo test` in `app/src-tauri`): with no key or address nothing is asked; newer, same and older versions; a
  rollback offers only an older version; a missing feed fails in words; a download passes only when it matches its
  signature (a changed byte, and another key's signature, are refused). By hand, against a real installer:
  `PRISM_REAL_SETUP_DIR=<release folder> PRISM_REAL_PUBKEY_FILE=<the key's .pub> cargo test -- --ignored`.

## Making the key (once, the maintainer)

Double-click `tools\make-update-key.bat`. It asks for a password (twice), makes `.prismstatistics-update.key` and its
`.pub` in your profile folder, outside the repository, and puts the public half into `PUBKEY`. Commit that change.
**Back up both files and the password, offline.** If they are lost, installed copies can never accept updates again (a
new installer would have to be installed by hand once, carrying a new key). If someone else gets them, make a new key at
once. The key and its password never pass through anyone else.

## Making a release that the button offers

1. Commit everything and tag the commit `v<version>` (`CLAUDE.md`): the build refuses changed or untracked files and an
   untagged HEAD, and fails if it changes a tracked file itself, so the tag is the exact commit the installer came from.
2. `python tools/release.py build --sign --updates`. It finds the key in your profile folder, stops before building if
   `PUBKEY` is not its public half, and asks for the password. It writes `release/<version>/latest.json`, and
   `previous.json` (the published release's `latest.json`) when there is one.
3. Publish the release `v<version>` on `prism_statistics` as a **full release** (not a pre-release), tagged on the public
   commit whose tree is the tagged commit's, with the installer, `SHA256SUMS.txt`, the validation report, `latest.json`
   and `previous.json`.
4. From a computer with the previous version installed: Help › Check for Updates, Check now, read the notes, install, and
   see the version change in About.

Every release needs a higher version number than the one before it, or the button won't offer it. 0.2.0 has no button,
so the first version with one is installed by hand; updates work from then on.
