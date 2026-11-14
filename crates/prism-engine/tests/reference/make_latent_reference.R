# Reference values for latent class analysis and item response theory (prism-engine/tests/latent_reference.rs), from
# poLCA and mirt. Run from this folder: Rscript make_latent_reference.R
# Writes the packages' public example data (poLCA's carcinoma and values, mirt's LSAT7 and Science) and
# latent_reference.json.
suppressMessages({ library(poLCA); library(mirt) })
v <- c()
put <- function(name, x) if (!is.na(x)) v[[name]] <<- unname(x)

# Latent class analysis. Classes are random-started (nrep), so they are reported in order of size, largest first.
data(carcinoma); data(values)
write.csv(carcinoma, "carcinoma.csv", row.names = FALSE)
write.csv(values, "values.csv", row.names = FALSE)
lca <- function(p, data, items, R) {
  set.seed(1)
  f <- as.formula(paste0("cbind(", paste(items, collapse = ","), ") ~ 1"))
  m <- poLCA(f, data, nclass = R, nrep = 50, maxiter = 10000, tol = 1e-14, verbose = FALSE)
  o <- order(m$P, decreasing = TRUE)
  m <- poLCA(f, data, nclass = R, probs.start = poLCA.reorder(m$probs.start, o), maxiter = 10000, tol = 1e-14, verbose = FALSE)
  for (r in 1:R) { put(paste0(p, ".P.", r), m$P[r]); put(paste0(p, ".P.se.", r), m$P.se[r]) }
  for (j in items) for (r in 1:R) for (k in seq_len(ncol(m$probs[[j]]))) {
    put(paste0(p, ".prob.", j, ".", r, ".", k), m$probs[[j]][r, k])
    put(paste0(p, ".prob.se.", j, ".", r, ".", k), m$probs.se[[j]][r, k])
  }
  for (s in c("llik", "aic", "bic", "Gsq", "Chisq", "resid.df", "npar")) put(paste0(p, ".", s), m[[s]])
  # Relative entropy of the classification, 1 - sum(-h log h) / (N log R).
  h <- m$posterior
  put(paste0(p, ".entropy"), 1 - sum(-h * log(pmax(h, 1e-300))) / (nrow(h) * log(R)))
  for (r in 1:R) put(paste0(p, ".modal.", r), sum(m$predclass == r))
}
lca("car3", carcinoma, LETTERS[1:7], 3)
lca("car2", carcinoma, LETTERS[1:7], 2)
lca("val2", values, LETTERS[1:4], 2)

# Item response models by mirt (marginal ML on its 61-point grid from -6 to 6, converged to 1e-10; Oakes standard
# errors): Rasch and 2PL on LSAT7, the graded response model on Science.
lsat <- expand.table(LSAT7); names(lsat) <- paste0("i", 1:5)
write.csv(lsat, "lsat7.csv", row.names = FALSE)
write.csv(Science, "science.csv", row.names = FALSE)
irt <- function(p, data, type) {
  m <- mirt(data, 1, itemtype = type, SE = TRUE, TOL = 1e-10, verbose = FALSE)
  cf <- coef(m, printSE = TRUE); ir <- coef(m, IRTpars = TRUE, printSE = TRUE)
  for (it in colnames(data)) {
    for (nm in colnames(cf[[it]])) if (nm == "a1" || grepl("^d", nm)) {
      put(paste0(p, ".", nm, ".", it), cf[[it]]["par", nm]); put(paste0(p, ".", nm, ".se.", it), cf[[it]]["SE", nm])
    }
    for (nm in colnames(ir[[it]])) if (grepl("^b", nm)) {
      put(paste0(p, ".", nm, ".", it), ir[[it]]["par", nm]); put(paste0(p, ".", nm, ".se.", it), ir[[it]]["SE", nm])
    }
  }
  put(paste0(p, ".var"), cf$GroupPars["par", "COV_11"]); put(paste0(p, ".var.se"), cf$GroupPars["SE", "COV_11"])
  put(paste0(p, ".logl"), logLik(m)); put(paste0(p, ".aic"), extract.mirt(m, "AIC")); put(paste0(p, ".bic"), extract.mirt(m, "BIC"))
  put(paste0(p, ".npar"), extract.mirt(m, "nest"))
}
irt("rasch", lsat, "Rasch")
irt("twopl", lsat, "2PL")
irt("grm", Science, "graded")

lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, ", poLCA ", packageVersion("poLCA"), ", mirt ", packageVersion("mirt"), '",'), paste(lines, collapse = ",\n"), "}"), "latent_reference.json")
