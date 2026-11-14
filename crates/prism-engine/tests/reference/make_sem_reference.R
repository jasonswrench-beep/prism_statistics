# Reference values for structural equation modeling (prism-engine/tests/sem_reference.rs), from lavaan.
# Run from this folder: Rscript make_sem_reference.R
# Writes the two public example datasets the tests use (Holzinger and Swineford 1939; Bollen's political democracy
# data) and sem_reference.json.
library(lavaan)
hs <- HolzingerSwineford1939
write.csv(hs[, c("id", "sex", "ageyr", "school", "grade", paste0("x", 1:9))], "holzinger.csv", row.names = FALSE, na = "")
write.csv(PoliticalDemocracy, "poldem.csv", row.names = FALSE)

v <- c()
put <- function(name, x) v[[name]] <<- unname(x)
dump <- function(p, fit, mi = character()) {
  pe <- parameterEstimates(fit)
  st <- standardizedSolution(fit)
  gl <- lavInspect(fit, "group.label")
  for (i in seq_len(nrow(pe))) {
    # In multigroup models each parameter is keyed by its group's label.
    gp <- if (length(gl) > 1 && pe$group[i] > 0) paste0(gl[pe$group[i]], ":") else ""
    # In two-level models, by level: "within:" or "between:".
    if (!is.null(pe$level) && pe$level[i] > 0) gp <- paste0(c("within", "between")[pe$level[i]], ":")
    k <- paste0(p, ".", gp, pe$lhs[i], pe$op[i], pe$rhs[i])
    put(paste0(k, ".est"), pe$est[i])
    if (!is.na(pe$se[i]) && pe$se[i] > 0) put(paste0(k, ".se"), pe$se[i])
    put(paste0(k, ".std"), st$est.std[i])
    if (!is.na(st$se[i]) && st$se[i] > 0) put(paste0(k, ".stdse"), st$se[i])
  }
  fm <- fitMeasures(fit)
  for (m in c("npar", "chisq", "df", "pvalue", "baseline.chisq", "baseline.df", "cfi", "tli", "rmsea", "rmsea.ci.lower",
              "rmsea.ci.upper", "rmsea.pvalue", "srmr", "aic", "bic", "logl", "unrestricted.logl", "chisq.scaled",
              "chisq.scaling.factor", "pvalue.scaled", "baseline.chisq.scaled", "baseline.chisq.scaling.factor", "cfi.scaled",
              "tli.scaled", "rmsea.scaled", "cfi.robust", "tli.robust", "rmsea.robust", "rmsea.ci.lower.robust",
              "rmsea.ci.upper.robust", "srmr_within", "srmr_between")) if (!is.null(fm[m]) && !is.na(fm[m])) put(paste0(p, ".fit.", m), fm[[m]])
  r2 <- lavInspect(fit, "r2")
  if (lavInspect(fit, "nlevels") > 1) {
    for (l in 1:2) for (n in names(r2[[l]])) put(paste0(p, ".r2.", c("within", "between")[l], ":", n), r2[[l]][[n]])
  } else if (length(gl) > 1) {
    for (g in seq_along(gl)) for (n in names(r2[[g]])) put(paste0(p, ".r2.", gl[g], ":", n), r2[[g]][[n]])
  } else {
    for (n in names(r2)) put(paste0(p, ".r2.", n), r2[[n]])
  }
  if (length(mi)) {
    x <- modindices(fit)
    for (r in mi) {
      row <- x[paste0(x$lhs, x$op, x$rhs) == r, ]
      put(paste0(p, ".mi.", r), row$mi); put(paste0(p, ".epc.", r), row$epc); put(paste0(p, ".sepc.", r), row$sepc.all)
    }
  }
}
# lavaan's optimizer (nlminb) stops at a relative change of about 1e-10 in the fit function, which leaves estimates
# good to about six digits; the references are fitted to full precision instead. At this tolerance nlminb reaches the
# optimum (the estimates agree to 12 digits with rel.tol = 1e-14) but reports "false convergence", so lavaan is told
# to accept it.
tight <- list(rel.tol = 1e-13, iter.max = 10000, eval.max = 20000)
cfa <- function(...) lavaan::cfa(..., control = tight, optim.force.converged = TRUE)
sem <- function(...) lavaan::sem(..., control = tight, optim.force.converged = TRUE)

# Confirmatory factor analysis, identified by marker indicators, then by fixed factor variances.
hsm <- "visual =~ x1 + x2 + x3
textual =~ x4 + x5 + x6
speed =~ x7 + x8 + x9"
dump("cfa", cfa(hsm, hs), c("visual=~x9", "x7~~x8", "textual=~x1", "x2~~x3"))
dump("cfastd", cfa(hsm, hs, std.lv = TRUE))
dump("cfamean", cfa(hsm, hs, meanstructure = TRUE))
# A higher-order factor.
dump("ho", cfa(paste(hsm, "g =~ visual + textual + speed", sep = "\n"), hs))

# Full SEM: Bollen's political democracy model with equal loadings across waves and defined effects.
pd <- "ind60 =~ x1 + x2 + x3
dem60 =~ y1 + a*y2 + b*y3 + c*y4
dem65 =~ y5 + a*y6 + b*y7 + c*y8
dem60 ~ g1*ind60
dem65 ~ g2*ind60 + g3*dem60
y1 ~~ y5
y2 ~~ y4 + y6
y3 ~~ y7
y4 ~~ y8
y6 ~~ y8
indirect := g1*g3
total := g2 + g1*g3"
dump("pd", sem(pd, PoliticalDemocracy), c("y2~~y3", "ind60=~y4"))

# Path analysis with observed variables: exogenous covariates fixed at their sample values, mediation, and two pure
# outcomes whose residuals covary.
dump("path", sem("x4 ~ a*x1 + x2
x9 ~ b*x4 + c*x1
x5 ~ x2
ab := a*b", hs))
# The same with the predictors' moments estimated (fixed.x = FALSE): their variances and covariance are free.
dump("pathfree", sem("x4 ~ a*x1 + x2
x9 ~ b*x4 + c*x1
x5 ~ x2
ab := a*b", hs, fixed.x = FALSE))

# Robust maximum likelihood: Satorra-Bentler (MLM) and Yuan-Bentler (MLR) standard errors and scaled tests.
dump("mlm", cfa(hsm, hs, estimator = "MLM"))
# The same with the weight at the sample moments (h1.information = "unstructured"), as some other packages compute it.
dump("mlmu", cfa(hsm, hs, estimator = "MLM", h1.information = "unstructured"))
dump("mlr", cfa(hsm, hs, estimator = "MLR"))
dump("pdmlr", sem(pd, PoliticalDemocracy, estimator = "MLR"))

# Full-information maximum likelihood with missing values, made by a fixed rule so the test data are the same.
hm <- hs[, c("id", paste0("x", 1:9))]
i <- seq_len(nrow(hm))
hm$x1[i %% 7 == 0] <- NA
hm$x5[i %% 11 == 3] <- NA
hm$x9[i %% 5 == 1 & i < 150] <- NA
hm$x2[i %% 13 == 4] <- NA
hm$x3[i %% 13 == 4] <- NA
hm[i %% 17 == 9, paste0("x", 4:9)] <- NA
write.csv(hm, "holzinger_missing.csv", row.names = FALSE, na = "")
dump("fiml", cfa(hsm, hm, missing = "ml"), c("visual=~x9", "x7~~x8"))
dump("fimlr", cfa(hsm, hm, missing = "ml", estimator = "MLR"))

# Multigroup models and measurement invariance across the two schools (groups in order of first appearance, as lavaan
# orders them: Pasteur, then Grant-White).
steps <- list(configural = character(), metric = "loadings", scalar = c("loadings", "intercepts"),
              strict = c("loadings", "intercepts", "residuals"))
fits <- list(); fitsr <- list()
for (st in names(steps)) {
  fits[[st]] <- cfa(hsm, hs, group = "school", group.equal = steps[[st]])
  dump(paste0("mg", st), fits[[st]])
  fitsr[[st]] <- cfa(hsm, hs, group = "school", group.equal = steps[[st]], estimator = "MLR")
  f <- fitMeasures(fitsr[[st]], c("chisq", "df", "chisq.scaled", "chisq.scaling.factor", "cfi.robust", "rmsea.robust"))
  for (m in names(f)) put(paste0("mgr", st, ".fit.", m), f[[m]])
}
for (k in 2:4) {
  a <- names(steps)[k - 1]; b <- names(steps)[k]
  l <- lavTestLRT(fits[[a]], fits[[b]])
  put(paste0("inv.", b, ".dchisq"), l[2, "Chisq diff"]); put(paste0("inv.", b, ".ddf"), l[2, "Df diff"]); put(paste0("inv.", b, ".p"), l[2, "Pr(>Chisq)"])
  l <- lavTestLRT(fitsr[[a]], fitsr[[b]])
  put(paste0("invr.", b, ".dchisq"), l[2, "Chisq diff"]); put(paste0("invr.", b, ".p"), l[2, "Pr(>Chisq)"])
}
# Partial invariance: the intercept of x3 free across groups.
dump("mgpartial", cfa(hsm, hs, group = "school", group.equal = c("loadings", "intercepts"), group.partial = "x3~1"))
# Equal loadings with factor variances fixed at 1 (the variances are then free in the second group).
dump("mgstd", cfa(hsm, hs, group = "school", group.equal = "loadings", std.lv = TRUE))

# Ordinal indicators by WLSMV (lavaan's defaults for ordered variables): each score cut at its quartiles.
ho <- data.frame(id = hs$id)
for (xv in paste0("x", 1:9)) ho[[xv]] <- cut(hs[[xv]], quantile(hs[[xv]], 0:4/4), include.lowest = TRUE, labels = FALSE)
write.csv(ho, "holzinger_ordinal.csv", row.names = FALSE)
fo <- cfa(hsm, ho, ordered = paste0("x", 1:9))
dump("ord", fo)
ts <- lavInspect(fo, "test")$scaled.shifted
put("ord.fit.shift", ts$shift.parameter)
pc <- lavInspect(fo, "sampstat")$cov
for (i in 2:9) for (j in 1:(i - 1)) put(paste0("ord.polychoric.", rownames(pc)[j], ".", rownames(pc)[i]), pc[i, j])
nacov <- lavInspect(fo, "gamma")
put("ord.gamma.1.1", nacov[1, 1]); put("ord.gamma.28.28", nacov[28, 28]); put("ord.gamma.28.29", nacov[28, 29]); put("ord.gamma.1.28", nacov[1, 28])

# Latent growth curves (lavaan's growth(): observed intercepts fixed at 0, latent means free), unconditional and with
# time-invariant and time-varying covariates.
write.csv(Demo.growth, "growth.csv", row.names = FALSE)
lg <- "i =~ 1*t1 + 1*t2 + 1*t3 + 1*t4
s =~ 0*t1 + 1*t2 + 2*t3 + 3*t4"
growth <- function(...) lavaan::growth(..., control = tight, optim.force.converged = TRUE)
dump("lgm", growth(lg, Demo.growth))
dump("lgmc", growth(paste(lg, "i + s ~ x1 + x2", "t1 ~ c1
t2 ~ c2
t3 ~ c3
t4 ~ c4", sep = "
"), Demo.growth))

# Two-level models (random intercepts) on lavaan's Demo.twolevel: lavaan's own example, with within-level and
# between-level covariates, and a two-level CFA. lavaan's two-level optimizer is run with its defaults (the tighter
# control above breaks it).
write.csv(Demo.twolevel, "twolevel.csv", row.names = FALSE)
dump("ml", lavaan::sem("level: 1
  fw =~ y1 + y2 + y3
  fw ~ x1 + x2 + x3
level: 2
  fb =~ y1 + y2 + y3
  fb ~ w1 + w2", Demo.twolevel, cluster = "cluster"))
dump("mlcfa", lavaan::sem("level: 1
  fw =~ y1 + y2 + y3
level: 2
  fb =~ y1 + y2 + y3", Demo.twolevel, cluster = "cluster"))

# Reliability and validity (semTools defaults): omega from the observed total variance, AVE from the observed variances,
# and the heterotrait-monotrait ratio (HTMT2, geometric means; and the original HTMT, arithmetic means).
suppressMessages(library(semTools))
f <- cfa(hsm, hs)
om <- compRelSEM(f); av <- AVE(f)
for (k in names(om)) { put(paste0("rel.omega.", k), om[[k]]); put(paste0("rel.ave.", k), av[[k]]) }
h2 <- htmt(hsm, hs); h1 <- htmt(hsm, hs, htmt2 = FALSE)
for (a in rownames(h2)) for (b in colnames(h2)) if (a != b && match(a, rownames(h2)) > match(b, colnames(h2))) {
  put(paste0("rel.htmt2.", b, ".", a), h2[a, b]); put(paste0("rel.htmt.", b, ".", a), h1[a, b])
}

lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, ", lavaan ", packageVersion("lavaan"), '",'), paste(lines, collapse = ",\n"), "}"), "sem_reference.json")
