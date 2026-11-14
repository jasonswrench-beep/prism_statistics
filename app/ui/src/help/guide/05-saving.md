# Saving, backups and recovery

## Projects

**File › Save project** (Ctrl+S) saves everything in one .pstat file:

- the data and the variable definitions
- every result
- the report layout
- the record of operations

**File › Save project as…** (Ctrl+Shift+S) saves a copy under a new name. Double-clicking a .pstat file opens it in PrismStatistics.

## How saving protects your file

A save never writes over your project directly. PrismStatistics first writes the new version to a temporary file beside it. It makes sure the file reached the disk and reads it back to check it. Only then does it put the new version in place.

If the computer loses power during a save, you still have either the old file or the new one. You never get a damaged mix of the two.

## Backups

Each save keeps the version it replaces beside the project as a backup. For example, saving `study.pstat` keeps the previous version as `study.backup.pstat`. If you saved something by mistake, open the backup with **File › Open project…**.

Only one backup is kept. A damaged file is never kept as the backup.

## Recovering unsaved work

While a project has unsaved changes, PrismStatistics copies it to a recovery folder in your user profile every half minute. If PrismStatistics or the computer stops before you save, the next start offers that copy under **Recover unsaved work**:

- **Open** brings the work back as unsaved. Save it where you want to keep it.
- **Discard** deletes the copy.
- **Decide later** leaves the copy for the next start.

Saving the project removes its recovery copy. Closing with unsaved changes keeps it, so work closed by mistake can be recovered too.
