# Reference values for the analyses added after the preview (prism-engine/tests/more_reference.rs).
# Run from this folder: Rscript make_more_reference.R
# Uses only base R. preview.csv is described in make_preview_data.R and scale.csv in make_scale_data.R; user-missing
# codes are set to NA by hand here, so the tests check that PrismStatistics leaves out the same cases.
d <- read.csv("preview.csv")
d$cond[d$cond %in% 9] <- NA
d$pre[d$pre %in% -99] <- NA
d$post[d$post %in% -99] <- NA
s <- read.csv("scale.csv")
for (i in c("i1", "i2", "i3", "i4", "i5")) s[[i]][s[[i]] %in% 9] <- NA
v <- c()
put <- function(name, x) v[[name]] <<- unname(x)

# Fisher's exact test: sex by smoker.
ft <- fisher.test(table(d$sex, d$smoker))
# fisher.test finds the conditional odds ratio with uniroot at its default tolerance (about 1e-4), so the estimate is
# also solved here to full precision.
x <- table(d$sex, d$smoker); m <- sum(x[, 1]); n <- sum(x[, 2]); k <- sum(x[1, ]); s0 <- max(0, k - n):min(k, m)
ld <- dhyper(s0, m, n, k, log = TRUE)
mn <- function(t) { w <- exp(ld + t * s0 - max(ld + t * s0)); sum(s0 * w) / sum(w) }
put("fisher.or.exact", exp(uniroot(function(t) mn(t) - x[1, 1], c(-10, 10), tol = 1e-14)$root))
pw <- function(t) { w <- exp(ld + t * s0 - max(ld + t * s0)); w / sum(w) }
put("fisher.lo.exact", exp(uniroot(function(t) sum(pw(t)[s0 >= x[1, 1]]) - 0.025, c(-10, 10), tol = 1e-14)$root))
put("fisher.hi.exact", exp(uniroot(function(t) sum(pw(t)[s0 <= x[1, 1]]) - 0.025, c(-10, 10), tol = 1e-14)$root))
put("fisher.p", ft$p.value); put("fisher.or", ft$estimate); put("fisher.lo", ft$conf.int[1]); put("fisher.hi", ft$conf.int[2])

# McNemar: before and after.
m <- s[!is.na(s$before) & !is.na(s$after), ]
mt <- table(m$before, m$after)
mc <- mcnemar.test(mt)
mu <- mcnemar.test(mt, correct = FALSE)
put("mc.n", nrow(m)); put("mc.b", mt[1, 2]); put("mc.c", mt[2, 1])
put("mc.chi2", mc$statistic); put("mc.p", mc$p.value); put("mc.chi2u", mu$statistic); put("mc.pu", mu$p.value)
put("mc.pexact", binom.test(mt[1, 2], mt[1, 2] + mt[2, 1])$p.value)

# Friedman: i1, i2, i3 (i2 has a user-missing code, i3 an empty cell).
f3 <- s[complete.cases(s[, c("i1", "i2", "i3")]), c("i1", "i2", "i3")]
fr <- friedman.test(as.matrix(f3))
put("fr.n", nrow(f3)); put("fr.chi2", fr$statistic); put("fr.df", fr$parameter); put("fr.p", fr$p.value)
put("fr.W", fr$statistic / (nrow(f3) * 2))
rk <- t(apply(as.matrix(f3), 1, rank))
put("fr.mr1", mean(rk[, 1])); put("fr.mr3", mean(rk[, 3]))

# Partial correlation: hours and score controlling for pre, then for pre and post (Pearson on residuals).
pc <- function(x, y, z) cor(resid(lm(x ~ z)), resid(lm(y ~ z)))
c1 <- d[complete.cases(d[, c("hours", "score", "pre")]), ]
r1 <- pc(c1$hours, c1$score, c1$pre); df1 <- nrow(c1) - 3
t1 <- r1 * sqrt(df1 / (1 - r1^2))
put("pc1.n", nrow(c1)); put("pc1.r", r1); put("pc1.df", df1); put("pc1.p", 2 * pt(-abs(t1), df1))
put("pc1.lo", tanh(atanh(r1) - qnorm(0.975) / sqrt(nrow(c1) - 4))); put("pc1.hi", tanh(atanh(r1) + qnorm(0.975) / sqrt(nrow(c1) - 4)))
put("pc1.r0", cor(c1$hours, c1$score))
c2 <- d[complete.cases(d[, c("hours", "score", "pre", "post")]), ]
r2 <- pc(c2$hours, c2$score, cbind(c2$pre, c2$post)); df2 <- nrow(c2) - 4
put("pc2.n", nrow(c2)); put("pc2.r", r2); put("pc2.p", 2 * pt(-abs(r2 * sqrt(df2 / (1 - r2^2))), df2))

# Factorial ANOVA, Type III: score by cond and sex, then by cond, sex and smoker. Sum-to-zero contrasts, and each
# effect's sum of squares is the rise in the residual sum of squares when its columns are dropped from the full model.
type3 <- function(formula, data) {
  fit <- lm(formula, data = data, contrasts = lapply(Filter(is.factor, data), function(x) "contr.sum"))
  X <- model.matrix(fit); a <- attr(X, "assign"); rss <- sum(resid(fit)^2); dfe <- fit$df.residual
  labs <- attr(terms(fit), "term.labels")
  out <- lapply(seq_along(labs), function(j) {
    r0 <- sum(lm.fit(X[, a != j, drop = FALSE], data[[all.vars(formula)[1]]])$residuals^2)
    ss <- r0 - rss; df <- sum(a == j); f <- (ss / df) / (rss / dfe)
    c(ss = ss, df = df, F = f, p = pf(f, df, dfe, lower.tail = FALSE), pes = ss / (ss + rss))
  })
  names(out) <- labs
  list(effects = out, rss = rss, dfe = dfe, int = coef(summary(fit))[1, 3]^2, r2 = summary(fit)$r.squared)
}
fa <- d[complete.cases(d[, c("score", "cond", "sex")]), ]
fa$cond <- factor(fa$cond); fa$sex <- factor(fa$sex)
t2 <- type3(score ~ cond * sex, fa)
put("fa2.n", nrow(fa)); put("fa2.dfe", t2$dfe); put("fa2.rss", t2$rss); put("fa2.intF", t2$int); put("fa2.r2", t2$r2)
for (nm in names(t2$effects)) for (st in c("ss", "df", "F", "p", "pes")) put(paste0("fa2.", nm, ".", st), t2$effects[[nm]][[st]])
lev <- function(y, g) { m <- ave(y, g, FUN = median); summary(aov(abs(y - m) ~ g))[[1]] }
lv <- lev(fa$score, interaction(fa$cond, fa$sex))
put("fa2.leveneF", lv$`F value`[1]); put("fa2.levenep", lv$`Pr(>F)`[1])
put("fa2.emm.cond1", mean(tapply(fa$score[fa$cond == 1], fa$sex[fa$cond == 1], mean)))
fb <- d[complete.cases(d[, c("score", "cond", "sex", "smoker")]), ]
fb$cond <- factor(fb$cond); fb$sex <- factor(fb$sex); fb$smoker <- factor(fb$smoker)
t3 <- type3(score ~ cond * sex * smoker, fb)
put("fa3.n", nrow(fb)); put("fa3.dfe", t3$dfe)
for (nm in names(t3$effects)) for (st in c("ss", "F", "p")) put(paste0("fa3.", nm, ".", st), t3$effects[[nm]][[st]])

# Cronbach's alpha: i1 to i5 with i4 reversed (6 - value), and Feldt's 95% interval.
cronbach <- function(m) { k <- ncol(m); k / (k - 1) * (1 - sum(apply(m, 2, var)) / var(rowSums(m))) }
it <- s[complete.cases(s[, c("i1", "i2", "i3", "i4", "i5")]), c("i1", "i2", "i3", "i4", "i5")]
put("al.raw", cronbach(as.matrix(it)))
it$i4 <- 6 - it$i4
m <- as.matrix(it); n <- nrow(m); k <- ncol(m)
al <- cronbach(m)
put("al.n", n); put("al.alpha", al)
put("al.lo", 1 - (1 - al) * qf(0.975, n - 1, (n - 1) * (k - 1))); put("al.hi", 1 - (1 - al) * qf(0.025, n - 1, (n - 1) * (k - 1)))
rr <- cor(m); rbar <- mean(rr[upper.tri(rr)])
put("al.std", k * rbar / (1 + (k - 1) * rbar)); put("al.rbar", rbar)
put("al.rit4", cor(m[, 4], rowSums(m[, -4]))); put("al.del4", cronbach(m[, -4])); put("al.del1", cronbach(m[, -1]))

# Logistic regression: smoker = Yes from hours and cond (Lecture is the reference).
lg <- d[complete.cases(d[, c("smoker", "hours", "cond")]), ]
lg$ev <- as.integer(lg$smoker == 2); lg$f <- relevel(factor(lg$cond), ref = "1")
# glm stops at a relative change in deviance of 1e-8 by default; a tighter tolerance gives the estimates to full precision.
gl <- glm(ev ~ hours + f, family = binomial, data = lg, control = glm.control(epsilon = 1e-14, maxit = 100))
cf <- summary(gl)$coefficients; n <- nrow(lg)
put("lg.n", n); put("lg.events", sum(lg$ev)); put("lg.dev", deviance(gl)); put("lg.null", gl$null.deviance)
put("lg.chi2", gl$null.deviance - deviance(gl)); put("lg.p", pchisq(gl$null.deviance - deviance(gl), 3, lower.tail = FALSE))
cs <- 1 - exp((deviance(gl) - gl$null.deviance) / n)
put("lg.coxsnell", cs); put("lg.nagelkerke", cs / (1 - exp(-gl$null.deviance / n))); put("lg.aic", AIC(gl))
put("lg.b0", cf[1, 1]); put("lg.hours.b", cf[2, 1]); put("lg.hours.se", cf[2, 2]); put("lg.hours.p", cf[2, 4]); put("lg.hours.wald", cf[2, 3]^2)
put("lg.hours.or", exp(cf[2, 1])); put("lg.hours.orlo", exp(cf[2, 1] - qnorm(0.975) * cf[2, 2])); put("lg.hours.orhi", exp(cf[2, 1] + qnorm(0.975) * cf[2, 2]))
pci <- suppressMessages(confint(gl))  # profile-likelihood intervals (stats, R 4.4 and later)
put("lg.hours.orlo.profile", exp(pci["hours", 1])); put("lg.hours.orhi.profile", exp(pci["hours", 2]))
put("lg.f3.orlo.profile", exp(pci["f3", 1])); put("lg.f3.orhi.profile", exp(pci["f3", 2]))
put("lg.f3.b", cf["f3", 1]); put("lg.f3.p", cf["f3", 4])
dc <- deviance(glm(ev ~ hours, family = binomial, data = lg, control = glm.control(epsilon = 1e-14, maxit = 100))) - deviance(gl)
put("lg.cond.chi2", dc); put("lg.cond.p", pchisq(dc, 2, lower.tail = FALSE))
put("lg.accuracy", mean((fitted(gl) >= 0.5) == (lg$ev == 1)))

# Canonical correlation: pre and post with hours, score and sex. cancor's coefficients give variates with a sum of
# squares of one, so the standardized coefficients are those times each variable's SD times sqrt(n - 1). Signs are set
# so the first set's structure coefficients sum to a positive number, as PrismStatistics does.
cc <- d[complete.cases(d[, c("pre", "post", "hours", "score", "sex")]), ]
X <- as.matrix(cc[, c("pre", "post")]); Y <- as.matrix(cc[, c("hours", "score", "sex")])
ca <- cancor(X, Y); n <- nrow(cc); p <- 2; q <- 3
put("cc.n", n); put("cc.r1", ca$cor[1]); put("cc.r2", ca$cor[2])
for (i in 1:2) {
  uu <- scale(X, scale = FALSE) %*% ca$xcoef[, i]; vv <- scale(Y, scale = FALSE) %*% ca$ycoef[, i]
  sx <- cor(X, uu); sg <- sign(sum(sx))
  put(paste0("cc.f", i, ".a1"), sg * ca$xcoef[1, i] * sd(X[, 1]) * sqrt(n - 1)); put(paste0("cc.f", i, ".a2"), sg * ca$xcoef[2, i] * sd(X[, 2]) * sqrt(n - 1))
  put(paste0("cc.f", i, ".b2"), sg * ca$ycoef[2, i] * sd(Y[, 2]) * sqrt(n - 1))
  put(paste0("cc.f", i, ".sa1"), sg * sx[1]); put(paste0("cc.f", i, ".sb3"), sg * cor(Y, vv)[3])
  lam <- prod(1 - ca$cor[i:2]^2); pk <- p - i + 1; qk <- q - i + 1
  w <- n - 1.5 - (p + q) / 2; tt <- if (pk^2 + qk^2 - 5 > 0) sqrt((pk^2 * qk^2 - 4) / (pk^2 + qk^2 - 5)) else 1
  df1 <- pk * qk; df2 <- w * tt - pk * qk / 2 + 1; f <- (1 - lam^(1 / tt)) / lam^(1 / tt) * df2 / df1
  put(paste0("cc.f", i, ".lambda"), lam); put(paste0("cc.f", i, ".F"), f); put(paste0("cc.f", i, ".df2"), df2); put(paste0("cc.f", i, ".p"), pf(f, df1, df2, lower.tail = FALSE))
}

# Power: the formulas of the pwr package written out in base R (pwr isn't needed), two-sided, alpha .05. The sample
# size is the smallest whole number reaching the power asked for.
pw.t <- function(n, d, type) {
  df <- if (type == "two") 2 * n - 2 else n - 1; ncp <- if (type == "two") d * sqrt(n / 2) else d * sqrt(n)
  q <- qt(0.975, df); pt(q, df, ncp, lower.tail = FALSE) + pt(-q, df, ncp)
}
pw.f <- function(n, f, k) { d1 <- k - 1; d2 <- k * (n - 1); pf(qf(0.95, d1, d2), d1, d2, f^2 * k * n, lower.tail = FALSE) }
pw.r <- function(n, r) {
  tt <- qt(0.975, n - 2); rc <- sqrt(tt^2 / (tt^2 + n - 2)); zr <- atanh(r) + r / (2 * (n - 1)); zrc <- atanh(rc)
  pnorm((zr - zrc) * sqrt(n - 3)) + pnorm((-zr - zrc) * sqrt(n - 3))
}
pw.h <- function(n, h) { z <- qnorm(0.975); pnorm(abs(h) * sqrt(n) - z) + pnorm(-abs(h) * sqrt(n) - z) }
need <- function(f, from) { n <- from; while (f(n) < 0.8) n <- n + 1; n }
n1 <- need(function(n) pw.t(n, 0.5, "one"), 2); put("pw.one.n", n1); put("pw.one.power", pw.t(n1, 0.5, "one"))
n2 <- need(function(n) pw.t(n, 0.5, "two"), 2); put("pw.ind.n", n2); put("pw.ind.power", pw.t(n2, 0.5, "two"))
put("pw.ind.power50", pw.t(50, 0.4, "two"))
na <- need(function(n) pw.f(n, 0.25, 3), 2); put("pw.aov.n", na); put("pw.aov.power", pw.f(na, 0.25, 3))
mf <- c(10, 12, 14); fm <- sqrt(mean((mf - mean(mf))^2)) / 5
put("pw.aovraw.f", fm); put("pw.aovraw.power20", pw.f(20, fm, 3))
nr <- need(function(n) pw.r(n, 0.3), 4); put("pw.r.n", nr); put("pw.r.power", pw.r(nr, 0.3))
h1 <- 2 * asin(sqrt(0.65)) - 2 * asin(sqrt(0.5)); n3 <- need(function(n) pw.h(n, h1), 1); put("pw.p1.n", n3); put("pw.p1.power", pw.h(n3, h1))
h2 <- 2 * asin(sqrt(0.6)) - 2 * asin(sqrt(0.4)); n4 <- need(function(n) pw.h(n / 2, h2), 1); put("pw.p2.n", need(function(n) pw.h(n / 2, h2), 1))

# Classic nonparametric tests.
gof <- chisq.test(table(d$region))
put("gof.chi2", gof$statistic); put("gof.p", gof$p.value); put("gof.n", sum(table(d$region)))
gof2 <- chisq.test(table(d$region), p = c(1, 2, 2) / 5)  # East, North, South
put("gof2.chi2", gof2$statistic); put("gof2.p", gof2$p.value)
bt <- binom.test(sum(d$sex == 1), nrow(d), p = 0.5)
put("bin.x", bt$statistic); put("bin.p", bt$p.value); put("bin.lo", bt$conf.int[1]); put("bin.hi", bt$conf.int[2])
sc <- d$score[!is.na(d$score)]; up <- sc >= median(sc)
runs <- 1 + sum(up[-1] != up[-length(up)])
put("runs.runs", runs); put("runs.above", sum(up))
pp <- d[!is.na(d$pre) & !is.na(d$post), ]; st <- pp$pre - pp$post; st <- st[st != 0]
put("sign.p", binom.test(sum(st > 0), length(st))$p.value); put("sign.pos", sum(st > 0))
am <- d[!is.na(d$cond) & !is.na(d$score), ]; med <- median(am$score); mt <- table(factor(am$score > med, c(TRUE, FALSE)), am$cond)
mtest <- chisq.test(mt, correct = FALSE)
put("med.chi2", mtest$statistic); put("med.p", mtest$p.value)
kx <- d$score[d$sex == 1 & !is.na(d$score)]; ky <- d$score[d$sex == 2 & !is.na(d$score)]
ks <- suppressWarnings(ks.test(kx, ky))
put("ks.D", ks$statistic); put("ks.p", ks$p.value)

# Crosstabs measures (DescTools for the ordinal measures and Breslow-Day).
suppressMessages({ library(DescTools); library(irr) })
x22 <- table(d$sex, d$smoker)
a1 <- x22[1, 1]; b1 <- x22[1, 2]; c1 <- x22[2, 1]; d1 <- x22[2, 2]
or <- a1 * d1 / (b1 * c1); se <- sqrt(1 / a1 + 1 / b1 + 1 / c1 + 1 / d1)
put("risk.or", or); put("risk.or.lo", exp(log(or) - qnorm(0.975) * se)); put("risk.or.hi", exp(log(or) + qnorm(0.975) * se))
rr <- (a1 / (a1 + b1)) / (c1 / (c1 + d1)); se <- sqrt(1 / a1 - 1 / (a1 + b1) + 1 / c1 - 1 / (c1 + d1))
put("risk.rr1", rr); put("risk.rr1.lo", exp(log(rr) - qnorm(0.975) * se))
ot <- table(d$cond, d$region)  # cond 1-3 by region East, North, South
g <- GoodmanKruskalGamma(ot, conf.level = 0.95); put("ord.gamma", g[1]); put("ord.gamma.lo", g[2])
put("ord.taub", KendallTauB(ot, conf.level = 0.95)[1]); put("ord.taub.lo", KendallTauB(ot, conf.level = 0.95)[2])
put("ord.tauc", StuartTauC(ot, conf.level = 0.95)[1]); put("ord.tauc.lo", StuartTauC(ot, conf.level = 0.95)[2])
put("ord.somers_c", SomersDelta(ot, direction = "column", conf.level = 0.95)[1]); put("ord.somers_c.lo", SomersDelta(ot, direction = "column", conf.level = 0.95)[2])
put("ord.somers_r", SomersDelta(ot, direction = "row")[1])
x3 <- table(d$sex, d$smoker, d$region)
mh <- mantelhaen.test(x3, correct = FALSE)
put("cmh.chi2", mh$statistic); put("cmh.p", mh$p.value); put("cmh.or", mh$estimate); put("cmh.or.lo", mh$conf.int[1]); put("cmh.or.hi", mh$conf.int[2])
bd <- BreslowDayTest(x3); put("bd.chi2", bd$statistic); put("bd.p", bd$p.value)
ba <- s[complete.cases(s[, c("before", "after")]), c("before", "after")]; cq <- CochranQTest(as.matrix(ba) == 2)
put("cq.Q", cq$statistic); put("cq.p", cq$p.value)

# Agreement: the items as raters (i2 has a user-missing code, i3 an empty cell).
r12 <- s[complete.cases(s[, c("i1", "i2")]), c("i1", "i2")]
k0 <- kappa2(r12); kw <- kappa2(r12, "equal"); kq <- kappa2(r12, "squared")
put("kap.n", nrow(r12)); put("kap.k", k0$value); put("kap.z", k0$statistic); put("kap.p", k0$p.value)
put("kapw.k", kw$value); put("kapw.z", kw$statistic); put("kapq.k", kq$value); put("kapq.z", kq$statistic)
ck <- suppressWarnings(psych::cohen.kappa(as.matrix(r12)))
put("kap.lo", ck$confid[1, 1]); put("kap.hi", ck$confid[1, 3])
r3 <- s[complete.cases(s[, c("i1", "i2", "i3")]), c("i1", "i2", "i3")]
fk <- kappam.fleiss(r3); put("fl.k", fk$value); put("fl.z", fk$statistic); put("fl.p", fk$p.value)
km <- t(as.matrix(s[, c("i1", "i2", "i3")]))
put("kr.nominal", kripp.alpha(km, "nominal")$value); put("kr.ordinal", kripp.alpha(km, "ordinal")$value)
put("kr.interval", kripp.alpha(km, "interval")$value); put("kr.ratio", kripp.alpha(km, "ratio")$value)
for (mod in c("oneway", "twoway")) for (ty in c("consistency", "agreement")) for (un in c("single", "average")) {
  if (mod == "oneway" && ty == "agreement") next
  ic <- icc(r3, model = mod, type = ty, unit = un)
  nm <- paste("icc", mod, ty, un, sep = ".")
  put(nm, ic$value); put(paste0(nm, ".lo"), ic$lbound); put(paste0(nm, ".hi"), ic$ubound); put(paste0(nm, ".F"), ic$Fvalue); put(paste0(nm, ".p"), ic$p.value)
}

# Explore: score overall and in condition 2 (Discussion).
ex <- d$score[!is.na(d$score)]
n <- length(ex)
put("ex.trim", mean(ex, trim = 0.05)); put("ex.p10", quantile(ex, 0.10, type = 7)); put("ex.p95", quantile(ex, 0.95, type = 7))
put("ex.skew", DescTools::Skew(ex, method = 2)); put("ex.kurt", DescTools::Kurt(ex, method = 2))
put("ex.skew.se", sqrt(6 * n * (n - 1) / ((n - 2) * (n + 1) * (n + 3))))
put("ex.kurt.se", 2 * sqrt(6 * n * (n - 1) / ((n - 2) * (n + 1) * (n + 3))) * sqrt((n^2 - 1) / ((n - 3) * (n + 5))))
put("ex.sw.W", shapiro.test(ex)$statistic); put("ex.sw.p", shapiro.test(ex)$p.value)
li <- DescTools::LillieTest(ex); put("ex.ks.D", li$statistic); put("ex.ks.p", li$p.value)
e2 <- d$score[d$cond %in% 2 & !is.na(d$score)]
put("ex2.sw.p", shapiro.test(e2)$p.value); put("ex2.ks.p", DescTools::LillieTest(e2)$p.value); put("ex2.trim", mean(e2, trim = 0.05))

# Hierarchical regression: hours, then cond, then hours x cond, with hours mean-centered.
hr <- d[complete.cases(d[, c("score", "hours", "cond")]), ]
hr$hc <- hr$hours - mean(hr$hours); hr$f <- relevel(factor(hr$cond), ref = "1")
h1 <- lm(score ~ hc, hr); h2 <- lm(score ~ hc + f, hr); h3 <- lm(score ~ hc + f + hc:f, hr)
put("hier.r2.1", summary(h1)$r.squared); put("hier.r2.3", summary(h3)$r.squared)
a12 <- anova(h1, h2); a23 <- anova(h2, h3)
put("hier.dr2.2", summary(h2)$r.squared - summary(h1)$r.squared); put("hier.F.2", a12$F[2]); put("hier.p.2", a12$`Pr(>F)`[2])
put("hier.F.3", a23$F[2]); put("hier.df1.3", a23$Df[2]); put("hier.p.3", a23$`Pr(>F)`[2])
put("hier.int.b", coef(h3)["hc:f3"]); put("hier.hc.b", coef(h3)["hc"])

# Logistic regression in blocks (hours, then cond) and the ROC curve with DeLong's interval.
ctl <- glm.control(epsilon = 1e-14, maxit = 100)
lb1 <- glm(ev ~ hours, binomial, lg, control = ctl); lb2 <- glm(ev ~ hours + f, binomial, lg, control = ctl)
ch <- deviance(lb1) - deviance(lb2)
put("lgb.chi2.1", lb1$null.deviance - deviance(lb1)); put("lgb.chi2.2", ch); put("lgb.p.2", pchisq(ch, 2, lower.tail = FALSE))
rc <- pROC::roc(lg$ev, fitted(lb2), quiet = TRUE, direction = "<")
ci <- pROC::ci.auc(rc, method = "delong")
put("roc.auc", as.numeric(rc$auc)); put("roc.lo", ci[1]); put("roc.hi", ci[3])

# Factorial ANOVA follow-ups (car, emmeans): Type II, EMMs, pairwise comparisons, simple effects, ANCOVA.
suppressMessages({ library(car); library(emmeans) })
fa$cond <- factor(fa$cond); fa$sex <- factor(fa$sex)
opt <- options(contrasts = c("contr.sum", "contr.poly"))
fm <- lm(score ~ cond * sex, fa)
a2 <- Anova(fm, type = 2)
put("fx.t2.cond.ss", a2["cond", "Sum Sq"]); put("fx.t2.sex.ss", a2["sex", "Sum Sq"]); put("fx.t2.sex.p", a2["sex", "Pr(>F)"])
em <- summary(emmeans(fm, ~ cond))
put("fx.emm2", em$emmean[2]); put("fx.emm2.se", em$SE[2]); put("fx.emm2.lo", em$lower.CL[2])
pt <- summary(pairs(emmeans(fm, ~ cond), adjust = "tukey"), infer = TRUE)
put("fx.tukey.12.p", pt$p.value[1]); put("fx.tukey.23.p", pt$p.value[3]); put("fx.tukey.12.lo", pt$lower.CL[1])
pb <- summary(pairs(emmeans(fm, ~ cond), adjust = "bonferroni"), infer = TRUE)
put("fx.bonf.13.p", pb$p.value[2]); put("fx.bonf.13.hi", pb$upper.CL[2])
ph <- summary(pairs(emmeans(fm, ~ cond), adjust = "holm"))
put("fx.holm.12.p", ph$p.value[1])
jt <- as.data.frame(joint_tests(fm, by = "sex"))
put("fx.simple.sex1.F", jt$F.ratio[jt$sex == "1"]); put("fx.simple.sex1.p", jt$p.value[jt$sex == "1"])
jt2 <- as.data.frame(joint_tests(fm, by = "cond"))
put("fx.simple.cond3.F", jt2$F.ratio[jt2$cond == "3"])
fm2 <- lm(score ~ cond + sex, fa); am <- Anova(fm2, type = 2)
put("fx.main.cond.ss", am["cond", "Sum Sq"]); put("fx.main.dfe", fm2$df.residual)
an <- d[complete.cases(d[, c("score", "cond", "sex", "pre")]), ]; an$cond <- factor(an$cond); an$sex <- factor(an$sex)
ac <- lm(score ~ pre + cond * sex, an); a3 <- Anova(ac, type = 3)
put("fx.anc.n", nrow(an)); put("fx.anc.pre.F", a3["pre", "F value"]); put("fx.anc.cond.F", a3["cond", "F value"]); put("fx.anc.cond.p", a3["cond", "Pr(>F)"])
put("fx.anc.emm1", summary(emmeans(ac, ~ cond))$emmean[1])
sl <- anova(ac, lm(score ~ pre + cond * sex + pre:cond + pre:sex, an)); put("fx.anc.slopes.F", sl$F[2]); put("fx.anc.slopes.p", sl$`Pr(>F)`[2])
options(opt)

# One-way ANOVA follow-ups: planned contrasts (pooled and Welch), trend contrasts, Bonferroni and Holm pairs.
ow <- d[complete.cases(d[, c("score", "cond")]), ]; ow$cond <- factor(ow$cond)
ofit <- lm(score ~ cond, ow)
oe <- emmeans(ofit, ~ cond)
oc <- summary(contrast(oe, list(c1 = c(1, -0.5, -0.5), c2 = c(0, 1, -1)), adjust = "holm"), infer = TRUE)
put("ow.c1.est", oc$estimate[1]); put("ow.c1.se", oc$SE[1]); put("ow.c1.t", oc$t.ratio[1]); put("ow.c1.df", oc$df[1])
put("ow.c1.p_holm", oc$p.value[1]); put("ow.c2.p_holm", oc$p.value[2])
ocn <- summary(contrast(oe, list(c1 = c(1, -0.5, -0.5), c2 = c(0, 1, -1)), adjust = "none"), infer = TRUE)
put("ow.c1.p", ocn$p.value[1]); put("ow.c1.lo", ocn$lower.CL[1])
ocb <- summary(contrast(oe, list(c1 = c(1, -0.5, -0.5), c2 = c(0, 1, -1)), adjust = "bonferroni"), infer = TRUE)
put("ow.c2.hi_bonf", ocb$upper.CL[2]); put("ow.c2.p_bonf", ocb$p.value[2])
wc <- function(cc) {
  mm <- tapply(ow$score, ow$cond, mean); vv2 <- tapply(ow$score, ow$cond, var); nn <- tapply(ow$score, ow$cond, length)
  est <- sum(cc * mm); q <- cc^2 * vv2 / nn; se <- sqrt(sum(q)); dfw <- sum(q)^2 / sum(q^2 / (nn - 1))
  c(est = est, se = se, t = est / se, df = dfw, p = 2 * pt(-abs(est / se), dfw))
}
w1 <- wc(c(1, -0.5, -0.5)); put("ow.c1.welch.se", w1["se"]); put("ow.c1.welch.df", w1["df"]); put("ow.c1.welch.p", w1["p"])
pf2 <- lm(score ~ cond, ow, contrasts = list(cond = "contr.poly")); cs <- summary(pf2)$coefficients
put("ow.lin.t", cs["cond.L", "t value"]); put("ow.quad.p", cs["cond.Q", "Pr(>|t|)"]); put("ow.lin.est", cs["cond.L", "Estimate"])
pb2 <- pairwise.t.test(ow$score, ow$cond, p.adjust.method = "bonferroni"); put("ow.bonf.31.p", pb2$p.value["3", "1"])
ph2 <- pairwise.t.test(ow$score, ow$cond, p.adjust.method = "holm"); put("ow.holm.21.p", ph2$p.value["2", "1"])

# One-sided tests. The exact paths use small vectors with no ties (repeated in more_reference.rs).
os1 <- t.test(d$score, mu = 70, alternative = "greater"); os2 <- t.test(d$score, mu = 70, alternative = "less")
put("os.t1.p", os1$p.value); put("os.t1.lo", os1$conf.int[1] - 70); put("os.t2.p", os2$p.value); put("os.t2.hi", os2$conf.int[2] - 70)
pp2 <- d[complete.cases(d[, c("pre", "post")]), ]
op <- t.test(pp2$pre, pp2$post, paired = TRUE, alternative = "less"); put("os.paired.p", op$p.value); put("os.paired.hi", op$conf.int[2])
ig <- d[complete.cases(d[, c("score", "sex")]), ]
oi <- t.test(ig$score[ig$sex == 1], ig$score[ig$sex == 2], alternative = "greater"); put("os.ind.p", oi$p.value); put("os.ind.lo", oi$conf.int[1])
om <- suppressWarnings(wilcox.test(ig$score[ig$sex == 1], ig$score[ig$sex == 2], alternative = "less", exact = FALSE)); put("os.mw.p", om$p.value)
ow2 <- suppressWarnings(wilcox.test(pp2$pre, pp2$post, paired = TRUE, alternative = "greater", exact = FALSE)); put("os.wx.p", ow2$p.value)
sh <- d[complete.cases(d[, c("score", "hours")]), ]
oc1 <- cor.test(sh$score, sh$hours, alternative = "greater"); put("os.pear.p", oc1$p.value); put("os.pear.lo", oc1$conf.int[1])
oc2 <- cor.test(sh$score, sh$hours, alternative = "less"); put("os.pear.less.p", oc2$p.value); put("os.pear.less.hi", oc2$conf.int[2])
put("os.spear.t.p", suppressWarnings(cor.test(sh$score, sh$hours, method = "spearman", alternative = "less"))$p.value)
put("os.kend.z.p", cor.test(sh$score, sh$hours, method = "kendall", alternative = "greater")$p.value)
x8 <- c(1.2, 3.4, 2.2, 5.1, 4.0, 6.3, 0.7, 2.9); y8 <- c(2.0, 3.1, 1.5, 4.8, 5.5, 5.9, 1.1, 2.4)
x12 <- c(x8, 7.7, 0.2, 3.9, 8.8); y12 <- c(y8, 4.1, 6.6, 0.4, 7.2)
dd <- c(0.5, -1.3, 2.1, 0.8, -0.2, 1.7, 2.6, -0.9)
for (alt in c("greater", "less")) {
  put(paste0("xs.spear8.", alt), cor.test(x8, y8, method = "spearman", alternative = alt)$p.value)
  put(paste0("xs.spear12.", alt), cor.test(x12, y12, method = "spearman", alternative = alt)$p.value)
  put(paste0("xs.kend8.", alt), cor.test(x8, y8, method = "kendall", alternative = alt)$p.value)
  put(paste0("xs.mw.", alt), wilcox.test(x8, y8, alternative = alt)$p.value)
  put(paste0("xs.wx.", alt), wilcox.test(x8, x8 - dd, paired = TRUE, alternative = alt)$p.value)
}
sh3 <- d[complete.cases(d[, c("score", "hours", "pre")]), ]
rawp <- c(cor.test(sh3$score, sh3$hours)$p.value, cor.test(sh3$score, sh3$pre)$p.value, cor.test(sh3$hours, sh3$pre)$p.value)
hp3 <- p.adjust(rawp, "holm"); put("adj.holm.12", hp3[1]); put("adj.holm.13", hp3[2]); put("adj.holm.23", hp3[3])

# Curve estimation: score on hours, each curve as lm() of the transformed variables.
cv <- d[complete.cases(d[, c("score", "hours")]), ]; hx <- cv$hours; sy <- cv$score
cfits <- list(linear = lm(sy ~ hx), logarithmic = lm(sy ~ log(hx)), inverse = lm(sy ~ I(1 / hx)),
  quadratic = lm(sy ~ hx + I(hx^2)), cubic = lm(sy ~ hx + I(hx^2) + I(hx^3)),
  power = lm(log(sy) ~ log(hx)), exponential = lm(log(sy) ~ hx), s_curve = lm(log(sy) ~ I(1 / hx)))
for (nm in names(cfits)) {
  sm <- summary(cfits[[nm]]); fs <- sm$fstatistic
  put(paste0("cv.", nm, ".r2"), sm$r.squared); put(paste0("cv.", nm, ".F"), fs[1])
  put(paste0("cv.", nm, ".p"), pf(fs[1], fs[2], fs[3], lower.tail = FALSE))
  cf <- coef(cfits[[nm]]); put(paste0("cv.", nm, ".b0"), cf[1]); put(paste0("cv.", nm, ".b1"), cf[2])
  put(paste0("cv.", nm, ".se1"), sm$coefficients[2, 2])
}
put("cv.cubic.b3", coef(cfits$cubic)[4])
pw <- exp(fitted(cfits$power)); put("cv.power.r2o", 1 - sum((sy - pw)^2) / sum((sy - mean(sy))^2))
put("cv.n", nrow(cv))

# Bootstrap: the estimates the intervals are built around (resamples can't match R's random numbers).
sc <- na.omit(d$score)
put("bt.mean", mean(sc)); put("bt.median", median(sc)); put("bt.trim", mean(sc, trim = 0.05)); put("bt.sd", sd(sc)); put("bt.se.mean", sd(sc) / sqrt(length(sc)))
put("bt.r", cor(sh$score, sh$hours)); put("bt.rho", cor(sh$score, sh$hours, method = "spearman"))
br <- d[complete.cases(d[, c("score", "hours", "pre")]), ]; bm <- summary(lm(score ~ hours + pre, br))$coefficients
put("bt.b1", bm["hours", 1]); put("bt.b1.se", bm["hours", 2]); put("bt.b0", bm[1, 1])

# Data changes: a computed variable, a scale score (i2 reversed on 1-5, at least four items answered), a recode.
cp <- ifelse(d$pre > 50, (d$pre + d$post) / 2, log(d$post))
put("dc.compute.sum", sum(cp, na.rm = TRUE)); put("dc.compute.na", sum(is.na(cp)))
it <- s[, c("i1", "i2", "i3", "i4", "i5")]; it$i2 <- 6 - it$i2
ss <- ifelse(rowSums(!is.na(it)) >= 4, rowMeans(it, na.rm = TRUE), NA)
put("dc.scale.sum", sum(ss, na.rm = TRUE)); put("dc.scale.na", sum(is.na(ss))); put("dc.scale.1", ss[1])
ps <- ifelse(rowSums(!is.na(it)) >= 4, rowMeans(it, na.rm = TRUE) * 5, NA); put("dc.prorated.sum", sum(ps, na.rm = TRUE))
rc <- ifelse(is.na(d$cond), 0, ifelse(d$cond == 1, 1, ifelse(d$cond <= 3, 2, NA)))
put("dc.recode.1", sum(rc == 1, na.rm = TRUE)); put("dc.recode.2", sum(rc == 2, na.rm = TRUE)); put("dc.recode.0", sum(rc == 0, na.rm = TRUE)); put("dc.recode.na", sum(is.na(rc)))

# Select cases, weight cases and split file.
fs <- d[with(d, sex == 1 & hours > 5) %in% TRUE, ]; put("sub.filter.n", sum(!is.na(fs$score))); put("sub.filter.mean", mean(fs$score, na.rm = TRUE))
wt <- d[rep(seq_len(nrow(d)), ifelse(is.na(d$cond), 0, d$cond)), ]
wtt <- t.test(score ~ sex, wt); put("sub.weight.t", wtt$statistic); put("sub.weight.df", wtt$parameter); put("sub.weight.n", sum(!is.na(wt$score) & !is.na(wt$sex)))
for (g in 1:2) { sp <- d[d$sex %in% g & !is.na(d$cond) & !is.na(d$score), ]; put(paste0("sub.split.F.", g), summary(aov(score ~ factor(cond), sp))[[1]][1, "F value"]) }

# Repeated-measures and mixed ANOVA: pre, post and score as three measures; sex between (car, afex).
suppressMessages(library(afex))
rmd <- d[complete.cases(d[, c("pre", "post", "score", "sex")]), ]; rmd$sex <- factor(rmd$sex)
idata <- data.frame(time = factor(1:3))
ra <- summary(car::Anova(lm(cbind(pre, post, score) ~ 1, rmd), idata = idata, idesign = ~time, type = 3), multivariate = FALSE)
put("rm.F", ra$univariate.tests["time", "F value"]); put("rm.p", ra$univariate.tests["time", "Pr(>F)"]); put("rm.ss", ra$univariate.tests["time", "Sum Sq"])
put("rm.mauchly.W", ra$sphericity.tests["time", "Test statistic"]); put("rm.mauchly.p", ra$sphericity.tests["time", "p-value"])
put("rm.gg", ra$pval.adjustments["time", "GG eps"]); put("rm.gg.p", ra$pval.adjustments["time", "Pr(>F[GG])"]); put("rm.hf", ra$pval.adjustments["time", "HF eps"]); put("rm.hf.p", ra$pval.adjustments["time", "Pr(>F[HF])"])
rmd$id <- seq_len(nrow(rmd)); long <- reshape(rmd[, c("id", "sex", "pre", "post", "score")], direction = "long", varying = c("pre", "post", "score"), v.names = "y", timevar = "time", idvar = "id")
long$time <- factor(long$time)
a1 <- suppressMessages(aov_ez("id", "y", long, within = "time", anova_table = list(es = c("ges", "pes"), correction = "none")))$anova_table
put("rm.ges", a1["time", "ges"]); put("rm.pes", a1["time", "pes"])
ma <- summary(car::Anova(lm(cbind(pre, post, score) ~ sex, rmd, contrasts = list(sex = "contr.sum")), idata = idata, idesign = ~time, type = 3), multivariate = FALSE)
put("mx.sex.F", ma$univariate.tests["sex", "F value"]); put("mx.sex.p", ma$univariate.tests["sex", "Pr(>F)"])
put("mx.time.F", ma$univariate.tests["time", "F value"]); put("mx.int.F", ma$univariate.tests["sex:time", "F value"]); put("mx.int.p", ma$univariate.tests["sex:time", "Pr(>F)"])
put("mx.gg", ma$pval.adjustments["time", "GG eps"]); put("mx.hf", ma$pval.adjustments["time", "HF eps"]); put("mx.mauchly.W", ma$sphericity.tests["time", "Test statistic"]); put("mx.mauchly.p", ma$sphericity.tests["time", "p-value"])
a2 <- suppressMessages(aov_ez("id", "y", long, within = "time", between = "sex", anova_table = list(es = "ges", correction = "none")))$anova_table
put("mx.int.ges", a2["sex:time", "ges"]); put("mx.sex.ges", a2["sex", "ges"])
ph3 <- pairwise.t.test(long$y, long$time, paired = TRUE, p.adjust.method = "holm"); put("rm.holm.31", ph3$p.value["3", "1"])

# One-way MANOVA of pre, post and score by cond, with Box's M written out.
mv <- d[complete.cases(d[, c("pre", "post", "score", "cond")]), ]; mv$cond <- factor(mv$cond)
mfit <- manova(cbind(pre, post, score) ~ cond, mv)
for (tt in c("Pillai", "Wilks", "Hotelling-Lawley", "Roy")) { st <- summary(mfit, test = tt)$stats; key <- tolower(substr(tt, 1, 5)); put(paste0("mv.", key, ".stat"), st[1, 2]); put(paste0("mv.", key, ".F"), st[1, 3]); put(paste0("mv.", key, ".df2"), st[1, 5]); put(paste0("mv.", key, ".p"), st[1, 6]) }
Y <- as.matrix(mv[, c("pre", "post", "score")]); grs <- split(as.data.frame(Y), mv$cond); pp <- 3; gg <- length(grs); NN <- nrow(Y)
Sp <- Reduce(`+`, lapply(grs, function(z) cov(z) * (nrow(z) - 1))) / (NN - gg)
Mb <- (NN - gg) * log(det(Sp)) - sum(sapply(grs, function(z) (nrow(z) - 1) * log(det(cov(z)))))
cc <- (sum(1 / (sapply(grs, nrow) - 1)) - 1 / (NN - gg)) * (2 * pp^2 + 3 * pp - 1) / (6 * (pp + 1) * (gg - 1))
put("mv.boxm.chi2", Mb * (1 - cc)); put("mv.boxm.p", pchisq(Mb * (1 - cc), pp * (pp + 1) * (gg - 1) / 2, lower.tail = FALSE))
put("mv.uni.score.F", summary(aov(score ~ cond, mv))[[1]][1, "F value"])

# Factor analysis of the five scale items (psych, factanal).
suppressMessages(library(psych))
fi <- na.omit(s[, c("i1", "i2", "i3", "i4", "i5")]); fr <- cor(fi); fn <- nrow(fi)
put("fa.n", fn); put("fa.kmo", KMO(fr)$MSA); put("fa.msa1", KMO(fr)$MSAi[1])
cb <- cortest.bartlett(fr, n = fn); put("fa.bart.chi2", cb$chisq); put("fa.bart.p", cb$p.value)
fml <- factanal(covmat = fr, factors = 2, n.obs = fn, rotation = "none", control = list(opt = list(maxit = 10000, factr = 1)))
put("fa.ml.u1", fml$uniquenesses[1]); put("fa.ml.u5", fml$uniquenesses[5]); put("fa.ml.chi2", fml$STATISTIC)
fpa <- fa(fr, 1, n.obs = fn, fm = "pa", rotate = "none", min.err = 1e-13, max.iter = 100000); put("fa.pa.h1", fpa$communality[1]); put("fa.pa.h4", fpa$communality[4])
fmr <- fa(fr, 1, n.obs = fn, fm = "minres", rotate = "none"); put("fa.mr.h1", fmr$communality[1])
ss <- function(L) sort(colSums(unclass(L)^2), decreasing = TRUE)
fv <- varimax(unclass(fml$loadings), eps = 1e-14); put("fa.vm.ss1", ss(fv$loadings)[1]); put("fa.vm.ss2", ss(fv$loadings)[2])
fo <- GPArotation::oblimin(unclass(fml$loadings), eps = 1e-14, maxit = 100000); put("fa.ob.phi", abs(fo$Phi[1, 2])); put("fa.ob.ss1", ss(fo$loadings)[1])
# psych's Promax steps, from a varimax converged tightly (psych's own call uses varimax's loose default tolerance).
pmx <- function(x, m = 4) { xx <- varimax(x, eps = 1e-14); x <- xx$loadings; Q <- x * abs(x)^(m - 1); U <- lm.fit(x, Q)$coefficients; d <- diag(solve(t(U) %*% U)); U <- U %*% diag(sqrt(d)); z <- x %*% U; U <- xx$rotmat %*% U; ui <- solve(U); list(loadings = z, Phi = ui %*% t(ui)) }
fp <- pmx(unclass(fml$loadings)); put("fa.pm.phi", abs(fp$Phi[1, 2])); put("fa.pm.ss1", ss(fp$loadings)[1])
pc <- principal(fr, 2, rotate = "none"); put("fa.pc.ev1", pc$values[1]); put("fa.pc.h3", pc$communality[3])

# McDonald's omega from a one-factor CFA (lavaan, std.lv).
cf <- lavaan::cfa("F =~ i1 + i2 + i3 + i4 + i5", fi, std.lv = TRUE); pe <- lavaan::parameterEstimates(cf)
lam <- pe$est[pe$op == "=~"]; th <- pe$est[pe$op == "~~" & pe$lhs == pe$rhs & pe$lhs != "F"]; put("rel.omega", sum(lam)^2 / (sum(lam)^2 + sum(th)))

# Mediation (hours -> pre, post -> score) and moderation (hours x pre -> score).
md <- d[complete.cases(d[, c("hours", "pre", "post", "score")]), ]
ma1 <- summary(lm(pre ~ hours, md))$coefficients; ma2 <- summary(lm(post ~ hours, md))$coefficients
mb <- summary(lm(score ~ hours + pre + post, md))$coefficients; mc <- summary(lm(score ~ hours, md))$coefficients
put("me.n", nrow(md)); put("me.a1", ma1["hours", 1]); put("me.a1.se", ma1["hours", 2]); put("me.b2", mb["post", 1]); put("me.b2.se", mb["post", 2])
put("me.direct", mb["hours", 1]); put("me.direct.p", mb["hours", 4]); put("me.total", mc["hours", 1]); put("me.ind1", ma1["hours", 1] * mb["pre", 1])
mo <- d[complete.cases(d[, c("hours", "pre", "score")]), ]; mf <- lm(score ~ hours * pre, mo); cf <- coef(mf); vc <- vcov(mf)
put("mo.xw", cf["hours:pre"]); put("mo.xw.p", summary(mf)$coefficients["hours:pre", 4])
w16 <- quantile(mo$pre, 0.16); sl <- cf["hours"] + cf["hours:pre"] * w16; se <- sqrt(vc["hours", "hours"] + 2 * w16 * vc["hours", "hours:pre"] + w16^2 * vc["hours:pre", "hours:pre"])
put("mo.w16", w16); put("mo.slope16", sl); put("mo.slope16.se", se)
jn <- interactions::johnson_neyman(mf, pred = hours, modx = pre, plot = FALSE)$bounds; put("mo.jn.lo", min(jn)); put("mo.jn.hi", max(jn))
mc2 <- lm(score ~ I(hours - mean(hours)) * I(pre - mean(pre)), mo); put("mo.c.x", coef(mc2)[2])

# Ordinal, multinomial and count regression.
og <- d[complete.cases(d[, c("cond", "hours", "sex")]), ]; og$cf <- factor(og$cond, ordered = TRUE); og$sf <- factor(og$sex)
po <- MASS::polr(cf ~ hours + sf, og, Hess = TRUE, control = list(reltol = 1e-14, maxit = 10000)); pcf <- coef(summary(po))
put("or.n", nrow(og)); put("or.hours", pcf["hours", 1]); put("or.hours.se", pcf["hours", 2]); put("or.sex", pcf["sf2", 1]); put("or.theta2", po$zeta[2]); put("or.ll", as.numeric(logLik(po)))
mn <- nnet::multinom(factor(cond) ~ hours + sf, og, trace = FALSE, reltol = 1e-14, maxit = 10000, Hess = TRUE); mcf <- coef(mn); mse <- sqrt(diag(solve(mn$Hessian)))
put("mn.c2.hours", mcf["2", "hours"]); put("mn.c3.sex", mcf["3", "sf2"]); put("mn.ll", as.numeric(logLik(mn))); put("mn.c2.hours.se", mse["2:hours"])
mn0 <- nnet::multinom(factor(cond) ~ sf, og, trace = FALSE, reltol = 1e-14, maxit = 10000); put("mn.lr.hours", 2 * (as.numeric(logLik(mn)) - as.numeric(logLik(mn0))))
cd <- d[complete.cases(d[, c("hours", "pre", "sex")]), ]; cd$cnt <- round(cd$hours); cd$sf <- factor(cd$sex)
gp <- glm(cnt ~ pre + sf, poisson, cd, control = glm.control(epsilon = 1e-14, maxit = 100)); gcf <- coef(summary(gp))
put("ct.n", nrow(cd)); put("ct.p.pre", gcf["pre", 1]); put("ct.p.pre.se", gcf["pre", 2]); put("ct.p.dev", deviance(gp)); put("ct.p.aic", AIC(gp))
gn <- MASS::glm.nb(cnt ~ pre + sf, cd, control = glm.control(epsilon = 1e-14, maxit = 100)); ncf <- coef(summary(gn))
put("ct.nb.pre", ncf["pre", 1]); put("ct.nb.pre.se", ncf["pre", 2]); put("ct.nb.theta", gn$theta); put("ct.nb.ll", as.numeric(logLik(gn)))

# Robust standard errors (sandwich) and trimmed-means tests (WRS2).
hm <- lm(score ~ hours + pre, br); hv <- sandwich::vcovHC(hm, type = "HC3"); ct <- lmtest::coeftest(hm, vcov. = hv)
put("hc3.hours.se", ct["hours", 2]); put("hc3.hours.p", ct["hours", 4]); put("hc1.hours.se", sqrt(sandwich::vcovHC(hm, type = "HC1")["hours", "hours"]))
wt2 <- lmtest::waldtest(hm, vcov = hv, test = "F"); put("hc3.F", wt2$F[2])
yu <- WRS2::yuen(score ~ factor(sex), d[complete.cases(d[, c("score", "sex")]), ]); put("yu.t", yu$test); put("yu.df", yu$df); put("yu.p", yu$p.value); put("yu.diff", yu$diff); put("yu.lo", yu$conf.int[1])
yd <- WRS2::yuend(pp2$pre, pp2$post); put("yd.t", yd$test); put("yd.df", yd$df); put("yd.p", yd$p.value); put("yd.diff", yd$diff)
tw <- d[complete.cases(d[, c("score", "cond")]), ]; t1 <- WRS2::t1way(score ~ factor(cond), tw); put("t1.F", t1$test); put("t1.df2", t1$df2); put("t1.p", t1$p.value)

# Linear mixed models on lme4's sleepstudy (written to sleepstudy.csv for the Rust test), with lmerTest's df.
suppressMessages(library(lmerTest))
ss <- lme4::sleepstudy; write.csv(ss, "sleepstudy.csv", row.names = FALSE)
ctl <- lme4::lmerControl(optCtrl = list(xtol_abs = 1e-12, ftol_abs = 1e-14))
m1 <- lmer(Reaction ~ Days + (1 | Subject), ss, control = ctl); s1 <- summary(m1)$coefficients; vc1 <- as.data.frame(VarCorr(m1))
put("lmm1.days", s1["Days", 1]); put("lmm1.days.se", s1["Days", 2]); put("lmm1.days.df", s1["Days", 3]); put("lmm1.int.df", s1[1, 3])
put("lmm1.tau", vc1$vcov[1]); put("lmm1.sigma2", vc1$vcov[2]); put("lmm1.reml", REMLcrit(m1))
m2 <- lmer(Reaction ~ Days + (Days | Subject), ss, control = ctl); s2 <- summary(m2)$coefficients; vc2 <- as.data.frame(VarCorr(m2))
put("lmm2.days", s2["Days", 1]); put("lmm2.days.se", s2["Days", 2]); put("lmm2.days.df", s2["Days", 3]); put("lmm2.reml", REMLcrit(m2))
put("lmm2.tau00", vc2$vcov[1]); put("lmm2.tau11", vc2$vcov[2]); put("lmm2.cor", vc2$sdcor[3]); put("lmm2.sigma2", vc2$vcov[4])
m3 <- lmer(Reaction ~ Days + (1 | Subject), ss, REML = FALSE, control = ctl); put("lmm3.dev", deviance(m3)); put("lmm3.tau", as.data.frame(VarCorr(m3))$vcov[1])

# Little's MCAR test (EM to convergence with norm) and Rubin's rules (mice::pool.scalar).
lx <- as.matrix(d[, c("pre", "post", "score", "hours")]); lx <- lx[rowSums(!is.na(lx)) > 0, ]
ns <- norm::prelim.norm(lx); th <- norm::em.norm(ns, showits = FALSE, criterion = 1e-14, maxits = 100000); par <- norm::getparam.norm(ns, th)
pat <- apply(!is.na(lx), 1, paste, collapse = ""); d2 <- 0; dfl <- 0
for (pp in unique(pat)) { rows <- lx[pat == pp, , drop = FALSE]; o <- !is.na(rows[1, ]); mj <- colMeans(rows[, o, drop = FALSE]); dv <- mj - par$mu[o]; d2 <- d2 + nrow(rows) * drop(t(dv) %*% solve(par$sigma[o, o]) %*% dv); dfl <- dfl + sum(o) }
put("little.chi2", d2); put("little.df", dfl - ncol(lx)); put("little.naniar", naniar::mcar_test(as.data.frame(lx))$statistic)
pq <- mice::pool.scalar(c(1.2, 1.5, 1.1, 1.4, 1.3), c(0.04, 0.05, 0.045, 0.05, 0.042), n = 100, k = 3)
put("pool.qbar", pq$qbar); put("pool.t", pq$t); put("pool.df", pq$df); put("pool.fmi", pq$fmi); put("pool.r", pq$r)

# Bayes factors (BayesFactor, default priors).
suppressMessages(library(BayesFactor))
bfv <- function(x) extractBF(x)$bf
put("bf.one", bfv(ttestBF(na.omit(d$score), mu = 60))); put("bf.paired", bfv(ttestBF(pp2$pre, pp2$post, paired = TRUE)))
put("bf.ind", bfv(ttestBF(ig$score[ig$sex == 1], ig$score[ig$sex == 2]))); put("bf.cor", bfv(correlationBF(sh$score, sh$hours)))
ctab <- table(d$sex, d$smoker); put("bf.ct.joint", bfv(contingencyTableBF(ctab, sampleType = "jointMulti"))); put("bf.ct.rows", bfv(contingencyTableBF(ctab, sampleType = "indepMulti", fixedMargin = "rows")))
put("bf.ct.cols", bfv(contingencyTableBF(ctab, sampleType = "indepMulti", fixedMargin = "cols"))); put("bf.ct.pois", bfv(contingencyTableBF(ctab, sampleType = "poisson")))
twf <- tw; twf$cond <- factor(twf$cond); ab <- anovaBF(score ~ cond, twf); put("bf.anova", bfv(ab)); put("bf.anova.err", extractBF(ab)$error)
put("bf.reg", bfv(lmBF(score ~ hours + pre, br)))

# Exact permutation tests on the small untied vectors.
pg <- data.frame(v = c(x8, y8), g = factor(rep(1:2, each = 8)))
put("perm.two", coin::pvalue(coin::oneway_test(v ~ g, pg, distribution = "exact")))
pdd <- dd; flips <- as.matrix(expand.grid(rep(list(c(1, -1)), 8))); sums <- abs(flips %*% pdd); put("perm.paired", mean(sums >= abs(sum(pdd)) - 1e-12))
perms <- function(v) { if (length(v) <= 1) return(list(v)); out <- list(); for (i in seq_along(v)) for (p in perms(v[-i])) out[[length(out) + 1]] <- c(v[i], p); out }
pr0 <- abs(cor(x8, y8)); rs <- sapply(perms(y8), function(yy) abs(cor(x8, yy))); put("perm.cor", mean(rs >= pr0 - 1e-12))

# Survival (survival::lung, written to lung.csv) and ROC (pROC).
lung <- survival::lung; write.csv(lung[, c("time", "status", "age", "sex", "ph.ecog")], "lung.csv", row.names = FALSE, na = "")
library(survival)
kf <- survfit(Surv(time, status == 2) ~ sex, lung); ks <- summary(kf)$table
put("km.med1", ks[1, "median"]); put("km.med1.lo", ks[1, "0.95LCL"]); put("km.med1.hi", ks[1, "0.95UCL"]); put("km.med2", ks[2, "median"])
k1 <- survfit(Surv(time, status == 2) ~ 1, lung); put("km.all.med", summary(k1)$table["median"]); put("km.all.s100", summary(k1, times = 100)$surv); put("km.all.se100", summary(k1, times = 100)$std.err)
sd2 <- survdiff(Surv(time, status == 2) ~ sex, lung); put("lr.chi2", sd2$chisq)
cx <- coxph(Surv(time, status == 2) ~ age + factor(sex), lung); cs2 <- summary(cx)
put("cox.age", coef(cx)[1]); put("cox.age.se", cs2$coefficients[1, 3]); put("cox.sex.hr", exp(coef(cx)[2])); put("cox.lr", cs2$logtest[1]); put("cox.wald", drop(t(coef(cx)) %*% solve(vcov(cx)) %*% coef(cx))); put("cox.score", cs2$sctest[1])
ro <- d[complete.cases(d[, c("smoker", "score")]), ]; rc2 <- pROC::roc(ro$smoker, ro$score, levels = c(1, 2), direction = "<", quiet = TRUE)
put("roc.auc2", as.numeric(rc2$auc)); put("roc.lo2", pROC::ci.auc(rc2, method = "delong")[1]); put("roc.best", pROC::coords(rc2, "best", best.method = "youden", ret = "threshold")[1, 1])

# Time series: LakeHuron and Nile (written to lakehuron.csv and nile.csv).
write.csv(data.frame(level = as.numeric(LakeHuron)), "lakehuron.csv", row.names = FALSE); write.csv(data.frame(flow = as.numeric(Nile)), "nile.csv", row.names = FALSE)
lh <- as.numeric(LakeHuron); ac <- acf(lh, lag.max = 10, plot = FALSE)$acf; pc <- pacf(lh, lag.max = 10, plot = FALSE)$acf
put("ts.acf1", ac[2]); put("ts.acf5", ac[6]); put("ts.pacf2", pc[2]); put("ts.lb", Box.test(lh, lag = 10, type = "Ljung-Box")$statistic)
hw <- HoltWinters(as.numeric(Nile), beta = FALSE, gamma = FALSE, optim.control = list()); put("hw.alpha", hw$alpha); put("hw.sse", hw$SSE); put("hw.level", hw$coefficients[1])
ar1 <- arima(lh, order = c(1, 0, 1), method = "ML", optim.control = list(reltol = 1e-14, maxit = 5000))
put("ar.ar1", coef(ar1)[1]); put("ar.ma1", coef(ar1)[2]); put("ar.mean", coef(ar1)[3]); put("ar.ar1.se", sqrt(diag(ar1$var.coef))[1]); put("ar.sigma2", ar1$sigma2); put("ar.loglik", ar1$loglik); put("ar.aic", ar1$aic)
ar2 <- arima(as.numeric(Nile), order = c(0, 1, 1), method = "ML", optim.control = list(reltol = 1e-14, maxit = 5000)); fc <- predict(ar2, n.ahead = 3)
put("ar2.ma1", coef(ar2)[1]); put("ar2.sigma2", ar2$sigma2); put("ar2.f1", fc$pred[1]); put("ar2.f3.se", fc$se[3])

# Survey design (survey's apistrat and apiclus1, written to CSV).
suppressMessages(library(survey)); data(api)
write.csv(apistrat[, c("stype", "pw", "api00", "enroll", "ell", "meals", "sch.wide")], "apistrat.csv", row.names = FALSE)
write.csv(apiclus1[, c("dnum", "pw", "api00", "enroll", "ell", "meals")], "apiclus1.csv", row.names = FALSE)
dst <- svydesign(id = ~1, strata = ~stype, weights = ~pw, data = apistrat); dcl <- svydesign(id = ~dnum, weights = ~pw, data = apiclus1)
sm <- svymean(~api00 + enroll, dst); put("sv.mean.api", coef(sm)[1]); put("sv.mean.api.se", SE(sm)[1]); put("sv.mean.enroll.se", SE(sm)[2])
stt <- svytotal(~enroll, dst); put("sv.total", coef(stt)); put("sv.total.se", SE(stt))
sp <- svymean(~sch.wide, dst); put("sv.prop.yes", coef(sp)[2]); put("sv.prop.yes.se", SE(sp)[2])
cm <- svymean(~api00, dcl); put("sv.cl.mean", coef(cm)); put("sv.cl.se", SE(cm))
sg <- summary(svyglm(api00 ~ ell + meals, dcl))$coefficients; put("sv.glm.ell", sg["ell", 1]); put("sv.glm.ell.se", sg["ell", 2]); put("sv.glm.ell.p", sg["ell", 4])

lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, '",'), paste(lines, collapse = ",\n"), "}"), "more_reference.json")
