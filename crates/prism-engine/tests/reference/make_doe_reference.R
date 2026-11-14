# Reference values for designed experiments (prism-engine/tests/doe_reference.rs), from FrF2, DoE.base, rsm, BsMD and
# AlgDesign.
# Run from this folder: Rscript make_doe_reference.R
# Writes filtration.csv, replicated.csv, chemreact.csv, yarn.csv and blend.csv and doe_reference.json.
suppressMessages({library(FrF2); library(rsm); library(BsMD); library(AlgDesign); library(rcdd)})
v <- c(); txt <- list()
put <- function(name, x) if (length(x) == 1 && !is.na(x)) v[[name]] <<- unname(x)
mat <- function(p, x) {
  x <- as.matrix(x)
  put(paste0(p, ".runs"), nrow(x))
  for (i in seq_len(nrow(x))) for (j in seq_len(ncol(x))) put(paste0(p, ".x.", i, ".", j), x[i, j])
}
# Fractional factorials (standard order).
for (d in list(c(4, 3), c(8, 4), c(8, 5), c(8, 7), c(16, 6), c(16, 8), c(32, 7), c(64, 10))) {
  p <- paste0("frf2_", d[1], "_", d[2])
  des <- FrF2(d[1], d[2], randomize = FALSE)
  mat(p, desnum(des))
  info <- design.info(des)
  e <- catlg[[paste0(d[2], "-", d[2] - log2(d[1]), ".1")]]
  if (!is.null(e)) {
    put(paste0(p, ".resolution"), e$res)
    wlp <- e$WLP
    for (i in 3:length(wlp)) put(paste0(p, ".wlp.", i), wlp[i])
  }
  txt[[paste0(p, ".alias.main")]] <- paste(info$aliased$main, collapse = "; ")
  txt[[paste0(p, ".alias.fi2")]] <- paste(info$aliased$fi2, collapse = "; ")
}
# Full factorial: two- and three-level factors (fac.design codes levels 1, 2, ...).
mat("full_2_3_2", sapply(as.data.frame(fac.design(nlevels = c(2, 3, 2), randomize = FALSE)), as.integer))
# Central composite designs.
cc <- ccd(3, n0 = c(4, 2), randomize = FALSE); mat("ccd3", as.data.frame(cc)[, c("x1", "x2", "x3")]); put("ccd3.alpha", max(abs(as.data.frame(cc)$x1)))
for (i in seq_len(nrow(cc))) put(paste0("ccd3.block.", i), as.integer(as.data.frame(cc)$Block[i]))
cr <- ccd(2, n0 = c(3, 3), alpha = "rotatable", randomize = FALSE); mat("ccd2r", as.data.frame(cr)[, c("x1", "x2")])
cf <- ccd(4, n0 = c(2, 2), alpha = "faces", randomize = FALSE); mat("ccd4f", as.data.frame(cf)[, paste0("x", 1:4)])
# Box-Behnken.
for (k in 3:5) {
  b <- as.data.frame(bbd(k, n0 = 2, randomize = FALSE))
  mat(paste0("bbd", k), b[, paste0("x", 1:k)])
  if ("Block" %in% names(b)) for (i in seq_len(nrow(b))) put(paste0("bbd", k, ".block.", i), as.integer(b$Block[i]))
}
# Unreplicated 2^4: Montgomery's filtration-rate experiment (Example 6.2).
fil <- expand.grid(A = c(-1, 1), B = c(-1, 1), C = c(-1, 1), D = c(-1, 1))
fil$rate <- c(45, 71, 48, 65, 68, 60, 80, 65, 43, 100, 45, 104, 75, 86, 70, 96)
write.csv(fil, "filtration.csv", row.names = FALSE)
m <- lm(rate ~ A * B * C * D, data = fil)
ef <- 2 * coef(m)[-1]
# Order the effects as PrismStatistics does: by order, then standard order.
nm <- names(ef); ord <- order(sapply(strsplit(nm, ":"), length), seq_along(nm))
for (i in seq_along(ord)) put(paste0("fil.effect.", i, ".est"), ef[ord[i]])
L <- LenthPlot(m, plt = FALSE)
put("fil.lenth.pse", L[["PSE"]]); put("fil.lenth.me", L[["ME"]]); put("fil.lenth.sme", L[["SME"]])
# A replicated 2^3 with center points (t tests on the residual df).
set.seed(5)
rep3 <- rbind(expand.grid(A = c(10, 20), B = c(1, 3), C = c("lo", "hi")), expand.grid(A = c(10, 20), B = c(1, 3), C = c("lo", "hi")))
rep3$C <- as.character(rep3$C)
rep3$y <- round(50 + 4 * ifelse(rep3$A == 20, 1, -1) - 3 * ifelse(rep3$B == 3, 1, -1) + 2 * ifelse(rep3$A == 20, 1, -1) * ifelse(rep3$B == 3, 1, -1) + rnorm(16, 0, 1.5), 2)
write.csv(rep3, "replicated.csv", row.names = FALSE)
rc <- data.frame(A = ifelse(rep3$A == 20, 1, -1), B = ifelse(rep3$B == 3, 1, -1), C = ifelse(rep3$C == "lo", 1, -1), y = rep3$y)
m2 <- lm(y ~ A * B * C, data = rc)
cf2 <- summary(m2)$coefficients
nm <- rownames(cf2)[-1]; ord <- order(sapply(strsplit(nm, ":"), length), seq_along(nm))
for (i in seq_along(ord)) {
  r <- nm[ord[i]]
  put(paste0("rep.effect.", i, ".est"), 2 * cf2[r, 1]); put(paste0("rep.effect.", i, ".se"), 2 * cf2[r, 2])
  put(paste0("rep.effect.", i, ".t"), cf2[r, 3]); put(paste0("rep.effect.", i, ".p"), cf2[r, 4])
}
put("rep.df", df.residual(m2))
# Response surface: rsm's ChemReact example with blocks.
CR <- coded.data(ChemReact, x1 ~ (Time - 85) / 5, x2 ~ (Temp - 175) / 5)
write.csv(data.frame(Block = as.character(ChemReact$Block), x1 = CR$x1, x2 = CR$x2, Yield = CR$Yield), "chemreact.csv", row.names = FALSE)
r <- rsm(Yield ~ Block + SO(x1, x2), data = CR)
s <- summary(r)
co <- s$coefficients
for (j in seq_len(nrow(co))) { put(paste0("rsm.coef.", j, ".est"), co[j, 1]); put(paste0("rsm.coef.", j, ".se"), co[j, 2]); put(paste0("rsm.coef.", j, ".t"), co[j, 3]); put(paste0("rsm.coef.", j, ".p"), co[j, 4]) }
put("rsm.r2", s$r.squared); put("rsm.adj_r2", s$adj.r.squared)
a <- s$lof
put("rsm.anova.block.ss", a["Block", "Sum Sq"]); put("rsm.anova.fo.ss", a["FO(x1, x2)", "Sum Sq"]); put("rsm.anova.twi.ss", a["TWI(x1, x2)", "Sum Sq"]); put("rsm.anova.pq.ss", a["PQ(x1, x2)", "Sum Sq"])
put("rsm.anova.fo.f", a["FO(x1, x2)", "F value"]); put("rsm.anova.pq.p", a["PQ(x1, x2)", "Pr(>F)"])
put("rsm.anova.residual.ss", a["Residuals", "Sum Sq"]); put("rsm.lof.ss", a["Lack of fit", "Sum Sq"]); put("rsm.lof.f", a["Lack of fit", "F value"]); put("rsm.lof.p", a["Lack of fit", "Pr(>F)"]); put("rsm.pe.ss", a["Pure error", "Sum Sq"])
put("rsm.stationary.1", s$canonical$xs[1]); put("rsm.stationary.2", s$canonical$xs[2])
put("rsm.eigen.1", s$canonical$eigen$values[1]); put("rsm.eigen.2", s$canonical$eigen$values[2])
put("rsm.stationary.y", predict(r, newdata = data.frame(Block = "B1", x1 = s$canonical$xs[1], x2 = s$canonical$xs[2])))
# Plackett-Burman designs (FrF2's pb, standard order).
for (d in list(c(12, 11), c(12, 7), c(20, 19), c(20, 10), c(24, 23), c(36, 30), c(44, 43), c(48, 47))) {
  mat(paste0("pb_", d[1], "_", d[2]), suppressWarnings(desnum(pb(d[1], d[2], randomize = FALSE))))
}
# Simplex lattices (AlgDesign's gen.mixture takes m + 1 levels) and simplex centroids.
for (d in list(c(3, 2), c(3, 3), c(4, 3), c(5, 2), c(4, 1))) mat(paste0("sld_", d[1], "_", d[2]), gen.mixture(d[2] + 1, d[1]))
scd <- function(q) do.call(rbind, lapply(1:q, function(s) t(apply(combn(q, s), 2, function(ix) { x <- rep(0, q); x[ix] <- 1 / s; x }))))
axial <- function(q) t(sapply(1:q, function(i) { x <- rep(1 / (2 * q), q); x[i] <- (q + 1) / (2 * q); x }))
mat("scd_3", scd(3)); mat("scd_4", scd(4)); mat("scd_4a", rbind(scd(4), axial(4)))
mat("sld_3_2a", rbind(as.matrix(gen.mixture(3, 3)), rep(1 / 3, 3), axial(3)))
# D-optimal designs: the best of many Federov searches, and AlgDesign's evaluation of that design.
opt <- function(p, k, frml, levels, n) {
  cand <- gen.factorial(levels, k, varNames = LETTERS[1:k])
  set.seed(11); o <- optFederov(frml, cand, nTrials = n, nRepeats = 300)
  e <- eval.design(frml, o$design)
  put(paste0(p, ".d"), o$D); put(paste0(p, ".a"), o$A); put(paste0(p, ".eval.d"), e$determinant); put(paste0(p, ".eval.a"), e$A)
  mat(p, o$design)
}
opt("opt_q3", 3, ~quad(A, B, C), 3, 14)
opt("opt_q2", 2, ~quad(A, B), 3, 7)
opt("opt_q3b", 3, ~quad(A, B, C), 3, 20)
opt("opt_i4", 4, ~(A + B + C + D)^2, 2, 12)
opt("opt_l5", 5, ~A + B + C + D + E, 2, 9)
opt("opt_q4", 4, ~quad(A, B, C, D), 3, 18)
# Mixture models: Cornell's yarn elongation ({3, 2} lattice, replicated) and a special cubic on an augmented
# centroid design, its components in percent.
yarn <- data.frame(x1 = c(1, 1, .5, .5, .5, 0, 0, 0, 0, 0, 0, 0, 0, .5, .5, .5), x2 = c(0, 0, .5, .5, .5, 1, 1, .5, .5, .5, 0, 0, 0, 0, 0, 0),
                   x3 = c(0, 0, 0, 0, 0, 0, 0, .5, .5, .5, 1, 1, 1, .5, .5, .5),
                   elongation = c(11, 12.4, 15, 14.8, 16.1, 8.8, 10, 10, 9.7, 11.8, 16.8, 16, 15.2, 17.7, 16.4, 16.6))
write.csv(yarn, "yarn.csv", row.names = FALSE)
mix <- function(p, d, terms, intercept_terms) {
  m <- lm(as.formula(paste("y ~ -1 +", terms)), data = d)
  co <- summary(m)$coefficients
  for (j in seq_len(nrow(co))) { put(paste0(p, ".coef.", j, ".est"), co[j, 1]); put(paste0(p, ".coef.", j, ".se"), co[j, 2]); put(paste0(p, ".coef.", j, ".t"), co[j, 3]); put(paste0(p, ".coef.", j, ".p"), co[j, 4]) }
  m1 <- lm(as.formula(paste("y ~", intercept_terms)), data = d); s1 <- summary(m1)
  stopifnot(abs(deviance(m1) - deviance(m)) < 1e-8)
  put(paste0(p, ".r2"), s1$r.squared); put(paste0(p, ".adj_r2"), s1$adj.r.squared); put(paste0(p, ".f"), s1$fstatistic[1])
  put(paste0(p, ".f.p"), pf(s1$fstatistic[1], s1$fstatistic[2], s1$fstatistic[3], lower.tail = FALSE)); put(paste0(p, ".sigma"), s1$sigma)
  cell <- interaction(d$x1, d$x2, d$x3, drop = TRUE)
  if (nlevels(cell) < nrow(d) && nlevels(cell) > length(coef(m))) {
    a <- anova(m, lm(y ~ cell - 1, data = d))
    put(paste0(p, ".lof.ss"), a$`Sum of Sq`[2]); put(paste0(p, ".lof.f"), a$F[2]); put(paste0(p, ".lof.p"), a$`Pr(>F)`[2]); put(paste0(p, ".pe.ss"), a$RSS[2])
  }
  put(paste0(p, ".centroid.y"), predict(m, newdata = data.frame(x1 = 1 / 3, x2 = 1 / 3, x3 = 1 / 3)))
}
yd <- data.frame(x1 = yarn$x1, x2 = yarn$x2, x3 = yarn$x3, y = yarn$elongation)
mix("yarn", yd, "x1 + x2 + x3 + x1:x2 + x1:x3 + x2:x3", "x2 + x3 + x1:x2 + x1:x3 + x2:x3")
mix("yarnl", yd, "x1 + x2 + x3", "x2 + x3")
bl <- as.data.frame(rbind(scd(3), axial(3), scd(3)[c(1, 4, 7), ])); names(bl) <- c("x1", "x2", "x3")
set.seed(3)
bl$y <- round(with(bl, 20 * x1 + 30 * x2 + 25 * x3 + 15 * x1 * x2 - 10 * x2 * x3 + 90 * x1 * x2 * x3) + rnorm(nrow(bl), 0, 0.8), 2)
write.csv(data.frame(a = 100 * bl$x1, b = 100 * bl$x2, c = 100 * bl$x3, y = bl$y), "blend.csv", row.names = FALSE)
mix("blend", bl, "x1 + x2 + x3 + x1:x2 + x1:x3 + x2:x3 + x1:x2:x3", "x2 + x3 + x1:x2 + x1:x3 + x2:x3 + x1:x2:x3")
# I-optimal designs: AlgDesign's best of many searches and its evaluation of that design (I over the candidates).
iopt <- function(p, k, frml, levels, n) {
  cand <- gen.factorial(levels, k, varNames = LETTERS[1:k])
  set.seed(12); o <- optFederov(frml, cand, nTrials = n, criterion = "I", nRepeats = 200)
  put(paste0(p, ".i"), o$I); put(paste0(p, ".eval.i"), eval.design(frml, o$design, X = cand)$I)
  mat(p, o$design)
}
iopt("iopt_q2", 2, ~quad(A, B), 3, 8)
iopt("iopt_q3", 3, ~quad(A, B, C), 3, 14)
iopt("iopt_i4", 4, ~(A + B + C + D)^2, 2, 14)
# Extreme vertices of constrained mixture regions, by cdd's exact vertex enumeration (rcdd), sorted.
verts <- function(p, lo, hi) {
  q <- length(lo)
  # Inequalities b - A x >= 0 in cdd's H-representation: rows (0, b, -A); the equality sum x = 1 as a linearity row.
  hrep <- rbind(c(1, 1, rep(-1, q)), cbind(0, -lo, diag(q)), cbind(0, hi, -diag(q)))
  hrep <- d2q(hrep)
  sc <- scdd(hrep, adjacency = TRUE)
  v <- q2d(sc$output)
  # Edges between distinct vertices (cdd's adjacency of its listed vertices, duplicates merged).
  key <- apply(round(v[, -(1:2), drop = FALSE], 12), 1, paste, collapse = ",")
  ed <- unique(do.call(rbind, lapply(seq_along(sc$adjacency), function(i) if (length(sc$adjacency[[i]])) t(apply(cbind(key[i], key[sc$adjacency[[i]]]), 1, sort)))))
  ed <- ed[ed[, 1] != ed[, 2], , drop = FALSE]
  put(paste0(p, ".edges"), nrow(ed))
  v <- unique(round(v[, -(1:2), drop = FALSE], 12))  # cdd lists a degenerate vertex once per basis
  v <- v[do.call(order, as.data.frame(v)), , drop = FALSE]
  mat(p, v)
}
verts("xv_flare", c(0.4, 0.1, 0.1, 0.03), c(0.6, 0.5, 0.5, 0.08))
verts("xv_3", c(0.1, 0.2, 0.1), c(0.5, 0.6, 0.4))
verts("xv_5", c(0.05, 0.1, 0, 0.2, 0.1), c(0.3, 0.4, 0.25, 0.5, 0.35))
lines <- c(sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v))), sprintf('  "%s": "%s"', names(txt), unlist(txt)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, ", FrF2 ", packageVersion("FrF2"), ", rsm ", packageVersion("rsm"), ", BsMD ", packageVersion("BsMD"), ", AlgDesign ", packageVersion("AlgDesign"), '",'), paste(lines, collapse = ",\n"), "}"), "doe_reference.json")
