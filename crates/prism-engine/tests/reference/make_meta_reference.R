# Reference values for meta-analysis (prism-engine/tests/meta_reference.rs), from metafor.
# Run from this folder: Rscript make_meta_reference.R
# Writes bcg.csv, normand.csv and molloy.csv (from the metadat package) and meta_reference.json.
suppressMessages(library(metafor))
bcg <- dat.bcg
bcg$n1 <- bcg$tpos + bcg$tneg
bcg$n2 <- bcg$cpos + bcg$cneg
bcg$label <- paste(bcg$author, bcg$year)
write.csv(bcg[, c("trial", "label", "tpos", "n1", "cpos", "n2", "ablat", "year", "alloc")], "bcg.csv", row.names = FALSE)
nm <- dat.normand1999
write.csv(nm[, c("study", "source", "n1i", "m1i", "sd1i", "n2i", "m2i", "sd2i")], "normand.csv", row.names = FALSE)
ml <- dat.molloy2014
ml$label <- paste(ml$authors, ml$year)
write.csv(ml[, c("label", "ni", "ri", "controls", "meanage")], "molloy.csv", row.names = FALSE)

v <- c()
put <- function(name, x) if (length(x) == 1 && !is.na(x)) v[[name]] <<- unname(x)
tight <- list(threshold = 1e-12, maxiter = 10000, tol = 1e-12)
fitall <- function(p, res, ci = TRUE, bias = FALSE, loo = FALSE) {
  for (j in seq_along(res$beta)) {
    put(paste0(p, ".beta.", j, ".est"), res$beta[j]); put(paste0(p, ".beta.", j, ".se"), res$se[j])
    put(paste0(p, ".beta.", j, ".stat"), res$zval[j]); put(paste0(p, ".beta.", j, ".p"), res$pval[j])
    put(paste0(p, ".beta.", j, ".lower"), res$ci.lb[j]); put(paste0(p, ".beta.", j, ".upper"), res$ci.ub[j])
  }
  put(paste0(p, ".k"), res$k)
  put(paste0(p, ".tau2"), res$tau2); put(paste0(p, ".tau2.se"), res$se.tau2)
  put(paste0(p, ".qe"), res$QE); put(paste0(p, ".qe.p"), res$QEp)
  put(paste0(p, ".i2"), res$I2); put(paste0(p, ".h2"), res$H2)
  if (res$p > 1) { put(paste0(p, ".qm"), res$QM); put(paste0(p, ".qm.p"), res$QMp); if (!is.null(res$R2)) put(paste0(p, ".r2"), res$R2) }
  fs <- fitstats(res)
  put(paste0(p, ".logl"), fs[1]); put(paste0(p, ".aic"), fs[3]); put(paste0(p, ".bic"), fs[4])
  for (i in seq_len(res$k)) {
    put(paste0(p, ".yi.", i), res$yi[i]); put(paste0(p, ".vi.", i), res$vi[i])
    put(paste0(p, ".weight.", i), weights(res)[i])
  }
  if (res$p == 1 && res$method != "FE") {
    pr <- predict(res)
    put(paste0(p, ".pi.lower"), pr$pi.lb); put(paste0(p, ".pi.upper"), pr$pi.ub)
  }
  if (ci && res$method != "FE") {
    cf <- confint(res, control = list(tol = 1e-12, tau2.max = 1e6))$random  # metafor caps its search at 10 tau2 by default
    put(paste0(p, ".tau2.lower"), cf["tau^2", "ci.lb"]); put(paste0(p, ".tau2.upper"), cf["tau^2", "ci.ub"])
    put(paste0(p, ".tau.lower"), cf["tau", "ci.lb"]); put(paste0(p, ".tau.upper"), cf["tau", "ci.ub"])
    put(paste0(p, ".i2.lower"), cf["I^2(%)", "ci.lb"]); put(paste0(p, ".i2.upper"), cf["I^2(%)", "ci.ub"])
    put(paste0(p, ".h2.lower"), cf["H^2", "ci.lb"]); put(paste0(p, ".h2.upper"), cf["H^2", "ci.ub"])
  }
  if (bias) {
    eg <- regtest(res)
    put(paste0(p, ".egger.stat"), eg$zval); put(paste0(p, ".egger.p"), eg$pval)
    put(paste0(p, ".egger.limit"), eg$est); put(paste0(p, ".egger.limit.lower"), eg$ci.lb); put(paste0(p, ".egger.limit.upper"), eg$ci.ub)
    tf <- trimfill(res)
    put(paste0(p, ".tf.k0"), tf$k0); put(paste0(p, ".tf.se.k0"), tf$se.k0)
    put(paste0(p, ".tf.est"), tf$beta[1]); put(paste0(p, ".tf.lower"), tf$ci.lb); put(paste0(p, ".tf.upper"), tf$ci.ub)
    put(paste0(p, ".tf.tau2"), tf$tau2)
  }
  if (loo) {
    l1 <- leave1out(res)
    for (i in seq_along(l1$estimate)) {
      put(paste0(p, ".loo.", i, ".est"), l1$estimate[i]); put(paste0(p, ".loo.", i, ".tau2"), l1$tau2[i]); put(paste0(p, ".loo.", i, ".i2"), l1$I2[i])
    }
  }
}

or <- escalc("OR", ai = tpos, n1i = n1, ci = cpos, n2i = n2, data = bcg)
rr <- escalc("RR", ai = tpos, n1i = n1, ci = cpos, n2i = n2, data = bcg)
rd <- escalc("RD", ai = tpos, n1i = n1, ci = cpos, n2i = n2, data = bcg)
fitall("or", rma(yi, vi, data = or, control = tight), bias = TRUE, loo = TRUE)
for (m in c("FE", "REML", "ML", "DL", "PM", "HE", "SJ")) fitall(paste0("rr", tolower(m)), rma(yi, vi, data = rr, method = m, control = tight), bias = TRUE)
fitall("rrknha", rma(yi, vi, data = rr, test = "knha", control = tight), bias = TRUE)
fitall("rd", rma(yi, vi, data = rd, method = "DL", control = tight))
fitall("mods", rma(yi, vi, mods = ~ ablat + factor(alloc), data = rr, control = tight), ci = TRUE)
fitall("modsknha", rma(yi, vi, mods = ~ ablat, data = rr, test = "knha", control = tight))
fitall("modsml", rma(yi, vi, mods = ~ ablat, data = rr, method = "ML", control = tight))
for (m in c("DL", "PM", "HE", "SJ")) fitall(paste0("mods", tolower(m)), rma(yi, vi, mods = ~ ablat, data = rr, method = m, control = tight))
smd <- escalc("SMD", m1i = m1i, sd1i = sd1i, n1i = n1i, m2i = m2i, sd2i = sd2i, n2i = n2i, data = nm)
md <- escalc("MD", m1i = m1i, sd1i = sd1i, n1i = n1i, m2i = m2i, sd2i = sd2i, n2i = n2i, data = nm)
fitall("smd", rma(yi, vi, data = smd, control = tight), bias = TRUE)
fitall("md", rma(yi, vi, data = md, method = "DL", control = tight))
zc <- escalc("ZCOR", ri = ri, ni = ni, data = ml)
fitall("zcor", rma(yi, vi, data = zc, control = tight), bias = TRUE, loo = TRUE)

# Multilevel (three-level) models: Konstantopoulos (2011), schools in districts; Assink and Wibbelink (2016), effects
# in studies with a moderator.
kd <- dat.konstantopoulos2011
write.csv(kd[, c("district", "school", "study", "yi", "vi", "year")], "konstantopoulos.csv", row.names = FALSE)
as <- dat.assink2016
write.csv(as[, c("study", "esid", "yi", "vi", "pubstatus", "year")], "assink.csv", row.names = FALSE)
mtight <- list(rel.tol = 1e-12)
ml3 <- function(p, res, data, cl, id) {
  for (j in seq_along(res$beta)) {
    put(paste0(p, ".beta.", j, ".est"), res$beta[j]); put(paste0(p, ".beta.", j, ".se"), res$se[j])
    put(paste0(p, ".beta.", j, ".stat"), res$zval[j]); put(paste0(p, ".beta.", j, ".p"), res$pval[j])
    put(paste0(p, ".beta.", j, ".lower"), res$ci.lb[j]); put(paste0(p, ".beta.", j, ".upper"), res$ci.ub[j])
  }
  put(paste0(p, ".k"), res$k); put(paste0(p, ".clusters"), res$s.nlevels[1])
  put(paste0(p, ".sigma2.1"), res$sigma2[1]); put(paste0(p, ".sigma2.2"), res$sigma2[2])
  put(paste0(p, ".qe"), res$QE); put(paste0(p, ".qe.p"), res$QEp)
  if (res$p > 1) { put(paste0(p, ".qm"), res$QM); put(paste0(p, ".qm.p"), res$QMp) }
  put(paste0(p, ".logl"), logLik(res)); put(paste0(p, ".aic"), AIC(res)); put(paste0(p, ".bic"), BIC(res))
  W <- diag(1 / res$vi); X <- model.matrix(res); P <- W - W %*% X %*% solve(t(X) %*% W %*% X) %*% t(X) %*% W
  vt <- (res$k - res$p) / sum(diag(P))
  put(paste0(p, ".i2.1"), 100 * res$sigma2[1] / (sum(res$sigma2) + vt)); put(paste0(p, ".i2.2"), 100 * res$sigma2[2] / (sum(res$sigma2) + vt))
  put(paste0(p, ".i2"), 100 * sum(res$sigma2) / (sum(res$sigma2) + vt))
  w <- weights(res); for (i in seq_along(w)) put(paste0(p, ".weight.", i), w[i])
  if (res$p == 1) { pr <- predict(res); put(paste0(p, ".pi.lower"), pr$pi.lb); put(paste0(p, ".pi.upper"), pr$pi.ub) }
  for (j in 1:2) {
    s2 <- c(NA, NA); s2[j] <- 0
    r0 <- update(res, sigma2 = s2, control = list())
    a <- anova(res, r0); put(paste0(p, ".lrt.", j), a$LRT); put(paste0(p, ".lrt.", j, ".p"), a$pval)
  }
}
ml3("ml_k", rma.mv(yi, vi, random = ~ 1 | district/school, data = kd, control = mtight), kd)
ml3("ml_kml", rma.mv(yi, vi, random = ~ 1 | district/school, data = kd, method = "ML", control = mtight), kd)
ml3("ml_kt", rma.mv(yi, vi, random = ~ 1 | district/school, data = kd, test = "t", control = mtight), kd)
ml3("ml_a", rma.mv(yi, vi, mods = ~ year, random = ~ 1 | study/esid, data = as, control = mtight), as)
ml3("ml_a0", rma.mv(yi, vi, random = ~ 1 | study/esid, data = as, control = list()), as)

lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, ", metafor ", packageVersion("metafor"), '",'), paste(lines, collapse = ",\n"), "}"), "meta_reference.json")
