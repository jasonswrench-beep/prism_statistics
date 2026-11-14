# Reference values for the independent-samples t test (prism-engine/tests/ttest_reference.rs).
# Run from this folder: Rscript make_ttest_reference.R
# Coding used by the test: site 1 = Urban, 2 = Rural, 3 = Online, 9 = user-missing; stress -99 = user-missing.
d <- read.csv("two_groups.csv")
d$site[d$site %in% 9] <- NA
d$stress[d$stress %in% -99] <- NA
k <- d[!is.na(d$site) & d$site %in% c(1, 2) & !is.na(d$stress), ]
x <- k$stress[k$site == 1]; y <- k$stress[k$site == 2]
w <- t.test(x, y, var.equal = FALSE, conf.level = 0.95)
s <- t.test(x, y, var.equal = TRUE, conf.level = 0.90)
n1 <- length(x); n2 <- length(y)
sp <- sqrt(((n1 - 1) * var(x) + (n2 - 1) * var(y)) / (n1 + n2 - 2))
dd <- (mean(x) - mean(y)) / sp
m <- n1 + n2 - 2
J <- exp(lgamma(m / 2) - log(sqrt(m / 2)) - lgamma((m - 1) / 2))
v <- c(
  n_total = nrow(d), n1 = n1, n2 = n2, mean1 = mean(x), sd1 = sd(x), mean2 = mean(y), sd2 = sd(y),
  welch_t = unname(w$statistic), welch_df = unname(w$parameter), welch_p = w$p.value, welch_lo = w$conf.int[1], welch_hi = w$conf.int[2],
  student_t = unname(s$statistic), student_df = unname(s$parameter), student_p = s$p.value, student90_lo = s$conf.int[1], student90_hi = s$conf.int[2],
  d = dd, g = dd * J
)
lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", v))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, '",'), paste(lines, collapse = ",\n"), "}"), "ttest_reference.json")
