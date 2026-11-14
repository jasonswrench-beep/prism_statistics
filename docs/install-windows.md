# Installing PrismStatistics on Windows

The teaching preview ships as one installer file, `PrismStatistics_<version>_x64-setup.exe`.

## Installing

1. Double-click the installer.
2. The installer and the program are code-signed. A new download can still show **"Windows protected your PC"** until enough people have installed it; check that the publisher shown is the one on the release page, then choose **More info** and **Run anyway**. You can also check the file against the SHA-256 on the release page.
3. Accept the license (Apache 2.0) and finish. No administrator rights are needed: PrismStatistics installs for the current user only, under `%LOCALAPPDATA%\PrismStatistics`.

The installer adds PrismStatistics to the Start menu (in a **PrismOpenResearch** folder) and makes `.pstat` project files open in PrismStatistics when double-clicked.

PrismStatistics draws its window with the web view built into current versions of Windows 10 and 11. If a computer lacks it, the installer downloads it.

## Connecting an AI assistant (optional)

AI assistants that support the Model Context Protocol (MCP) can use PrismStatistics to open data, run analyses and export results. It is off by default.

1. In PrismStatistics, choose **Tools › Connect an AI assistant** and tick **Allow AI assistants to use PrismStatistics**. Leave **read individual data rows** off unless the assistant needs to see single cases.
2. Choose **Copy** and paste the setting into your assistant's MCP configuration (or add a local "stdio" server with the command and the `--mcp` argument shown). Restart the assistant.

Assistants that show MCP prompts offer three ready-made ones: analyze and report in APA style, check a write-up against the results, and explain a result in plain language.

The assistant can write files only to new paths on this computer: it can't replace an existing file or use a network path. Text from your data (variable names, labels, values) is passed to it marked as data, not instructions.

The assistant starts its own copy of PrismStatistics in the background, with no window. To see its results in the window, ask it to save a project and open the `.pstat` file. An assistant that runs online sends what it reads (variable names, labels and results, plus rows if you allowed them) to its provider.

## Building a report

The **Report** screen arranges what goes into an exported document: which results, in what order, which of their parts (figures, tables, checks, paragraph, method note), and your own headings and text between them (write `*word*` for italics). The arrangement is saved with the project. **Export to Word**, **HTML** or **PDF** on that screen writes the report with the same APA formatting as the Output exports, numbering tables and figures in order.

## Scripts and the batch runner

Every analysis dialog has a **Paste** button. It adds the analysis to the **Script** screen (on the left) instead of running it. A script is plain text, one command per line, the same commands a project records. **Run all** runs them in order and stops at the first problem, naming its line. **Add the project's history** turns everything done so far into a script, and **Save…** writes it as a `.pscript` file.

A saved script also runs without the window, from a terminal:

```
"%LOCALAPPDATA%\PrismStatistics\PrismStatistics.exe" run analysis.pscript --data mydata.sav --out report.docx
```

`--data` opens a data file first, `--out` writes the results as a Word, HTML or PDF report, and `--project` saves them as a `.pstat` project. The APA paragraphs are printed as each result is made.

## Exporting an R script

**File › Export › R script…** (or **R script** on the Output page) writes the project's analyses as an R script: it reads the original data file, sets the user-missing codes to `NA` the way PrismStatistics does, and runs each analysis in order with base R (`haven` reads `.sav`, `.dta` and `.sas7bdat` files, `readxl` reads workbooks, and `pwr` covers two power analyses). The R functions are the ones PrismStatistics is tested against, so the numbers agree.

## Updating and removing

Run a newer installer over the old one; projects and settings are kept. To remove PrismStatistics, use **Settings › Apps › Installed apps › PrismStatistics › Uninstall**. Your `.pstat` projects and data files are never deleted.

The program's own files live in two folders under `%LOCALAPPDATA%`:

- `PrismStatistics`: the program. The uninstaller always removes it.
- `io.github.jasonswrench-beep.prismstatistics`: the AI writer's model (about 2.5 GB, if you downloaded it), settings, and copies kept for crash recovery. The uninstaller removes this folder when you tick **Delete the application data**; leave the box clear to keep the model for a later install.

## Building the installer

From `app/`, with Rust and Node.js installed and `npm ci` run in `app/ui`:

```
ui/node_modules/.bin/tauri build
```

The installer is written to `app/src-tauri/target/release/bundle/nsis/`. The build downloads the NSIS tools the first time.

## Not done yet

- **Code signing.** `python tools/release.py build --sign` signs and timestamps the program, the installer's parts and the installer with Azure Artifact Signing, then checks the installer's signature. The account names come from the `OPENSTAGE_SIGN_*` environment variables and the sign-in from `az login`; `python tools/sign_windows.py --ready` says whether everything it needs is there.
- **macOS or web version** for spring 2027 (D12).
- **Automatic updates.**
