# Reference values for Bayesian meta-analysis (prism-engine/tests/bmeta_reference.rs), from bayesmeta.
# Run from this folder: Rscript make_bmeta_reference.R
# Uses bcg.csv and normand.csv (from make_meta_reference.R); writes crins.csv and bmeta_reference.json.
suppressMessages({library(bayesmeta); library(metafor)})
data(CrinsEtAl2014, package = "bayesmeta")
cr <- CrinsEtAl2014
write.csv(data.frame(study = cr$publication, e1 = cr$exp.AR.events, n1 = cr$exp.total, e2 = cr$cont.AR.events, n2 = cr$cont.total), "crins.csv", row.names = FALSE)
v <- c()
put <- function(name, x) if (length(x) == 1 && !is.na(x)) v[[name]] <<- unname(x)
fine <- function(...) bayesmeta(...)
dump <- function(p, r) {
  s <- r$summary
  for (col in c("tau")) {
    k <- c(tau = "tau", mu = "mu", theta = "pred")[[col]]
    put(paste0(p, ".", k, ".mode"), s["mode", col]); put(paste0(p, ".", k, ".median"), s["median", col])
    put(paste0(p, ".", k, ".mean"), s["mean", col]); put(paste0(p, ".", k, ".sd"), s["sd", col])
    put(paste0(p, ".", k, ".lower"), s["95% lower", col]); put(paste0(p, ".", k, ".upper"), s["95% upper", col])
  }
  # bayesmeta finds tau's mode and shortest interval with optimize()'s default tolerance (about 1e-4); recompute
  # them tightly from its exact density and quantile functions.
  up <- r$qposterior(tau.p = 0.999)
  md <- optimize(function(t) -r$dposterior(tau = t), c(0, up), tol = 1e-12)$minimum
  put(paste0(p, ".tau.mode"), if (r$dposterior(tau = 0) >= r$dposterior(tau = md)) 0 else md)
  if (s["95% lower", "tau"] > 0) {
    a <- optimize(function(a) r$qposterior(tau.p = a + 0.95) - r$qposterior(tau.p = a), c(0, 0.05), tol = 1e-12)$minimum
    put(paste0(p, ".tau.lower"), r$qposterior(tau.p = a)); put(paste0(p, ".tau.upper"), r$qposterior(tau.p = a + 0.95))
  }
  # bayesmeta's mu, prediction and shrinkage summaries come from its DIRECT grid; replace them with exact mixtures
  # over the exact posterior of tau (dposterior) and the exact conditional moments (cond.moment).
  ex <- function(key, cm) {
    # cm(tau): conditional mean and sd at tau.
    pt <- function(t) r$dposterior(tau = t)
    m <- function(t) cm(t)[, 1]; s <- function(t) cm(t)[, 2]
    I <- function(f) integrate(function(t) f(t) * pt(t), 0, Inf, rel.tol = 1e-12, subdivisions = 10000L)$value
    mean <- I(m); sd <- sqrt(I(function(t) s(t)^2 + m(t)^2) - mean^2)
    cdf <- function(x) I(function(t) pnorm(x, m(t), s(t)))
    q <- function(p) uniroot(function(x) cdf(x) - p, mean + c(-30, 30) * sd, tol = 1e-13)$root
    a <- optimize(function(a) q(a + 0.95) - q(a), c(1e-9, 0.05 - 1e-9), tol = 1e-10)$minimum
    put(paste0(key, ".mean"), mean); put(paste0(key, ".sd"), sd); put(paste0(key, ".median"), q(0.5))
    put(paste0(key, ".lower"), q(a)); put(paste0(key, ".upper"), q(a + 0.95))
  }
  ex(paste0(p, ".mu"), function(t) r$cond.moment(tau = t))
  ex(paste0(p, ".pred"), function(t) r$cond.moment(tau = t, predict = TRUE))
  for (i in seq_len(r$k)) ex(paste0(p, ".theta.", i), function(t) { a <- r$cond.moment(tau = t, individual = TRUE); matrix(a[, , i], ncol = 2) })
}
es <- escalc("OR", ai = cr$exp.AR.events, n1i = cr$exp.total, ci = cr$cont.AR.events, n2i = cr$cont.total)
dump("crins", fine(y = es$yi, sigma = sqrt(es$vi)))
dump("crins_hn", fine(y = es$yi, sigma = sqrt(es$vi), tau.prior = function(t) dhalfnormal(t, scale = 0.5)))
dump("crins_hc", fine(y = es$yi, sigma = sqrt(es$vi), tau.prior = function(t) dhalfcauchy(t, scale = 0.5), mu.prior.mean = 0, mu.prior.sd = 4))
bcg <- read.csv("bcg.csv")
eb <- escalc("RR", ai = bcg$tpos, n1i = bcg$n1, ci = bcg$cpos, n2i = bcg$n2)
dump("bcg", fine(y = eb$yi, sigma = sqrt(eb$vi)))
nm <- read.csv("normand.csv")
en <- escalc("MD", m1i = nm$m1i, sd1i = nm$sd1i, n1i = nm$n1i, m2i = nm$m2i, sd2i = nm$sd2i, n2i = nm$n2i)
dump("normand", fine(y = en$yi, sigma = sqrt(en$vi), tau.prior = function(t) dhalfnormal(t, scale = 10)))
lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, ", bayesmeta ", packageVersion("bayesmeta"), '",'), paste(lines, collapse = ",\n"), "}"), "bmeta_reference.json")
