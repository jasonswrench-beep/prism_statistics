# Reference values for linear panel models (prism-engine/tests/panel_reference.rs), from plm.
# Run from this folder: Rscript make_panel_reference.R
# Writes grunfeld.csv, produc.csv and empluk.csv (logs precomputed) and panel_reference.json.
suppressMessages(library(plm))
data("Grunfeld", package = "plm"); data("Produc", package = "plm"); data("EmplUK", package = "plm")
write.csv(Grunfeld, "grunfeld.csv", row.names = FALSE)
Produc$lgsp <- log(Produc$gsp); Produc$lpcap <- log(Produc$pcap); Produc$lpc <- log(Produc$pc); Produc$lemp <- log(Produc$emp)
write.csv(Produc[, c("state", "year", "lgsp", "lpcap", "lpc", "lemp", "unemp")], "produc.csv", row.names = FALSE)
EmplUK$lemp <- log(EmplUK$emp); EmplUK$lwage <- log(EmplUK$wage); EmplUK$lcap <- log(EmplUK$capital)
write.csv(EmplUK[, c("firm", "year", "lemp", "lwage", "lcap")], "empluk.csv", row.names = FALSE)

v <- c()
put <- function(name, x) if (length(x) == 1 && !is.na(x)) v[[name]] <<- unname(x)
dump <- function(p, m, robust = FALSE) {
  vc <- if (robust) vcovHC(m, method = "arellano", type = "HC0", cluster = "group") else vcov(m)
  s <- summary(m, vcov = vc)
  cf <- s$coefficients
  for (j in seq_len(nrow(cf))) {
    k <- paste0(p, ".coef.", j)
    put(paste0(k, ".b"), cf[j, 1]); put(paste0(k, ".se"), cf[j, 2]); put(paste0(k, ".t"), cf[j, 3]); put(paste0(k, ".p"), cf[j, 4])
  }
  put(paste0(p, ".rss"), deviance(m)); put(paste0(p, ".df"), df.residual(m))
  put(paste0(p, ".r2"), s$r.squared[["rsq"]]); put(paste0(p, ".adj_r2"), s$r.squared[["adjrsq"]])
  fs <- s$fstatistic
  if (grepl("Chisq", names(fs$statistic))) { put(paste0(p, ".chisq"), fs$statistic); put(paste0(p, ".chisq.p"), fs$p.value) }
  else { put(paste0(p, ".f"), fs$statistic); put(paste0(p, ".f.p"), fs$p.value) }
  if (m$args$model == "random") {
    e <- ercomp(m)
    put(paste0(p, ".sigma2.idios"), e$sigma2[["idios"]]); if (m$args$effect != "time") put(paste0(p, ".sigma2.id"), e$sigma2[["id"]])
    if (m$args$effect == "twoways") {
      put(paste0(p, ".sigma2.time"), e$sigma2[["time"]]); put(paste0(p, ".theta.min"), e$theta$id); put(paste0(p, ".theta.time"), e$theta$time); put(paste0(p, ".theta.total"), e$theta$total)
    } else if (m$args$effect == "time") {
      put(paste0(p, ".sigma2.id"), e$sigma2[["time"]]); put(paste0(p, ".theta.min"), min(e$theta)); put(paste0(p, ".theta.max"), max(e$theta))
    } else { put(paste0(p, ".theta.min"), min(e$theta)); put(paste0(p, ".theta.max"), max(e$theta)) }
  }
}
tests <- function(p, f, data, index) {
  w <- plm(f, data, index = index, model = "within"); r <- plm(f, data, index = index, model = "random"); po <- plm(f, data, index = index, model = "pooling")
  ft <- pFtest(w, po); put(paste0(p, ".ftest.f"), ft$statistic); put(paste0(p, ".ftest.df1"), ft$parameter[1]); put(paste0(p, ".ftest.df2"), ft$parameter[2]); put(paste0(p, ".ftest.p"), ft$p.value)
  h <- phtest(w, r); put(paste0(p, ".hausman.chisq"), h$statistic); put(paste0(p, ".hausman.df"), h$parameter); put(paste0(p, ".hausman.p"), h$p.value)
  bp <- plmtest(po, type = "bp"); put(paste0(p, ".bp.chisq"), bp$statistic); put(paste0(p, ".bp.p"), bp$p.value)
}
gi <- c("firm", "year"); gf <- inv ~ value + capital
dump("gpool", plm(gf, Grunfeld, index = gi, model = "pooling"))
dump("gwithin", plm(gf, Grunfeld, index = gi, model = "within"))
dump("gtime", plm(gf, Grunfeld, index = gi, model = "within", effect = "time"))
dump("gtwo", plm(gf, Grunfeld, index = gi, model = "within", effect = "twoways"))
dump("grandom", plm(gf, Grunfeld, index = gi, model = "random"))
dump("gbetween", plm(gf, Grunfeld, index = gi, model = "between"))
dump("gfd", plm(gf, Grunfeld, index = gi, model = "fd"))
dump("gwithinr", plm(gf, Grunfeld, index = gi, model = "within"), robust = TRUE)
dump("grandomr", plm(gf, Grunfeld, index = gi, model = "random"), robust = TRUE)
dump("gpoolr", plm(gf, Grunfeld, index = gi, model = "pooling"), robust = TRUE)
tests("g", gf, Grunfeld, gi)
dump("grandom2", plm(gf, Grunfeld, index = gi, model = "random", effect = "twoways"))
dump("grandomt", plm(gf, Grunfeld, index = gi, model = "random", effect = "time"))
dump("grandom2r", plm(gf, Grunfeld, index = gi, model = "random", effect = "twoways"), robust = TRUE)
pf <- lgsp ~ lpcap + lpc + lemp + unemp
Produc$lgsp <- Produc$lgsp
dump("pwithin", plm(pf, Produc, index = c("state", "year"), model = "within"))
dump("prandom", plm(pf, Produc, index = c("state", "year"), model = "random"))
tests("p", pf, Produc, c("state", "year"))
dump("prandom2", plm(pf, Produc, index = c("state", "year"), model = "random", effect = "twoways"))
ef <- lemp ~ lwage + lcap
dump("ewithin", plm(ef, EmplUK, index = c("firm", "year"), model = "within"))
dump("etwo", plm(ef, EmplUK, index = c("firm", "year"), model = "within", effect = "twoways"))
dump("erandom", plm(ef, EmplUK, index = c("firm", "year"), model = "random"))
dump("erandomt", plm(ef, EmplUK, index = c("firm", "year"), model = "random", effect = "time"))
dump("efd", plm(ef, EmplUK, index = c("firm", "year"), model = "fd"))
dump("ebetween", plm(ef, EmplUK, index = c("firm", "year"), model = "between"))
tests("e", ef, EmplUK, c("firm", "year"))
lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, ", plm ", packageVersion("plm"), '",'), paste(lines, collapse = ",\n"), "}"), "panel_reference.json")
