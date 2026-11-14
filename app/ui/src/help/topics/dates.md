# Date and Time

Makes numeric variables from dates: their parts (year, month, day of the week …) or the time between two dates, such as an age.

## When to use it
- When dates came in as text (2026-11-14) or as the numbers other programs store dates as, and you need something you can analyze, such as the month of an enrollment or an age at a visit.

## How to run it
Choose **Transform › Date and Time…**. Choose **Parts of a date** or **Time between two dates**, the date variable (or the start and end dates), and how the dates are stored. Then choose what to make.

## Options
- **The dates are stored as**:
  - text with the year first (2026-11-14, 2026/11/14 13:05), the month first (11/14/2026) or the day first (14/11/2026)
  - spreadsheet date numbers (days since 30 December 1899)
  - seconds since 14 October 1582, as .sav files store dates
  - days or seconds since 1 January 1960, as .dta and .sas7bdat files store dates
- **Parts**: each makes `<name>_<part>`:
  - year
  - month
  - day of the month
  - day of the week (1 = Monday to 7 = Sunday, with value labels)
  - day of the year
  - hour
  - minute
  - day number (days since 1970-01-01, a plain number to sort or subtract)
- **Time between two dates**:
  - **Whole years completed (default)**, counted as an age is: the day before a birthday is not yet the birthday.
  - **Whole months completed**.
  - **Weeks** or **Days**, with a fraction for the time of day.

## What you get
- New numeric variables, and a message saying how many cases got a value.
- Text that isn't a real date, such as 31 February or a two-digit year, is left missing. The message counts those cells and shows one.
- The end date before the start date gives a negative time.

## Checked against
R's `as.POSIXlt` (year, month, day, weekday, day of the year, hour, minute) and `difftime`, with whole months counted as R does.
