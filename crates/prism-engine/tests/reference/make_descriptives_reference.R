# Reference values for Frequencies and Descriptives (prism-engine/tests/descriptives_reference.rs).
# Run from this folder: Rscript make_descriptives_reference.R
# Coding used by the test: age -99 = user-missing; ethnicity -9 = user-missing.
d <- read.csv("demographics.csv")
age <- d$age; age[age %in% -99] <- NA; a <- age[!is.na(age)]
v <- c(age_n = length(a), age_mean = mean(a), age_sd = sd(a), age_min = min(a), age_max = max(a),
       age_median = unname(quantile(a, 0.5, type = 7)), age_se = sd(a) / sqrt(length(a)),
       age_ci_lo = t.test(a)$conf.int[1], age_ci_hi = t.test(a)$conf.int[2],
       sex_1 = sum(d$sex == 1, na.rm = TRUE), sex_2 = sum(d$sex == 2, na.rm = TRUE), sex_3 = sum(d$sex == 3, na.rm = TRUE),
       sex_empty = sum(is.na(d$sex)), eth_missing = sum(d$ethnicity %in% -9))
lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", v))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, '",'), paste(lines, collapse = ",\n"), "}"), "descriptives_reference.json")
