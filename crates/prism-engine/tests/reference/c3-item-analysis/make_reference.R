# Reference values for the item-analysis test set (C3), written to reference.json.
# Run from this folder: Rscript make_reference.R
# Uses only base R and reads only the case files (cases/<name>/scores.csv, case.json and, for a case with letters,
# answers.csv), never a tool's code. README.md defines every figure; the names here are the names there.

SET_VERSION <- "1.1.0"

cases <- sort(list.dirs("cases", recursive = FALSE, full.names = FALSE))

# case.json, read line by line: make_cases.R writes one item per line in a fixed shape.
read_case <- function(name) {
  lines <- readLines(file.path("cases", name, "case.json"), warn = FALSE)
  items <- grep('"id": "q[0-9]+"', lines, value = TRUE)
  list(
    id = sub('.*"id": "(q[0-9]+)".*', "\\1", items),
    possible = as.numeric(sub('.*"possible": ([0-9.]+).*', "\\1", items)),
    partial = grepl('"partial_credit": true', items),
    options = lapply(items, function(line) list_of(line, "options")),
    correct = lapply(items, function(line) list_of(line, "correct"))
  )
}

# A list of strings in one item's line of case.json, such as "options": ["A", "B"]; empty when the line has none.
list_of <- function(line, member) {
  m <- regmatches(line, regexpr(sprintf('"%s": \\[[^]]*\\]', member), line))
  if (length(m) == 0) return(character())
  inner <- sub(sprintf('"%s": \\[([^]]*)\\]', member), "\\1", m)
  if (inner == "") character() else gsub('"', "", trimws(strsplit(inner, ",")[[1]]))
}

# A variance with n as the divisor.
varn <- function(v) mean((v - mean(v))^2)

# A correlation, or NA when either side does not vary (the figure is undefined, not zero).
r_or_na <- function(a, b) if (length(a) < 2 || sd(a) == 0 || sd(b) == 0) NA_real_ else cor(a, b)

# Cronbach's alpha of the columns of m (KR-20 when every item is right or wrong). NA when the total does not vary.
alpha_of <- function(m) {
  m <- as.matrix(m)
  k <- ncol(m)
  vt <- var(rowSums(m))
  if (k < 2 || nrow(m) < 2 || vt == 0) return(NA_real_)
  k / (k - 1) * (1 - sum(apply(m, 2, var)) / vt)
}

one_case <- function(name) {
  info <- read_case(name)
  d <- read.csv(file.path("cases", name, "scores.csv"))
  # A question that counts on no sheet is not in the .pstat file (PSTAT-1 section 7.1): it has no figures at all.
  live <- vapply(info$id, function(q) any(!is.na(d[[q]])), TRUE)
  info <- lapply(info, function(v) v[live])
  x <- as.matrix(d[, info$id, drop = FALSE])
  n <- nrow(x)
  total <- rowSums(x, na.rm = TRUE)

  # The 27 percent groups: g = 27 percent of the sheets rounded half up, in whole numbers so no rounding of 0.27 * n
  # can move it. Sheets are ordered by total, highest first, and a tie goes to the lower sheet number first.
  g <- (27 * n + 50) %/% 100
  split <- g >= 2 && 2 * g <= n
  ord <- order(-total, d$sheet)
  upper <- if (split) ord[seq_len(g)] else integer()
  lower <- if (split) ord[(n - g + 1):n] else integer()

  items <- list()
  for (j in seq_along(info$id)) {
    counted <- !is.na(x[, j])
    frac <- x[counted, j] / info$possible[j]
    in_upper <- which(counted) %in% upper
    in_lower <- which(counted) %in% lower
    both <- split && any(in_upper) && any(in_lower)
    items[[info$id[j]]] <- list(
      sheets = sum(counted),
      difficulty = mean(frac),
      point_biserial_item_included = r_or_na(x[counted, j], total[counted]),
      upper_lower_27 = if (both) mean(frac[in_upper]) - mean(frac[in_lower]) else NA_real_,
      upper_27 = if (both) mean(frac[in_upper]) else NA_real_,
      lower_27 = if (both) mean(frac[in_lower]) else NA_real_
    )
  }

  # Reliability as the class figure: the questions that count on every sheet, KR-20 unless one gives partial credit.
  every <- which(colSums(is.na(x)) == 0)
  rel_every <- list(
    kind = if (any(info$partial[every])) "Cronbach alpha" else "KR-20",
    items = length(every),
    left_out = I(setdiff(info$id, info$id[every])),
    value = if (length(every) >= 2) alpha_of(x[, every, drop = FALSE]) else NA_real_
  )

  # Reliability as a scale analysis: every question, sheets with a value on all of them (listwise).
  cc <- complete.cases(x)
  xc <- x[cc, , drop = FALSE]
  nc <- nrow(xc)
  k <- ncol(xc)
  a <- alpha_of(xc)
  sds <- apply(xc, 2, sd)
  rbar <- if (all(sds > 0)) { r <- cor(xc); mean(r[upper.tri(r)]) } else NA_real_
  rel_items <- list()
  for (j in seq_len(k)) {
    rest <- rowSums(xc[, -j, drop = FALSE])
    rel_items[[info$id[j]]] <- list(
      mean = mean(xc[, j]),
      sd = sd(xc[, j]),
      corrected_item_total_r = r_or_na(xc[, j], rest),
      alpha_if_deleted = if (k > 2) alpha_of(xc[, -j, drop = FALSE]) else NA_real_
    )
  }
  rel_listwise <- list(
    n = nc,
    items = k,
    alpha = a,
    feldt_95_lower = 1 - (1 - a) * qf(0.975, nc - 1, (nc - 1) * (k - 1)),
    feldt_95_upper = 1 - (1 - a) * qf(0.025, nc - 1, (nc - 1) * (k - 1)),
    standardized_alpha = if (is.na(rbar)) NA_real_ else k * rbar / (1 + (k - 1) * rbar),
    mean_inter_item_r = rbar,
    item = rel_items
  )

  # Item analysis on the points as a scale analysis (PrismStatistics's Item Analysis): the sheets with a value on every
  # question, variances divided by n.
  pc <- info$possible
  tc <- rowSums(xc)
  vt <- varn(tc)
  rw <- all(vapply(seq_len(k), function(j) all(xc[, j] == 0 | xc[, j] == pc[j]), TRUE))
  equal <- rw && all(pc == pc[1])
  rel <- if (k >= 2 && vt > 1e-12) k / (k - 1) * (1 - sum(apply(xc, 2, varn)) / vt) else NA_real_
  kr21 <- if (equal && vt > 1e-12) {
    w <- pc[1]
    mc <- mean(tc) / w
    k / (k - 1) * (1 - mc * (k - mc) / (k * vt / w^2))
  } else NA_real_
  gl <- (27 * nc + 50) %/% 100
  split_l <- gl >= 2 && 2 * gl <= nc
  ord_l <- order(-tc, d$sheet[cc])
  up_l <- if (split_l) ord_l[seq_len(gl)] else integer()
  lo_l <- if (split_l) ord_l[(nc - gl + 1):nc] else integer()
  ia_items <- list()
  for (j in seq_len(k)) {
    f <- xc[, j] / pc[j]
    ia_items[[info$id[j]]] <- list(
      p = mean(f),
      mean = mean(xc[, j]),
      point_biserial_item_included = r_or_na(xc[, j], tc),
      corrected_item_total_r = r_or_na(xc[, j], tc - xc[, j]),
      alpha_if_deleted = if (k > 2) alpha_of(xc[, -j, drop = FALSE]) else NA_real_,
      upper = if (split_l) mean(f[up_l]) else NA_real_,
      lower = if (split_l) mean(f[lo_l]) else NA_real_,
      d = if (split_l) mean(f[up_l]) - mean(f[lo_l]) else NA_real_
    )
  }
  ia <- list(
    n = nc,
    k = k,
    possible = sum(pc),
    mean = mean(tc),
    sd = sd(tc),
    sd_n = sqrt(vt),
    mean_p = mean(tc) / sum(pc),
    right_or_wrong = rw,
    reliability = rel,
    kr21 = kr21,
    sem = if (is.na(rel)) NA_real_ else sd(tc) * sqrt(max(0, 1 - rel)),
    group_size = gl,
    split = split_l,
    upper = I(d$sheet[cc][up_l]),
    lower = I(d$sheet[cc][lo_l]),
    item = ia_items
  )

  # The letters marked (a case with answers.csv): blanks, each answer by score group, and each letter's marks.
  answers <- NULL
  if (file.exists(file.path("cases", name, "answers.csv"))) {
    ans <- read.csv(file.path("cases", name, "answers.csv"), colClasses = "character", na.strings = character())
    stopifnot(all(cc), identical(as.integer(ans$sheet), as.integer(d$sheet)))
    middle <- setdiff(seq_len(nc), c(up_l, lo_l))
    answers <- list()
    for (j in seq_len(k)) {
      a <- ans[[info$id[j]]]
      seen <- unique(c(a[a != ""], info$options[[j]], info$correct[[j]]))
      seen <- seen[order(nchar(seen), seen)]
      by_answer <- list()
      for (o in c(seen, "blank")) {
        hit <- a == (if (o == "blank") "" else o)
        by_answer[[o]] <- list(n = sum(hit), upper = sum(hit[up_l]), middle = sum(hit[middle]), lower = sum(hit[lo_l]))
      }
      by_letter <- list()
      for (l in info$options[[j]]) by_letter[[l]] <- sum(grepl(l, a, fixed = TRUE))
      answers[[info$id[j]]] <- list(blank = sum(a == ""), by_answer = by_answer, by_letter = by_letter)
    }
  }

  out <- list(
    sheets = n,
    small_group = n < 30,
    test = list(mean = mean(total), sd_population = sqrt(mean((total - mean(total))^2)), sd_sample = sd(total)),
    groups_27 = list(size = g, split = split, upper = I(d$sheet[upper]), lower = I(d$sheet[lower])),
    item = items,
    reliability_every_sheet = rel_every,
    reliability_listwise = rel_listwise,
    item_analysis_listwise = ia
  )
  if (!is.null(answers)) out$answers <- answers
  out
}

# A small JSON writer: numbers to 17 significant digits, NA as null, I() for a list that must stay a list.
json <- function(x, indent = "") {
  inner <- paste0(indent, "  ")
  if (is.list(x) && !is.null(names(x)) && !inherits(x, "AsIs")) {
    parts <- vapply(names(x), function(k) paste0(inner, '"', k, '": ', json(x[[k]], inner)), "")
    return(paste0("{\n", paste(parts, collapse = ",\n"), "\n", indent, "}"))
  }
  scalar <- function(v) {
    if (is.na(v)) "null"
    else if (is.logical(v)) tolower(as.character(v))
    else if (is.character(v)) paste0('"', gsub('(["\\\\])', "\\\\\\1", v), '"')
    else sprintf("%.17g", v)
  }
  if (inherits(x, "AsIs")) return(paste0("[", paste(vapply(unclass(x), scalar, ""), collapse = ", "), "]"))
  stopifnot(length(x) == 1)
  scalar(x)
}

out <- list(
  "_about" = "Reference values for the OpenStage item-analysis test set. README.md defines each figure.",
  "_set_version" = SET_VERSION,
  "_made_with" = R.version.string,
  cases = setNames(lapply(cases, one_case), cases)
)
con <- file("reference.json", "wb")
writeLines(json(out), con)
close(con)
cat("Wrote reference.json:", length(cases), "cases\n")
