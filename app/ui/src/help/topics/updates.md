# Check for Updates

Finds out whether a newer version of PrismStatistics has been published, and installs it if you want it.

## When to use it
- When you want the newest version, or want to go back to the one before.

## How to run it
Choose **Help › Check for Updates**, then **Check now**. PrismStatistics asks the project's newest release on github.com whether it is a newer version. The request carries only what any web request carries (the program's version and the kind of computer), nothing about your data or projects.

**Check for updates once a day**, off unless you turn it on, makes the same check at most once a day and only tells you when a newer version is out; nothing is downloaded until you choose to.

If there is a newer version, the dialog shows its number and what's new. **Download and install** is a second step:
1. The installer is downloaded.
2. Its signature is checked against the maintainer's key, which is built into PrismStatistics. A file whose signature doesn't match is refused, and nothing is installed.
3. Unsaved work is copied to the recovery folder, so it is offered again when PrismStatistics next starts.
4. PrismStatistics closes and the installer runs, for your account only (no administrator rights).

**Return to the previous version** offers the release before this one. It is checked and installed the same way, and only when you confirm.

## Notes
- A copy that is not set up for updates says so and connects to nothing; download new versions from the project's releases page instead.
- In a web browser (the test server) updates can't be checked; use the installed program.
- Every update installer is also code-signed, so Windows shows who published it.
