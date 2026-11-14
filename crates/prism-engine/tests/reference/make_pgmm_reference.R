# Reference values for dynamic panel GMM (prism-engine/tests/pgmm_reference.rs), from plm's pgmm.
# Run from this folder: Rscript make_pgmm_reference.R
# Uses empluk.csv and grunfeld.csv (from make_panel_reference.R); writes pgmm_reference.json.
suppressMessages(library(plm))
em <- pdata.frame(read.csv("empluk.csv"), index = c("firm", "year"))
gr <- pdata.frame(read.csv("grunfeld.csv"), index = c("firm", "year"))
v <- c()
put <- function(name, x) if (length(x) == 1 && !is.na(x)) v[[name]] <<- unname(x)
key <- function(nm) {
  m <- regmatches(nm, regexec("^lag[(]([A-Za-z_]+), [0-9:]+[)]([0-9]+)$", nm))[[1]]
  if (length(m)) return(paste0(m[2], ".l", m[3]))
  m <- regmatches(nm, regexec("^lag[(]([A-Za-z_]+)(, ([0-9]+))?[)]$", nm))[[1]]
  if (length(m)) return(paste0(m[2], ".l", if (m[4] == "") 1 else m[4]))
  if (grepl("^[0-9]+$", nm)) return(paste0("time.", nm))
  paste0(nm, ".l0")
}
dump <- function(p, m, robust = TRUE) {
  s <- summary(m, robust = robust, time.dummies = TRUE)
  cf <- s$coefficients
  for (j in seq_len(nrow(cf))) {
    k <- paste0(p, ".coef.", key(rownames(cf)[j]))
    put(paste0(k, ".est"), cf[j, 1]); put(paste0(k, ".se"), cf[j, 2]); put(paste0(k, ".z"), cf[j, 3]); put(paste0(k, ".p"), cf[j, 4])
  }
  if (is.list(m$coefficients)) for (j in seq_along(m$coefficients[[1]])) put(paste0(p, ".coef.", key(names(m$coefficients[[1]])[j]), ".step1"), m$coefficients[[1]][j])
  put(paste0(p, ".sargan"), s$sargan$statistic); put(paste0(p, ".sargan.df"), s$sargan$parameter); put(paste0(p, ".sargan.p"), s$sargan$p.value)
  put(paste0(p, ".ar1"), s$m1$statistic); put(paste0(p, ".ar1.p"), s$m1$p.value)
  if (!is.null(s$m2)) { put(paste0(p, ".ar2"), s$m2$statistic); put(paste0(p, ".ar2.p"), s$m2$p.value) }
  put(paste0(p, ".wald"), s$wald.coef$statistic); put(paste0(p, ".wald.p"), s$wald.coef$p.value)
  if (!is.null(s$wald.td)) { put(paste0(p, ".wald.time"), s$wald.td$statistic); put(paste0(p, ".wald.time.p"), s$wald.td$p.value) }
  put(paste0(p, ".instruments"), ncol(m$W[[1]]))
}
# Arellano and Bond's model (as in plm's examples, logs precomputed).
ab <- lemp ~ lag(lemp, 1:2) + lag(lwage, 0:1) + lag(lcap, 0:1) | lag(lemp, 2:99)
dump("ab2", pgmm(ab, em, effect = "twoways", model = "twosteps"))
dump("ab1", pgmm(ab, em, effect = "twoways", model = "onestep"))
dump("ab1n", pgmm(ab, em, effect = "twoways", model = "onestep"), robust = FALSE)
dump("abi", pgmm(ab, em, effect = "individual", model = "twosteps"))
dump("abc", pgmm(ab, em, effect = "twoways", model = "twosteps", collapse = TRUE))
dump("ab4", pgmm(lemp ~ lag(lemp, 1) + lwage + lcap | lag(lemp, 2:4), em, effect = "twoways", model = "twosteps"))
dump("gr", pgmm(inv ~ lag(inv, 1) + value + capital | lag(inv, 2:99), gr, effect = "individual", model = "onestep"))
lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, ", plm ", packageVersion("plm"), '",'), paste(lines, collapse = ",\n"), "}"), "pgmm_reference.json")
