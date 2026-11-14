# Reference values for random forests (prism-engine/tests/forest_reference.rs), from randomForest.
# Run from this folder: Rscript make_forest_reference.R
# Uses boston.csv, iris.csv, pima.csv and birthwt.csv; writes forest_reference.json.
# 1. Deterministic trees (mtry = all predictors, every case once): fitted values must match exactly.
# 2. Forests over ten seeds: the mean and SD of the out-of-bag error and of each predictor's importance.
suppressMessages(library(randomForest))
v <- c()
put <- function(name, x) if (length(x) == 1 && !is.na(x)) v[[name]] <<- unname(x)
bos <- read.csv("boston.csv"); fb <- medv ~ crim + rm + lstat + ptratio + dis + nox + chas + age + tax
bw <- read.csv("birthwt.csv"); bw$race <- factor(bw$race)
one <- function(p, formula, data, nodesize) {
  set.seed(1)
  rf <- randomForest(formula, data = data, ntree = 1, mtry = length(all.vars(formula)) - 1, replace = FALSE, sampsize = nrow(data), nodesize = nodesize)
  f <- predict(rf, newdata = data)
  for (i in seq_along(f)) put(paste0(p, ".fitted.", i), f[i])
}
one("det5", fb, bos, 5)  # not compared: a node has two equally good splits, which randomForest breaks at random
one("det20", fb, bos, 20)
one("detcat", bwt ~ age + lwt + race + smoke + ptl + ht + ui, bw, 5)
many <- function(p, formula, data, class) {
  stats <- list()
  for (s in 1:10) {
    set.seed(s)
    rf <- randomForest(formula, data = data, ntree = 500, importance = TRUE)
    im <- importance(rf)
    e <- if (class) rf$err.rate[500, "OOB"] else rf$rsq[500]
    stats[[s]] <- c(err = e, setNames(im[, if (class) "MeanDecreaseGini" else "IncNodePurity"], paste0("purity.", rownames(im))), setNames(im[, if (class) "MeanDecreaseAccuracy" else "%IncMSE"], paste0("permutation.", rownames(im))))
  }
  m <- do.call(rbind, stats)
  for (nm in colnames(m)) { put(paste0(p, ".", nm, ".mean"), mean(m[, nm])); put(paste0(p, ".", nm, ".sd"), sd(m[, nm])) }
}
many("bos", fb, bos, FALSE)
ir <- read.csv("iris.csv"); ir$species <- factor(ir$species)
many("iris", species ~ sepal_length + sepal_width + petal_length + petal_width, ir, TRUE)
pima <- read.csv("pima.csv"); pima$type <- factor(pima$type)
many("pima", type ~ npreg + glu + bp + skin + bmi + ped + age, pima, TRUE)
lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, ", randomForest ", packageVersion("randomForest"), '",'), paste(lines, collapse = ",\n"), "}"), "forest_reference.json")
