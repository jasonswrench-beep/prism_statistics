# Reference values for the preview analyses (prism-engine/tests/preview_reference.rs).
# Run from this folder: Rscript make_preview_reference.R
# Uses only base R. The coding is described in make_preview_data.R; user-missing codes are set to NA by hand here, so
# the test checks that PrismStatistics leaves out the same cases.
d <- read.csv("preview.csv")
d$cond[d$cond %in% 9] <- NA
d$pre[d$pre %in% -99] <- NA
d$post[d$post %in% -99] <- NA
v <- c()
put <- function(name, x) v[[name]] <<- unname(x)

# One-sample t test: score against 60.
s <- d$score[!is.na(d$score)]
t1 <- t.test(s, mu = 60)
put("one.n", length(s)); put("one.mean", mean(s)); put("one.sd", sd(s))
put("one.t", t1$statistic); put("one.df", t1$parameter); put("one.p", t1$p.value)
put("one.lo", t1$conf.int[1] - 60); put("one.hi", t1$conf.int[2] - 60)
put("one.d", (mean(s) - 60) / sd(s))

# Paired t test: pre minus post.
k <- d[!is.na(d$pre) & !is.na(d$post), ]
tp <- t.test(k$pre, k$post, paired = TRUE)
dz <- k$pre - k$post
put("pair.n", nrow(k)); put("pair.mean", mean(dz)); put("pair.sd", sd(dz))
put("pair.t", tp$statistic); put("pair.df", tp$parameter); put("pair.p", tp$p.value)
put("pair.lo", tp$conf.int[1]); put("pair.hi", tp$conf.int[2])
put("pair.dz", mean(dz) / sd(dz)); put("pair.r", cor(k$pre, k$post))

# One-way ANOVA: score by cond.
a <- d[!is.na(d$cond) & !is.na(d$score), ]
a$f <- factor(a$cond)
fit <- aov(score ~ f, data = a)
tab <- summary(fit)[[1]]
put("aov.n", nrow(a)); put("aov.F", tab$`F value`[1]); put("aov.df1", tab$Df[1]); put("aov.df2", tab$Df[2]); put("aov.p", tab$`Pr(>F)`[1])
put("aov.ssb", tab$`Sum Sq`[1]); put("aov.ssw", tab$`Sum Sq`[2])
ssb <- tab$`Sum Sq`[1]; ssw <- tab$`Sum Sq`[2]; msw <- tab$`Mean Sq`[2]
put("aov.eta2", ssb / (ssb + ssw)); put("aov.omega2", (ssb - tab$Df[1] * msw) / (ssb + ssw + msw))
w <- oneway.test(score ~ f, data = a, var.equal = FALSE)
put("aov.welchF", w$statistic); put("aov.welchdf2", w$parameter[2]); put("aov.welchp", w$p.value)
med <- ave(a$score, a$f, FUN = median)
lev <- summary(aov(abs(a$score - med) ~ a$f))[[1]]
put("aov.leveneF", lev$`F value`[1]); put("aov.levenep", lev$`Pr(>F)`[1])
tk <- TukeyHSD(fit, conf.level = 0.95)$f
# Rows of TukeyHSD are 2-1, 3-1, 3-2; PrismStatistics reports 1-2, 1-3, 2-3, so the signs flip.
put("tukey.12.diff", -tk[1, "diff"]); put("tukey.12.p", tk[1, "p adj"]); put("tukey.12.lo", -tk[1, "upr"]); put("tukey.12.hi", -tk[1, "lwr"])
put("tukey.13.p", tk[2, "p adj"]); put("tukey.23.p", tk[3, "p adj"])
gh <- function(i, j) {
  x <- a$score[a$cond == i]; y <- a$score[a$cond == j]
  vx <- var(x) / length(x); vy <- var(y) / length(y)
  se <- sqrt(vx + vy); df <- (vx + vy)^2 / (vx^2 / (length(x) - 1) + vy^2 / (length(y) - 1))
  q <- abs(mean(x) - mean(y)) / se * sqrt(2)
  ptukey(q, 3, df, lower.tail = FALSE)
}
put("gh.12.p", gh(1, 2)); put("gh.13.p", gh(1, 3)); put("gh.23.p", gh(2, 3))

# Crosstabs: sex by smoker (2 x 2) and region (text) by cond (3 x 3).
ct <- table(d$sex, d$smoker)
cs <- suppressWarnings(chisq.test(ct, correct = FALSE))
put("x22.chi2", cs$statistic); put("x22.p", cs$p.value); put("x22.n", sum(ct))
put("x22.phi", (ct[1, 1] * ct[2, 2] - ct[1, 2] * ct[2, 1]) / sqrt(prod(rowSums(ct)) * prod(colSums(ct))))
ct2 <- table(d$region, d$cond)
cs2 <- suppressWarnings(chisq.test(ct2, correct = FALSE))
put("x33.chi2", cs2$statistic); put("x33.df", cs2$parameter); put("x33.p", cs2$p.value); put("x33.n", sum(ct2))
put("x33.V", sqrt(cs2$statistic / (sum(ct2) * (min(dim(ct2)) - 1))))
e <- cs2$expected; o <- ct2
g2 <- 2 * sum(ifelse(o > 0, o * log(o / e), 0))
put("x33.g2", g2); put("x33.g2p", pchisq(g2, cs2$parameter, lower.tail = FALSE))
put("x33.minexp", min(e))
put("x33.East.1", ct2["East", "1"])

# Correlations: pre, hours, score, listwise.
c3 <- d[!is.na(d$pre) & !is.na(d$hours) & !is.na(d$score), ]
p1 <- cor.test(c3$hours, c3$score)
put("cor.n", nrow(c3)); put("cor.hs.r", p1$estimate); put("cor.hs.p", p1$p.value); put("cor.hs.lo", p1$conf.int[1]); put("cor.hs.hi", p1$conf.int[2])
put("cor.ph.r", cor(c3$pre, c3$hours)); put("cor.ph.p", cor.test(c3$pre, c3$hours)$p.value)
s1 <- suppressWarnings(cor.test(c3$hours, c3$score, method = "spearman"))
put("spear.hs.r", s1$estimate); put("spear.hs.p", s1$p.value)
# Pairwise: hours and score among cases with both.
c2 <- d[!is.na(d$hours) & !is.na(d$score), ]
put("cor.pairwise.n", nrow(c2)); put("cor.pairwise.r", cor(c2$hours, c2$score))

# Mann-Whitney: score by sex (1 = Female first). These data have ties. R 4.6 then computes a conditional exact p value
# for small samples by default; PrismStatistics (05 section 1) uses the normal approximation with the continuity and tie
# corrections whenever there are ties, so R is asked for that with exact = FALSE. The same holds for the signed-rank test.
m1 <- d$score[d$sex == 1 & !is.na(d$score)]; m2 <- d$score[d$sex == 2 & !is.na(d$score)]
mw <- wilcox.test(m1, m2, exact = FALSE)
put("mw.n1", length(m1)); put("mw.n2", length(m2)); put("mw.U", mw$statistic); put("mw.p", mw$p.value)
put("mw.r", 2 * mw$statistic / (length(m1) * length(m2)) - 1)
put("mw.mdn1", median(m1)); put("mw.mdn2", median(m2))

# Wilcoxon signed-rank: pre minus post.
wx <- wilcox.test(k$pre, k$post, paired = TRUE, exact = FALSE)
put("wx.V", wx$statistic); put("wx.p", wx$p.value)
dd <- k$pre - k$post
put("wx.neg", sum(dd < 0)); put("wx.pos", sum(dd > 0)); put("wx.ties", sum(dd == 0))
rk <- rank(abs(dd[dd != 0]))
put("wx.r", (sum(rk[dd[dd != 0] > 0]) - sum(rk[dd[dd != 0] < 0])) / sum(rk))

# Kruskal-Wallis: score by cond, with Dunn's comparisons (Holm).
kw <- kruskal.test(score ~ f, data = a)
put("kw.H", kw$statistic); put("kw.df", kw$parameter); put("kw.p", kw$p.value)
N <- nrow(a); r <- rank(a$score); tt <- table(a$score)
base <- N * (N + 1) / 12 - sum(tt^3 - tt) / (12 * (N - 1))
mr <- tapply(r, a$f, mean); nn <- tapply(r, a$f, length)
z <- c((mr[1] - mr[2]) / sqrt(base * (1 / nn[1] + 1 / nn[2])), (mr[1] - mr[3]) / sqrt(base * (1 / nn[1] + 1 / nn[3])), (mr[2] - mr[3]) / sqrt(base * (1 / nn[2] + 1 / nn[3])))
pz <- 2 * pnorm(-abs(z))
put("dunn.12.z", z[1]); put("dunn.12.padj", p.adjust(pz, "holm")[1]); put("dunn.23.padj", p.adjust(pz, "holm")[3])

# Linear regression: score from hours and cond (Lecture is the reference).
rg <- d[!is.na(d$score) & !is.na(d$hours) & !is.na(d$cond), ]
rg$f <- relevel(factor(rg$cond), ref = "1")
lm1 <- lm(score ~ hours + f, data = rg)
sm <- summary(lm1); ci <- confint(lm1)
put("reg.n", nrow(rg)); put("reg.r2", sm$r.squared); put("reg.adjr2", sm$adj.r.squared)
put("reg.F", sm$fstatistic[1]); put("reg.df1", sm$fstatistic[2]); put("reg.df2", sm$fstatistic[3])
put("reg.p", pf(sm$fstatistic[1], sm$fstatistic[2], sm$fstatistic[3], lower.tail = FALSE)); put("reg.sigma", sm$sigma)
cf <- sm$coefficients
put("reg.b0", cf[1, 1]); put("reg.hours.b", cf[2, 1]); put("reg.hours.se", cf[2, 2]); put("reg.hours.t", cf[2, 3]); put("reg.hours.p", cf[2, 4])
put("reg.hours.lo", ci[2, 1]); put("reg.hours.hi", ci[2, 2])
put("reg.f3.b", cf["f3", 1]); put("reg.f3.p", cf["f3", 4])
put("reg.hours.beta", cf[2, 1] * sd(rg$hours) / sd(rg$score))
X <- model.matrix(lm1)[, -1]
vif <- function(j) 1 / (1 - summary(lm(X[, j] ~ X[, -j]))$r.squared)
put("reg.hours.vif", vif(1))
e2 <- residuals(lm1)^2
bp <- nrow(rg) * summary(lm(e2 ~ X))$r.squared
put("reg.bp", bp); put("reg.bp.p", pchisq(bp, ncol(X), lower.tail = FALSE))

lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, '",'), paste(lines, collapse = ",\n"), "}"), "preview_reference.json")
