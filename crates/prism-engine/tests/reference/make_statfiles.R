# Writes the statistical-file fixtures for the import tests (prism-engine/tests/statfile_reference.rs) and, from
# reading them back, the reference values in statfiles_reference.json. Uses the haven package (CRAN).
# Run from this folder: Rscript make_statfiles.R
#
# The same eight cases go into every format: sex with value labels and 9 as a user-missing code (.sav only), age with
# 90 to 99 as a user-missing range (.sav only), a score with one empty cell, a name with one empty string, and a date.
suppressMessages(library(haven))
d <- data.frame(
  id = 1:8,
  sex = c(1, 2, 2, 1, 9, 1, 2, 1),
  age = c(19, 22, 99, 20, 21, 35, 28, 24),
  score = c(4.5, 3.25, NA, 5, 2.75, 4, 3.5, 4.75),
  name = c("Ana", "Ben", "", "Dee", "Eli", "Fay", "Gus", "Hal"),
  visit = as.Date("2026-01-05") + c(0, 1, 2, 3, NA, 5, 6, 7),
  stringsAsFactors = FALSE
)
labs <- c(Female = 1, Male = 2, `No answer` = 9)

s <- d
s$sex <- labelled_spss(d$sex, labels = labs, na_values = 9, label = "Sex of participant")
s$age <- labelled_spss(d$age, na_range = c(90, 99), label = "Age in years")
attr(s$score, "label") <- "Quiz score"
write_sav(s, "statfile.sav")

t <- d
t$sex <- labelled(d$sex, labels = labs, label = "Sex of participant")
attr(t$age, "label") <- "Age in years"
attr(t$score, "label") <- "Quiz score"
write_dta(t, "statfile.dta", version = 15)

# The .sas7bdat writer in haven is marked experimental; the import test also reads a fixture shipped with the reader.
u <- d
attr(u$age, "label") <- "Age in years"
attr(u$score, "label") <- "Quiz score"
suppressWarnings(write_sas(u, "statfile.sas7bdat"))

# Read each file back with haven and record what it holds.
num <- function(x) ifelse(is.na(x), "null", sprintf("%.17g", x))
str <- function(x) ifelse(is.na(x), "null", paste0('"', x, '"'))
col <- function(x) {
  if (inherits(x, "Date")) return(paste0("[", paste(str(format(x)), collapse = ", "), "]"))
  if (is.character(x)) return(paste0("[", paste(str(x), collapse = ", "), "]"))
  paste0("[", paste(num(as.numeric(x)), collapse = ", "), "]")
}
block <- function(name, df) {
  cols <- vapply(names(df), function(n) sprintf('    "%s": %s', n, col(df[[n]])), "")
  lab <- vapply(names(df), function(n) { l <- attr(df[[n]], "label"); sprintf('"%s": %s', n, if (is.null(l)) "null" else paste0('"', l, '"')) }, "")
  sprintf('  "%s": {\n    "_labels": {%s},\n%s\n  }', name, paste(lab, collapse = ", "), paste(cols, collapse = ",\n"))
}
blocks <- c(
  block("sav", read_sav("statfile.sav", user_na = TRUE)),
  block("dta", read_dta("statfile.dta")),
  block("sas7bdat", read_sas("statfile.sas7bdat"))
)
writeLines(c("{", paste0('  "_made_with": "', R.version.string, ', haven ', packageVersion("haven"), '",'), paste(blocks, collapse = ",\n"), "}"), "statfiles_reference.json")
