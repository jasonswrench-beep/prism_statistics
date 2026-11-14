# Reference values for generalized estimating equations (prism-engine/tests/gee_reference.rs), from geepack.
# Run from this folder: Rscript make_gee_reference.R
# Writes ohio.csv, dietox.csv and epil.csv and gee_reference.json.
suppressMessages(library(geepack))
data(ohio); data(dietox)
epil <- MASS::epil
write.csv(ohio, "ohio.csv", row.names = FALSE)
write.csv(dietox[, c("Pig", "Time", "Weight", "Cu", "Evit")], "dietox.csv", row.names = FALSE)
write.csv(epil[, c("subject", "period", "y", "trt", "base", "age")], "epil.csv", row.names = FALSE)
v <- c()
put <- function(name, x) if (length(x) == 1 && !is.na(x)) v[[name]] <<- unname(x)
tight <- geese.control(epsilon = 1e-12, maxit = 1000)
dump <- function(p, f) {
  s <- summary(f)$coefficients
  for (j in seq_len(nrow(s))) {
    k <- paste0(p, ".coef.", j)
    put(paste0(k, ".b"), s[j, "Estimate"]); put(paste0(k, ".se"), s[j, "Std.err"])
    put(paste0(k, ".wald"), s[j, "Wald"]); put(paste0(k, ".p"), s[j, "Pr(>|W|)"])
    put(paste0(k, ".naive_se"), sqrt(f$geese$vbeta.naiv[j, j]))
  }
  put(paste0(p, ".phi"), f$geese$gamma)
  for (k in seq_along(f$geese$alpha)) put(paste0(p, ".alpha.", k), f$geese$alpha[k])
  q <- QIC(f)
  put(paste0(p, ".qic"), q[["QIC"]]); put(paste0(p, ".qicu"), q[["QICu"]]); put(paste0(p, ".quasi"), q[["Quasi Lik"]])
  put(paste0(p, ".cic"), q[["CIC"]]); put(paste0(p, ".qicc"), q[["QICC"]])
  put(paste0(p, ".clusters"), length(unique(f$id)))
}
for (cs in c("independence", "exchangeable", "ar1", "unstructured")) {
  dump(paste0("ohio", cs), geeglm(resp ~ age + smoke, id = id, data = ohio, family = binomial, corstr = cs, control = tight))
}
dietox$Cu <- factor(dietox$Cu)
for (cs in c("exchangeable", "ar1")) {
  dump(paste0("dietox", cs), geeglm(Weight ~ Time + Cu, id = Pig, waves = Time, data = dietox, family = gaussian, corstr = cs, control = tight))
}
dump("epil", geeglm(y ~ trt + base + age, id = subject, data = epil, family = poisson, corstr = "exchangeable", control = tight))
lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, ", geepack ", packageVersion("geepack"), '",'), paste(lines, collapse = ",\n"), "}"), "gee_reference.json")
