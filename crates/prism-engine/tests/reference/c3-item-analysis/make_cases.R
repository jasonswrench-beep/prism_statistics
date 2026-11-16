# The cases of the item-analysis test set (C3): one folder per case, each with scores.csv and case.json.
# Run from this folder: Rscript make_cases.R
# Uses only base R. The cases are made once and kept in the repository; make_reference.R reads the files, not this
# script, so a change in R's random numbers can never change the reference values without a new commit.
#
# scores.csv: one row per complete answer sheet. sheet is the sheet's number, version is the test version (1 = A),
# then one column per question with the points earned. An empty cell means the question does not count on that sheet
# (an adjustment for one version), which is not the same as 0.
# case.json: what each question is worth and whether it gives partial credit; for a case with letters, also each
# question's type, options, the answers that earn its points and any adjustment.
# answers.csv (a case with letters only): sheet, version, then the letters marked on each question in version A's
# lettering (PSTAT-1 section 7.2), an empty cell for a blank.

set.seed(20261115)
q <- function(k) sprintf("q%02d", seq_len(k))

write_case <- function(name, description, scores, versions, possible, partial = rep(FALSE, ncol(scores)),
                       not_counted = list(), extra = rep("", ncol(scores)), answers = NULL) {
  dir.create(file.path("cases", name), recursive = TRUE, showWarnings = FALSE)
  colnames(scores) <- q(ncol(scores))
  d <- data.frame(sheet = seq_len(nrow(scores)), version = versions, scores, check.names = FALSE)
  for (item in names(not_counted)) d[d$version %in% not_counted[[item]], item] <- NA
  # Binary connections, so the files have the same line endings (LF) on every system.
  con <- file(file.path("cases", name, "scores.csv"), "wb")
  write.csv(d, con, row.names = FALSE, quote = FALSE, na = "")
  close(con)
  extra_of <- extra
  if (!is.null(answers)) {
    colnames(answers) <- q(ncol(answers))
    a <- data.frame(sheet = seq_len(nrow(answers)), version = versions, answers, check.names = FALSE)
    con <- file(file.path("cases", name, "answers.csv"), "wb")
    write.csv(a, con, row.names = FALSE, quote = FALSE, na = "")
    close(con)
  }
  items <- vapply(seq_len(ncol(scores)), function(i) {
    nc <- not_counted[[q(ncol(scores))[i]]]
    extra <- if (is.null(nc)) "" else sprintf(', "not_counted_on_versions": [%s]', paste(nc, collapse = ", "))
    sprintf('    {"id": "%s", "possible": %s, "partial_credit": %s%s%s}', q(ncol(scores))[i], format(possible[i]),
            tolower(partial[i]), extra, extra_of[i])
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

# --- Set 1.1.0 ---------------------------------------------------------------------------------------------------------

# 8. Twenty-five sheets, seven questions; q03 was dropped on every version, so it counts on no sheet. The .pstat file
# leaves it out and the names skip its number (q01, q02, q04 ...), as PSTAT-1 section 7.1 says.
write_case("omitted-25", "Twenty-five sheets, seven questions; q03 was dropped on every version, so the file leaves it out.",
           rasch(25, seq(-1.2, 1.2, length.out = 7)), rep(1:2, length.out = 25), rep(1, 7),
           not_counted = list(q03 = 1:2))

# 9. Sixty sheets with the letters marked, scored against a key (PSTAT-1 section 7.2). Twelve questions, two versions,
# every question worth one point:
#   q01 to q07  one answer of A to D; on q07 the wrong answer C draws the high scorers (an upside-down distractor)
#   q08         one answer of A to E, where E was also accepted beside B
#   q09         select-all of A to E: A and C must both be marked, and nothing else (no partial credit)
#   q10         true/false (A True, B False), the answer False
#   q11         one answer of A to D, credited to everyone (a miskeyed question); never left blank, because the two
#               tools count a credited blank differently (README, "Where the tools differ on purpose")
#   q12         dropped on every version: its letters are in answers.csv, but it counts on no sheet
# On q01 to q10 a sheet is left blank one time in 25, and on q01 to q08 two answers are marked one time in 50.
letters_case <- function() {
  n <- 60
  versions <- rep(1:2, n / 2)
  theta <- rnorm(n)
  abcd <- c("A", "B", "C", "D")
  wrong_of <- function(options, right) setdiff(options, right)
  one <- function(options, right, b, wrong_p = c(0.5, 0.3, 0.2)) {
    vapply(seq_len(n), function(i) {
      if (runif(1) < plogis(theta[i] - b)) return(sample(right, 1))
      w <- wrong_of(options, right)
      sample(w, 1, prob = wrong_p[seq_along(w)])
    }, "")
  }
  a <- matrix("", n, 12)
  a[, 1] <- one(abcd, "C", -1.5)
  a[, 2] <- one(abcd, "A", -0.8)
  a[, 3] <- one(abcd, "D", -0.3)
  a[, 4] <- one(abcd, "B", 0.2)
  a[, 5] <- one(abcd, "A", 0.7)
  a[, 6] <- one(abcd, "D", 1.2)
  a[, 7] <- vapply(seq_len(n), function(i) {
    if (runif(1) < plogis(theta[i] - 0)) return("B")
    if (runif(1) < plogis(2 * theta[i])) "C" else sample(c("A", "D"), 1)
  }, "")
  a[, 8] <- vapply(seq_len(n), function(i) {
    if (runif(1) < plogis(theta[i] - 0.3)) return(if (runif(1) < 0.7) "B" else "E")
    sample(c("A", "C", "D"), 1, prob = c(0.5, 0.3, 0.2))
  }, "")
  a[, 9] <- vapply(seq_len(n), function(i) {
    if (runif(1) < plogis(theta[i] - 0.5)) return("AC")
    sample(c("A", "C", "ACD", "AB", "BC", "ABC"), 1, prob = c(0.3, 0.25, 0.15, 0.1, 0.1, 0.1))
  }, "")
  a[, 10] <- vapply(seq_len(n), function(i) if (runif(1) < plogis(theta[i] + 0.6)) "B" else "A", "")
  a[, 11] <- sample(abcd, n, replace = TRUE, prob = c(0.4, 0.3, 0.2, 0.1))
  a[, 12] <- sample(abcd, n, replace = TRUE)
  for (j in 1:10) a[runif(n) < 0.04, j] <- ""
  for (j in 1:8) {
    opts <- if (j == 8) c(abcd, "E") else abcd
    for (i in which(runif(n) < 0.02)) a[i, j] <- paste(sort(sample(opts, 2)), collapse = "")
  }

  # The points, by the rule of PSTAT-1 section 7.2, written here independently of any tool.
  correct <- list("C", "A", "D", "B", "A", "D", "B", c("B", "E"), "AC", "B", character(), character())
  pts <- matrix(0, n, 12)
  for (j in 1:8) pts[, j] <- as.numeric(nchar(a[, j]) == 1 & a[, j] %in% correct[[j]])
  pts[, 9] <- as.numeric(a[, 9] == "AC")
  pts[, 10] <- as.numeric(a[, 10] == "B")
  pts[, 11] <- 1
  pts[, 12] <- NA

  types <- c(rep("multiple_choice", 8), "select_all", "true_false", "multiple_choice", "multiple_choice")
  options <- list(abcd, abcd, abcd, abcd, abcd, abcd, abcd, c(abcd, "E"), c(abcd, "E"), c("A", "B"), abcd, abcd)
  adjustment <- c(rep("none", 10), "credit_all", "dropped")
  arr <- function(v) paste0("[", paste0('"', v, '"', collapse = ", "), "]")
  extra <- vapply(1:12, function(j) {
    sprintf(', "type": "%s", "options": %s, "correct": %s, "adjustment": "%s"', types[j], arr(options[[j]]),
            if (length(correct[[j]]) == 0) "[]" else arr(correct[[j]]), adjustment[j])
  }, "")
  write_case("letters-60", "Sixty sheets with the letters marked, scored against a key: one answer, an answer also accepted, select-all, true/false, a credited and a dropped question.",
             pts, versions, rep(1, 12), not_counted = list(q12 = 1:2), extra = extra, answers = a)
}
letters_case()
