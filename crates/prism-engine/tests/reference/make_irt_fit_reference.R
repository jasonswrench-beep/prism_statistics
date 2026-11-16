# Reference values for item response model fit (prism-engine/tests/irt_fit_reference.rs), from mirt's M2() and
# itemfit(). Run from this folder: Rscript make_irt_fit_reference.R
# Reads lsat7.csv and science.csv (written by make_latent_reference.R) and fits the same models as there: Rasch and 2PL
# on LSAT7, the graded response model on Science; and the graded model on bfi3.csv (below). Writes irt_fit_reference.json.
suppressMessages(library(mirt))
v <- c()
put <- function(name, x) if (length(x) == 1 && !is.na(x) && is.finite(x)) v[[name]] <<- unname(x)
lsat <- read.csv("lsat7.csv")
sci <- read.csv("science.csv")
# Ordered items with enough of them for M2* to have degrees of freedom: psych's bfi agreeableness items and
# the first two conscientiousness items, recoded from six categories to three (1-2, 3-4, 5-6), complete cases. Written as bfi3.csv.
suppressMessages(library(psych))
data(bfi)
b3 <- na.omit(bfi[, c(paste0("A", 1:5), paste0("C", 1:2))])
b3[] <- lapply(b3, function(x) (x + 1) %/% 2)
write.csv(b3, "bfi3.csv", row.names = FALSE)
fit <- function(p, data, type) {
  m <- mirt(data, 1, itemtype = type, SE = TRUE, TOL = 1e-10, verbose = FALSE)
  g <- tryCatch(M2(m), error = function(e) NULL) # M2 needs more margins than parameters
  if (!is.null(g)) for (s in c("M2", "df", "p", "RMSEA", "RMSEA_5", "RMSEA_95", "SRMSR", "TLI", "CFI")) put(paste0(p, ".m2.", tolower(s)), g[[s]])
  f <- itemfit(m)
  for (i in seq_len(nrow(f))) {
    it <- f$item[i]
    put(paste0(p, ".sx2.", it), f$S_X2[i]); put(paste0(p, ".sx2.df.", it), f$df.S_X2[i])
    put(paste0(p, ".sx2.rmsea.", it), f$RMSEA.S_X2[i]); put(paste0(p, ".sx2.p.", it), f$p.S_X2[i])
  }
}
fit("rasch", lsat, "Rasch")
fit("twopl", lsat, "2PL")
fit("grm", sci, "graded")
fit("bfi3", b3, "graded")
lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, ", mirt ", packageVersion("mirt"), ", psych ", packageVersion("psych"), '",'), paste(lines, collapse = ",\n"), "}"), "irt_fit_reference.json")
