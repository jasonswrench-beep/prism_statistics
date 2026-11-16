# The cases of the item-analysis test set (C3): one folder per case, each with scores.csv and case.json.
# Run from this folder: Rscript make_cases.R
# Uses only base R. The cases are made once and kept in the repository; make_reference.R reads the files, not this
# script, so a change in R's random numbers can never change the reference values without a new commit.
#
# scores.csv: one row per complete answer sheet. sheet is the sheet's number, version is the test version (1 = A),
# then one column per question with the points earned. An empty cell means the question does not count on that sheet
# (an adjustment for one version), which is not the same as 0.
# case.json: what each question is worth and whether it gives partial credit.

set.seed(20261115)
q <- function(k) sprintf("q%02d", seq_len(k))

write_case <- function(name, description, scores, versions, possible, partial = rep(FALSE, ncol(scores)),
                       not_counted = list()) {
  dir.create(file.path("cases", name), recursive = TRUE, showWarnings = FALSE)
  colnames(scores) <- q(ncol(scores))
  d <- data.frame(sheet = seq_len(nrow(scores)), version = versions, scores, check.names = FALSE)
  for (item in names(not_counted)) d[d$version %in% not_counted[[item]], item] <- NA
  # Binary connections, so the files have the same line endings (LF) on every system.
  con <- file(file.path("cases", name, "scores.csv"), "wb")
  write.csv(d, con, row.names = FALSE, quote = FALSE, na = "")
  close(con)
  items <- vapply(seq_len(ncol(scores)), function(i) {
    nc <- not_counted[[q(ncol(scores))[i]]]
    extra <- if (is.null(nc)) "" else sprintf(', "not_counted_on_versions": [%s]', paste(nc, collapse = ", "))
    sprintf('    {"id": "%s", "possible": %s, "partial_credit": %s%s}', q(ncol(scores))[i], format(possible[i]),
            tolower(partial[i]), extra)
  }, "")
  json <- c("{", sprintf('  "case": "%s",', name), sprintf('  "description": "%s",', description),
            sprintf('  "sheets": %d,', nrow(scores)), sprintf('  "versions": %d,', max(versions)),
            '  "items": [', paste(items, collapse = ",\n"), "  ]", "}")
  con <- file(file.path("cases", name, "case.json"), "wb")
  writeLines(json, con)
  close(con)
}

rasch <- function(n, b, negative = integer()) {
  theta <- rnorm(n)
  p <- plogis(outer(theta, b, "-"))
  for (j in negative) p[, j] <- plogis(-theta - b[j])
  matrix(rbinom(length(p), 1, p), n, length(b))
}

# 1. Twelve sheets, six right-or-wrong questions, written by hand. The totals tie across both 27 percent group
# boundaries (the groups hold three sheets each), and question 6 is answered right mostly by low scorers.
small <- matrix(c(
  1, 1, 1, 1, 1, 0,
  1, 1, 1, 1, 0, 1,
  1, 1, 0, 1, 1, 0,
  1, 0, 1, 1, 1, 0,
  1, 1, 1, 0, 0, 0,
  0, 1, 1, 1, 0, 0,
  1, 0, 1, 0, 0, 1,
  1, 1, 0, 0, 0, 0,
  0, 1, 0, 0, 0, 0,
  1, 0, 0, 0, 0, 0,
  0, 0, 1, 0, 0, 0,
  0, 0, 0, 0, 0, 1), ncol = 6, byrow = TRUE)
write_case("small-ties", "Twelve sheets, six right-or-wrong questions; totals tie at both group boundaries.",
           small, rep(1:2, 6), rep(1, 6))

# 2. Forty sheets, fifteen right-or-wrong questions in four versions; question 15 discriminates the wrong way.
write_case("class-40", "Forty sheets, fifteen right-or-wrong questions, four versions; q15 discriminates negatively.",
           rasch(40, seq(-2, 2, length.out = 15), negative = 15L), rep(1:4, 10), rep(1, 15))

# 3. Thirty sheets, ten right-or-wrong questions; q09 and q10 are worth two points each (0 or 2).
w <- rasch(30, seq(-1.5, 1.5, length.out = 10))
w[, 9:10] <- 2 * w[, 9:10]
write_case("weighted-30", "Thirty sheets, ten right-or-wrong questions; q09 and q10 are worth two points.",
           w, rep(1:2, 15), c(rep(1, 8), 2, 2))

# 4. Thirty-five sheets, eight questions; q07 and q08 give partial credit (0, 0.5 or 1 of 1 point).
theta <- rnorm(35)
pc <- matrix(rbinom(35 * 6, 1, plogis(outer(theta, seq(-1, 1, length.out = 6), "-"))), 35, 6)
part <- sapply(c(-0.5, 0.5), function(b) rbinom(35, 2, plogis(theta - b)) / 2)
write_case("partial-35", "Thirty-five sheets, eight questions; q07 and q08 give partial credit in halves.",
           cbind(pc, part), rep(1:3, length.out = 35), rep(1, 8), c(rep(FALSE, 6), TRUE, TRUE))

# 5. Twenty sheets, five questions; everyone got q03 right and everyone got q05 wrong.
cst <- rasch(20, c(-1, 0, 0, 1, 0))
cst[, 3] <- 1
cst[, 5] <- 0
write_case("constant-20", "Twenty sheets, five questions; q03 right on every sheet, q05 wrong on every sheet.",
           cst, rep(1, 20), rep(1, 5))

# 6. A hundred and fifty sheets, ten questions. 27 percent of 150 is 40.5, so the group size shows the rounding rule:
# half rounds up, to 41 (rounding half to even would give 40).
write_case("rounding-150", "A hundred and fifty sheets, ten questions; 27 percent of 150 is 40.5, which rounds up to 41.",
           rasch(150, seq(-1.5, 1.5, length.out = 10)), rep(1:2, 75), rep(1, 10))

# 7. Thirty sheets, six questions in two versions; q04 does not count on version 2 (an adjustment), so its cells there
# are empty and each sheet's total leaves it out.
write_case("adjusted-30", "Thirty sheets, six questions; q04 does not count on version 2, so its cells there are empty.",
           rasch(30, seq(-1, 1, length.out = 6)), rep(1:2, 15), rep(1, 6), not_counted = list(q04 = 2))
