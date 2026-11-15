# Scripts and reproducibility

Everything PrismStatistics does to a project is recorded as an operation: opening data, each change to the data, and each analysis with its settings. This record is saved with the project. It is what lets results be marked out of date and run again.

## The Script screen

Every analysis dialog has a **Paste** button. It adds the analysis to the **Script** screen instead of running it.

A script is plain text with one command per line: the command, then its settings. Lines starting with `#` are comments.

- **Run all** runs the commands in order. It stops at the first problem and names its line.
- **Add the project's history** turns everything done so far into a script.
- **Save…** writes the script as a `.pscript` file.

## Running a script without the window

A saved script also runs from a terminal. This is useful for running the same analyses on many data files, or as a scheduled job:

```
"%LOCALAPPDATA%\PrismStatistics\prismstatistics-app.exe" run analysis.pscript --data mydata.sav --out report.docx
```

The program file is `prismstatistics-app.exe`, in the folder PrismStatistics was installed to (`%LOCALAPPDATA%\PrismStatistics` unless you chose another).

- `--data` opens a data file first.
- `--out` writes the results as a Word, HTML or PDF report.
- `--project` saves the results as a .pstat project.

## AI assistants

**Tools › Connect an AI assistant…** lets an assistant that supports the Model Context Protocol use PrismStatistics to open data, run analyses and export results. It is off by default. Letting the assistant read individual data rows is a separate permission.
