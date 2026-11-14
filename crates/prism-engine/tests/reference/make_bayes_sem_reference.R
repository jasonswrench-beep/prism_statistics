# Reference values for Bayesian SEM (prism-engine/tests/latent_reference.rs), from blavaan with its default priors.
# Run from this folder: Rscript make_bayes_sem_reference.R
# MCMC results are random, so the chains are long (3 x 5000 draws after 1000 of warm-up) to keep blavaan's own Monte
# Carlo error small; the tests allow for both programs' error. Uses holzinger.csv (written by make_sem_reference.R).
suppressMessages(library(blavaan))
hs <- read.csv("holzinger.csv")
v <- c()
put <- function(name, x) if (!is.na(x)) v[[name]] <<- unname(x)
dump <- function(p, fit) {
  # Posterior summaries from blavaan's draws (columns in the order of the free parameters).
  draws <- do.call(rbind, lapply(blavInspect(fit, "mcmc"), as.matrix))
  pt <- lavaan::partable(fit)
  pt <- pt[pt$free > 0, ]
  pt <- pt[order(pt$free), ]
  stopifnot(ncol(draws) == nrow(pt))
  for (i in seq_len(nrow(pt))) {
    k <- paste0(p, ".", pt$lhs[i], pt$op[i], pt$rhs[i]); x <- draws[, i]
    put(paste0(k, ".mean"), mean(x)); put(paste0(k, ".sd"), sd(x))
    put(paste0(k, ".lower"), quantile(x, 0.025)); put(paste0(k, ".upper"), quantile(x, 0.975))
  }
  fm <- lavaan::fitMeasures(fit)
  for (m in c("ppp", "dic", "p_dic", "waic", "p_waic", "looic", "npar")) put(paste0(p, ".fit.", m), fm[[m]])
}
# blavaan inspects its call, so it is called directly rather than through a wrapper.
dump("bcfa", bcfa("visual =~ x1 + x2 + x3
textual =~ x4 + x5 + x6
speed =~ x7 + x8 + x9", hs, seed = 1, n.chains = 3, burnin = 1000, sample = 5000, bcontrol = list(cores = 1)))
dump("bpath", bsem("x4 ~ x1 + x2
x9 ~ x4 + x1", hs, seed = 1, n.chains = 3, burnin = 1000, sample = 5000, bcontrol = list(cores = 1)))
lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, ", blavaan ", packageVersion("blavaan"), '",'), paste(lines, collapse = ",\n"), "}"), "bayes_sem_reference.json")
