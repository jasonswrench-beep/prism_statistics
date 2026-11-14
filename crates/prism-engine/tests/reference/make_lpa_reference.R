# Reference values for latent profile analysis (prism-engine/tests/lpa_reference.rs), from mclust.
# Run from this folder: Rscript make_lpa_reference.R
# Uses iris.csv (from make_tree_reference.R); writes faithful.csv and lpa_reference.json.
suppressMessages(library(mclust))
ir <- read.csv("iris.csv")[, c("sepal_length", "sepal_width", "petal_length", "petal_width")]
write.csv(faithful, "faithful.csv", row.names = FALSE)
v <- c()
put <- function(name, x) if (length(x) == 1 && !is.na(x)) v[[name]] <<- unname(x)
ctl <- emControl(tol = c(1e-12, sqrt(.Machine$double.eps)))
dump <- function(p, X, model, g) {
  m <- Mclust(X, G = g, modelNames = model, control = ctl, verbose = FALSE)
  if (is.null(m)) return(invisible())
  k <- paste0(p, ".", model, ".", g)
  put(paste0(k, ".ll"), m$loglik); put(paste0(k, ".bic"), m$bic); put(paste0(k, ".npar"), m$df)
  put(paste0(k, ".icl"), icl(m))
  pro <- m$parameters$pro; if (is.null(pro)) pro <- 1
  mu <- m$parameters$mean; if (is.null(dim(mu))) mu <- matrix(mu, nrow = 1)
  sig <- m$parameters$variance$sigma
  # Classes in order of size, largest first (ties by mean of the first indicator).
  o <- order(-pro, mu[1, ])
  for (c in seq_along(o)) {
    put(paste0(k, ".class.", c, ".pro"), pro[o[c]])
    for (j in seq_len(nrow(mu))) {
      put(paste0(k, ".class.", c, ".mean.", j), mu[j, o[c]])
      put(paste0(k, ".class.", c, ".var.", j), if (is.null(dim(sig))) m$parameters$variance$sigmasq[min(o[c], length(m$parameters$variance$sigmasq))] else sig[j, j, o[c]])
    }
  }
}
for (model in c("EEI", "VVI", "EEE", "VVV")) for (g in 1:4) dump("iris", ir, model, g)
for (model in c("EEI", "VVI", "EEE", "VVV")) for (g in 1:4) dump("faith", faithful, model, g)
for (model in c("E", "V")) for (g in 1:3) dump("erupt", faithful$eruptions, model, g)
lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, ", mclust ", packageVersion("mclust"), '",'), paste(lines, collapse = ",\n"), "}"), "lpa_reference.json")
