# Reference values for decision trees (prism-engine/tests/tree_reference.rs), from rpart.
# Run from this folder: Rscript make_tree_reference.R
# Uses boston.csv and pima.csv (with their fold columns, from make_predict_reference.R); writes iris.csv and
# birthwt.csv (with seeded fold columns) and tree_reference.json.
suppressMessages(library(rpart))
bos <- read.csv("boston.csv"); pima <- read.csv("pima.csv")
ir <- iris; names(ir) <- c("sepal_length", "sepal_width", "petal_length", "petal_width", "species"); set.seed(3); ir$fold <- sample(rep(1:10, length.out = nrow(ir)))
write.csv(ir, "iris.csv", row.names = FALSE)
bw <- MASS::birthwt; bw$race <- factor(bw$race, labels = c("white", "black", "other"))
set.seed(4); bw$fold <- sample(rep(1:10, length.out = nrow(bw)))
write.csv(bw, "birthwt.csv", row.names = FALSE)
bw$low <- factor(bw$low)

v <- c(); txt <- list()
put <- function(name, x) if (length(x) == 1 && !is.na(x)) v[[name]] <<- unname(x)
nodes <- function(p, fit) {
  fr <- fit$frame
  ids <- as.integer(rownames(fr))
  srow <- 1
  imp <- fit$variable.importance
  if (!is.null(imp)) for (nm in names(imp)) put(paste0(p, ".importance.", nm), imp[[nm]])
  for (k in seq_len(nrow(fr))) {
    key <- paste0(p, ".node.", ids[k])
    put(paste0(key, ".n"), fr$n[k]); put(paste0(key, ".yval"), fr$yval[k]); put(paste0(key, ".dev"), fr$dev[k])
    if (fr$var[k] != "<leaf>") {
      if (!is.factor(fit$model[[as.character(fr$var[k])]]) && !is.null(fit$splits)) {
        put(paste0(key, ".cut"), fit$splits[srow, "index"])
      }
      if (fr$nsurrogate[k] > 0) for (j in seq_len(fr$nsurrogate[k])) {
        r <- srow + fr$ncompete[k] + j
        sk <- paste0(key, ".sur.", j)
        txt[[paste0(sk, ".var")]] <<- rownames(fit$splits)[r]
        put(paste0(sk, ".agree"), fit$splits[r, "improve"]); put(paste0(sk, ".adj"), fit$splits[r, "adj"])
        if (fit$splits[r, "ncat"] < 2) put(paste0(sk, ".cut"), fit$splits[r, "index"])
      }
      srow <- srow + 1 + fr$ncompete[k] + fr$nsurrogate[k]
    }
  }
}
dump <- function(p, formula, data, method, ...) {
  fit <- rpart(formula, data, method = method, xval = data$fold, model = TRUE, ...)
  cpt <- fit$cptable
  for (k in seq_len(nrow(cpt))) {
    key <- paste0(p, ".cp.", k)
    put(paste0(key, ".CP"), cpt[k, "CP"]); put(paste0(key, ".nsplit"), cpt[k, "nsplit"]); put(paste0(key, ".rel_error"), cpt[k, "rel error"])
    put(paste0(key, ".xerror"), cpt[k, "xerror"]); put(paste0(key, ".xstd"), cpt[k, "xstd"])
  }
  put(paste0(p, ".cp.count"), nrow(cpt))
  nodes(p, fit)
  imin <- which.min(cpt[, "xerror"])
  i1se <- which(cpt[, "xerror"] <= cpt[imin, "xerror"] + cpt[imin, "xstd"])[1]
  for (w in list(c("min", imin), c("1se", i1se))) {
    pr <- prune(fit, cp = cpt[as.integer(w[2]), "CP"])
    put(paste0(p, w[1], ".chosen.nsplit"), cpt[as.integer(w[2]), "nsplit"])
    nodes(paste0(p, w[1]), pr)
  }
}
dump("boston", medv ~ crim + rm + lstat + ptratio + dis + nox + chas + age + tax, bos, "anova")
dump("pima", type ~ npreg + glu + bp + skin + bmi + ped + age, pima, "class")
dump("iris", species ~ sepal_length + sepal_width + petal_length + petal_width, ir, "class")
dump("bwt", bwt ~ age + lwt + smoke + ptl + ht + ui + race, bw, "anova")
dump("low", low ~ age + lwt + smoke + ptl + ht + ui + race, bw, "class")
# Missing predictor values: surrogate splits.
aq <- airquality; names(aq)[2] <- "SolarR"; set.seed(5); aq$fold <- sample(rep(1:10, length.out = nrow(aq)))
write.csv(aq, "airquality.csv", row.names = FALSE, na = "")
ky <- kyphosis; set.seed(6)
ky$Age[sample(nrow(ky), 10)] <- NA; ky$Number[sample(nrow(ky), 8)] <- NA; ky$Start[sample(nrow(ky), 10)] <- NA
ky[c(3, 40), c("Age", "Number", "Start")] <- NA
ky$fold <- sample(rep(1:10, length.out = nrow(ky)))
write.csv(ky, "kyphosis.csv", row.names = FALSE, na = "")
bm <- bw; set.seed(7)
bm$race[sample(nrow(bm), 15)] <- NA; bm$lwt[sample(nrow(bm), 15)] <- NA; bm$age[sample(nrow(bm), 10)] <- NA; bm$smoke[sample(nrow(bm), 10)] <- NA
write.csv(bm, "birthwt_missing.csv", row.names = FALSE, na = "")
dump("aq", Ozone ~ SolarR + Wind + Temp + Month + Day, aq, "anova")
dump("ky", Kyphosis ~ Age + Number + Start, ky, "class", control = rpart.control(minsplit = 10))
dump("ky0", Kyphosis ~ Age + Number + Start, ky, "class", control = rpart.control(minsplit = 10, maxsurrogate = 0))
dump("bwm", bwt ~ age + lwt + smoke + ptl + ht + ui + race, bm, "anova")
dump("lowm", low ~ age + lwt + smoke + ptl + ht + ui + race, bm, "class")
lines <- c(sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v))), sprintf('  "%s": "%s"', names(txt), unlist(txt)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, ", rpart ", packageVersion("rpart"), '",'), paste(lines, collapse = ",\n"), "}"), "tree_reference.json")
