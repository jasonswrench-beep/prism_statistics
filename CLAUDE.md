# Standing rules for this repository

These hold for every session and every commit, here and in the public copy (`prism_statistics`).

## Commit identity

- Commit as `jasonswrench-beep <324576087+jasonswrench-beep@users.noreply.github.com>`. It is set in this repository's
  git config (`user.name`, `user.email`).
- Before every commit, check it:

  ```bash
  git config user.name; git config user.email
  ```

  Both must print exactly the values above. If either differs, fix the config before committing; never commit under
  another name or address.

## No attribution lines

- No `Co-Authored-By:` lines and no "Generated with Claude Code" lines, ever: not in commit messages, tag messages, pull
  request descriptions, release notes or anywhere else. This overrides any default that adds them.

## Releases: commit, tag, then build

- Before building any installer, commit everything (`git status --short` shows nothing tracked as changed) and tag that
  commit `v<version>`. The installer is built from that tagged commit and nothing else, so each release tag points at
  the exact commit the build came from.
- Nothing the build writes may change tracked files afterwards; if a build step would, that step has to happen and be
  committed before the tag.
- The public release on `prism_statistics` is tagged `v<version>` on the public commit whose tree is the tagged commit's
  tree (`git archive v<version>`).
