# Reference values for the item-analysis test set (C3), written to reference.json.
# Run from this folder: Rscript make_reference.R
# Uses only base R and reads only the case files (cases/<name>/scores.csv and case.json), never a tool's code.
# README.md defines every figure; the names here are the names there.

SET_VERSION <- "1.0.0"

cases <- sort(list.dirs("cases", recursive = FALSE, full.names = FALSE))

# case.json, read line by line: make_cases.R writes one item per line in a fixed shape.
read_case <- function(name) {
  lines <- readLines(file.path("cases", name, "case.json"), warn = FALSE)
  items <- grep('"id": "q[0-9]+"', lines, value = TRUE)
  list(
    id = sub('.*"id": "(q[0-9]+)".*', "\\1", items),
    possible = as.numeric(sub('.*"possible": ([0-9.]+).*', "\\1", items)),
    partial = grepl('"partial_credit": true', items)
  )
}

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
    ul <- if (split && any(in_upper) && any(in_lower)) mean(frac[in_upper]) - mean(frac[in_lower]) else NA_real_
    items[[info$id[j]]] <- list(
      sheets = sum(counted),
      difficulty = mean(frac),
      point_biserial_item_included = r_or_na(x[counted, j], total[counted]),
      upper_lower_27 = ul
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

  list(
    sheets = n,
    small_group = n < 30,
    test = list(mean = mean(total), sd_population = sqrt(mean((total - mean(total))^2)), sd_sample = sd(total)),
    groups_27 = list(size = g, split = split, upper = I(d$sheet[upper]), lower = I(d$sheet[lower])),
    item = items,
    reliability_every_sheet = rel_every,
    reliability_listwise = rel_listwise
  )
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
