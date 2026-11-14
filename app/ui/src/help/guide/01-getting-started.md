# Getting started

PrismStatistics runs the statistics taught in most courses and used in most research. It explains each result in plain language and writes an APA-style paragraph you can edit. Every analysis is checked against R in the test suite.

## The first five minutes

No data of your own yet? Choose **File › Open Example Data…** and start with a [worked example](topic:two-groups).

1. **Open data.** Choose **Open data** on the Home screen or **File › Open data…** (Ctrl+O). The data appear in **Data View**.
2. **Check the variables.** Switch to **Variable View** to give each variable a label, value labels (1 = "Female"), missing-value codes (−99) and a measure (nominal, ordinal or scale). **Data › Data Health…** looks for common problems first.
3. **Run an analysis.** Choose one from the **Analyze** menu, move variables into the boxes, pick options, and choose **Run**. The result appears on the **Output** screen. Not sure which analysis? **Help me choose** asks about your design and suggests one ([Choosing an analysis](topic:choosing)).
4. **Read the result.** Each result shows its tables and figures, the checks it made, an APA paragraph, and a method note naming exactly what was computed.
5. **Save the project** with **File › Save project** (Ctrl+S). A project (.pstat) keeps the data, every result, and the record of how each result was made.

Analyses use the Variable View settings: a value marked missing is left out, and a nominal variable is treated as groups.

## Files PrismStatistics opens

- CSV and TSV
- spreadsheets (.xlsx, .xlsm)
- .sav and .zsav, .dta, .sas7bdat and .jasp files
- online spreadsheets, through a shared link (**File › Import › Online spreadsheet**)

## The screens

- **Home**: open data or a project, with shortcuts to common analyses.
- **Data**: Data View (cases in rows, variables in columns) and Variable View (one row per variable).
- **Output**: every result, newest last.
- **Report**: arranges results, headings and your own text into a document to export. See [Reports and exporting](topic:exporting).
- **Script**: commands pasted from dialogs, run together or saved. See [Scripts and reproducibility](topic:scripts).

## Help while you work

Every dialog has a **?** button in its title bar, and **F1** opens the help for the dialog in front. **Help › PrismStatistics Help** opens this guide. The search box finds any word in any page.
