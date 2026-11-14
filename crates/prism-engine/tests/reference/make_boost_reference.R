# Reference values for gradient boosting (prism-engine/tests/boost_reference.rs), from gbm.
# Run from this folder: Rscript make_boost_reference.R
# Uses boston.csv, pima.csv, airquality.csv and birthwt.csv; writes boost_reference.json. bag.fraction = 1 makes gbm
# deterministic, so every training error, fitted value and relative influence can be compared.
suppressMessages(library(gbm))
v <- c()
put <- function(name, x) if (length(x) == 1 && !is.na(x)) v[[name]] <<- unname(x)
dump <- function(p, formula, data, dist, depth, minobs, ntrees = 100) {
  set.seed(1)
  m <- gbm(formula, data = data, distribution = dist, n.trees = ntrees, interaction.depth = depth, shrinkage = 0.1,
           bag.fraction = 1, n.minobsinnode = minobs, train.fraction = 1, cv.folds = 0, keep.data = TRUE, verbose = FALSE)
  put(paste0(p, ".init"), m$initF)
  for (t in seq_along(m$train.error)) put(paste0(p, ".train_error.", t), m$train.error[t])
  for (i in seq_along(m$fit)) put(paste0(p, ".fit.", i), m$fit[i])
  ri <- summary(m, n.trees = ntrees, plotit = FALSE)
  for (j in seq_len(nrow(ri))) put(paste0(p, ".influence.", ri$var[j]), ri$rel.inf[j])
}
bos <- read.csv("boston.csv")
fb <- medv ~ crim + rm + lstat + ptratio + dis + nox + chas + age + tax
dump("bos1", fb, bos, "gaussian", 1, 10)
dump("bos3", fb, bos, "gaussian", 3, 10)
pima <- read.csv("pima.csv"); pima$y <- as.integer(pima$type == "Yes")
dump("pima", y ~ npreg + glu + bp + skin + bmi + ped + age, pima, "bernoulli", 2, 10)
aq <- read.csv("airquality.csv"); aq <- aq[!is.na(aq$Ozone), ]
dump("aq", Ozone ~ SolarR + Wind + Temp + Month + Day, aq, "gaussian", 2, 5)
bw <- read.csv("birthwt.csv"); bw$race <- factor(bw$race)
dump("bw", low ~ age + lwt + race + smoke + ptl + ht + ui, bw, "bernoulli", 3, 5)
lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, ", gbm ", packageVersion("gbm"), '",'), paste(lines, collapse = ",\n"), "}"), "boost_reference.json")
