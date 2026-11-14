# Reference values for control charts and capability (prism-engine/tests/spc_reference.rs), from qcc 2.7.
# Run from this folder: Rscript make_spc_reference.R
# Writes CSVs of qcc's example data (pistonrings, orangejuice, circuit, pcmanufact) and spc_reference.json.
suppressMessages(library(qcc))
data(pistonrings); data(orangejuice); data(circuit); data(pcmanufact)
pr <- pistonrings[, c("sample", "diameter", "trial")]
write.csv(pr[pr$trial, 1:2], "rings_trial.csv", row.names = FALSE)
write.csv(pr[, 1:2], "rings_all.csv", row.names = FALSE)
# Unequal subgroups: drop a few measurements.
uneq <- pr[pr$trial, 1:2][-c(3, 9, 10, 27, 51, 52, 88), ]
write.csv(uneq, "rings_unequal.csv", row.names = FALSE)
wide <- qcc.groups(pr$diameter[pr$trial], pr$sample[pr$trial])
colnames(wide) <- paste0("d", 1:5)
write.csv(wide, "rings_wide.csv", row.names = FALSE)
oj <- orangejuice[orangejuice$trial, c("sample", "D", "size")]
write.csv(oj, "orangejuice.csv", row.names = FALSE)
oj2 <- oj; oj2$size <- oj2$size + c(0, 5, -3, 10, 0, -5)  # varying sample sizes
write.csv(oj2, "orangejuice_varying.csv", row.names = FALSE)
ci <- circuit[circuit$trial, c("x", "size")]
write.csv(ci, "circuit.csv", row.names = FALSE)
pc <- pcmanufact; pc$size <- pc$size + rep(c(0, 1, 2, 0, 3), length.out = nrow(pc))
write.csv(pc, "pcmanufact.csv", row.names = FALSE)

v <- c(); txt <- list()
put <- function(name, x) if (length(x) == 1 && !is.na(x)) v[[name]] <<- unname(x)
chart <- function(key, q) {
  m <- length(q$statistics)
  put(paste0(key, ".center"), q$center[1]); put(paste0(key, ".count"), m)
  lim <- q$limits; if (nrow(lim) == 1) lim <- lim[rep(1, m), , drop = FALSE]
  for (i in seq_len(m)) {
    put(paste0(key, ".stat.", i), q$statistics[i]); put(paste0(key, ".lcl.", i), lim[i, 1]); put(paste0(key, ".ucl.", i), lim[i, 2])
  }
  bl <- sort(q$violations$beyond.limits); vr <- sort(q$violations$violating.runs)
  put(paste0(key, ".beyond"), length(bl)); put(paste0(key, ".runs"), length(vr))
  txt[[paste0(key, ".beyond.points")]] <<- paste(names(q$statistics)[bl], collapse = ", ")
  txt[[paste0(key, ".runs.points")]] <<- paste(names(q$statistics)[vr], collapse = ", ")
}
cap <- function(p, q, lsl, usl, target) {
  pc <- if (missing(target)) process.capability(q, spec.limits = c(lsl, usl), print = FALSE) else process.capability(q, spec.limits = c(lsl, usl), target = target, print = FALSE)
  tab <- pc$indices
  for (r in c("Cp", "Cp_l", "Cp_u", "Cp_k", "Cpm")) {
    key <- switch(r, Cp = "cp", Cp_l = "cp_l", Cp_u = "cp_u", Cp_k = "cpk", Cpm = "cpm")
    put(paste0(p, ".", key), tab[r, 1]); put(paste0(p, ".", key, ".lower"), tab[r, 2]); put(paste0(p, ".", key, ".upper"), tab[r, 3])
  }
  put(paste0(p, ".exp_lsl"), pc$exp[1]); put(paste0(p, ".exp_usl"), pc$exp[2]); put(paste0(p, ".obs_lsl"), pc$obs[1]); put(paste0(p, ".obs_usl"), pc$obs[2])
}
g <- function(d) qcc.groups(d$diameter, d$sample)
for (set in list(list("trial", pr[pr$trial, ]), list("all", pr), list("unequal", uneq))) {
  x <- g(set[[2]]); p <- set[[1]]
  qx <- qcc(x, type = "xbar", plot = FALSE); chart(paste0(p, ".xbar"), qx); put(paste0(p, ".std_dev"), qx$std.dev)
  chart(paste0(p, ".r"), qcc(x, type = "R", plot = FALSE))
  qs <- qcc(x, type = "xbar", std.dev = "UWAVE-SD", plot = FALSE); chart(paste0(p, "s.xbar"), qs); put(paste0(p, "s.std_dev"), qs$std.dev)
  chart(paste0(p, "s.s"), qcc(x, type = "S", plot = FALSE))
  if (p == "trial") { cap("trial", qx, 73.95, 74.05); cap("trialt", qx, 73.95, 74.05, 74.01) }
}
# Individuals and moving range (the first 50 measurements of the trial rings).
xi <- pr$diameter[pr$trial][1:50]
write.csv(data.frame(diameter = xi), "rings_individuals.csv", row.names = FALSE)
qi <- qcc(xi, type = "xbar.one", plot = FALSE); chart("imr.i", qi); put("imr.std_dev", qi$std.dev)
chart("imr.mr", qcc(cbind(xi[-length(xi)], xi[-1]), type = "R", plot = FALSE))
cap("imr", qi, 73.97, 74.03)
chart("p.p", qcc(oj$D, sizes = oj$size, type = "p", plot = FALSE))
chart("np.np", qcc(oj$D, sizes = oj$size, type = "np", plot = FALSE))
chart("pv.p", qcc(oj2$D, sizes = oj2$size, type = "p", plot = FALSE))
chart("c.c", qcc(ci$x, sizes = ci$size, type = "c", plot = FALSE))
chart("u.u", qcc(pc$x, sizes = pc$size, type = "u", plot = FALSE))
# CUSUM and EWMA: all 40 ring samples (the last 15 drift), the 50 individual measurements, and options.
cus <- function(p, q) {
  m <- length(q$statistics); lab <- names(q$statistics)
  put(paste0(p, ".std_dev"), q$std.dev); put(paste0(p, ".cusum_pos.count"), m)
  for (i in seq_len(m)) { put(paste0(p, ".cusum_pos.stat.", i), q$pos[i]); put(paste0(p, ".cusum_neg.stat.", i), q$neg[i]); put(paste0(p, ".cusum_pos.ucl.", i), q$decision.interval); put(paste0(p, ".cusum_neg.lcl.", i), -q$decision.interval) }
  put(paste0(p, ".cusum_pos.beyond"), length(q$violations$upper)); put(paste0(p, ".cusum_neg.beyond"), length(q$violations$lower))
  txt[[paste0(p, ".cusum_pos.beyond.points")]] <<- paste(lab[sort(q$violations$upper)], collapse = ", ")
  txt[[paste0(p, ".cusum_neg.beyond.points")]] <<- paste(lab[sort(q$violations$lower)], collapse = ", ")
}
ewm <- function(p, q) {
  m <- length(q$statistics); lab <- names(q$statistics)
  put(paste0(p, ".std_dev"), q$std.dev); put(paste0(p, ".ewma.center"), q$center); put(paste0(p, ".ewma.count"), m)
  for (i in seq_len(m)) { put(paste0(p, ".ewma.stat.", i), q$y[i]); put(paste0(p, ".ewma.lcl.", i), q$limits[i, 1]); put(paste0(p, ".ewma.ucl.", i), q$limits[i, 2]) }
  put(paste0(p, ".ewma.beyond"), length(q$violations))
  txt[[paste0(p, ".ewma.beyond.points")]] <<- paste(lab[sort(q$violations)], collapse = ", ")
}
xa <- g(pr)
cus("cusum", cusum(xa, plot = FALSE))
cus("cusumo", cusum(xa, decision.interval = 4, se.shift = 1.5, head.start = 2, center = 74, std.dev = 0.01, plot = FALSE))
cus("cusumi", cusum(xi, plot = FALSE))
ewm("ewma", ewma(xa, plot = FALSE))
ewm("ewmao", ewma(xa, lambda = 0.3, nsigmas = 2.7, center = 74, std.dev = 0.01, plot = FALSE))
ewm("ewmai", ewma(xi, plot = FALSE))
cus("cusumu", cusum(g(uneq), plot = FALSE))
ewm("ewmau", ewma(g(uneq), plot = FALSE))
lines <- c(sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v))), sprintf('  "%s": "%s"', names(txt), unlist(txt)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, ", qcc ", packageVersion("qcc"), '",'), paste(lines, collapse = ",\n"), "}"), "spc_reference.json")
