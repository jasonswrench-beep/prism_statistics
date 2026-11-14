# Reference values for penalized regression with cross-validation (prism-engine/tests/predict_reference.rs), from
# glmnet. Run from this folder: Rscript make_predict_reference.R
# Writes boston.csv and pima.csv (with a seeded fold column, so both programs use the same folds) and
# predict_reference.json.
suppressMessages(library(glmnet))
bos <- MASS::Boston
set.seed(1); bos$fold <- sample(rep(1:10, length.out = nrow(bos)))
write.csv(bos, "boston.csv", row.names = FALSE)
pima <- MASS::Pima.tr
set.seed(2); pima$fold <- sample(rep(1:10, length.out = nrow(pima)))
write.csv(pima, "pima.csv", row.names = FALSE)

v <- c()
put <- function(name, x) if (length(x) == 1 && !is.na(x)) v[[name]] <<- unname(x)
run <- function(p, x, y, family, alpha, foldid) {
  ctl <- list(thresh = 1e-16, maxit = 1e7)
  fit <- glmnet(x, y, family = family, alpha = alpha, control = ctl)
  cv <- cv.glmnet(x, y, family = family, alpha = alpha, foldid = foldid, control = ctl, keep = TRUE)
  nl <- length(fit$lambda)
  put(paste0(p, ".lambda.count"), nl)
  stopifnot(isTRUE(all.equal(fit$lambda, cv$lambda)))
  for (l in seq_len(nl)) {
    put(paste0(p, ".path.", l, ".lambda"), fit$lambda[l])
    put(paste0(p, ".path.", l, ".cvm"), cv$cvm[l]); put(paste0(p, ".path.", l, ".cvsd"), cv$cvsd[l])
  }
  cf <- as.matrix(coef(fit))
  for (l in c(1, floor(nl / 2) + 1, nl)) for (j in seq_len(nrow(cf))) put(paste0(p, ".path.", l, ".coef.", j - 1), cf[j, l])
  for (k in c("min", "1se")) {
    lam <- cv[[paste0("lambda.", k)]]; i <- which(cv$lambda == lam)
    put(paste0(p, ".lambda.", k), lam); put(paste0(p, ".cvm.", k), cv$cvm[i]); put(paste0(p, ".cvsd.", k), cv$cvsd[i])
    c2 <- as.matrix(coef(cv, s = paste0("lambda.", k)))
    for (j in seq_len(nrow(c2))) put(paste0(p, ".coef.", k, ".", j - 1), c2[j, 1])
    put(paste0(p, ".nonzero.", k), sum(c2[-1, 1] != 0))
  }
  # Out-of-fold predictions at lambda.1se (the default choice).
  i <- which(cv$lambda == cv$lambda.1se)
  pv <- cv$fit.preval[, i]
  if (family == "gaussian") {
    put(paste0(p, ".cv.rmse"), sqrt(mean((y - pv)^2))); put(paste0(p, ".cv.mae"), mean(abs(y - pv)))
    put(paste0(p, ".cv.r2"), 1 - sum((y - pv)^2) / sum((y - mean(y))^2))
  } else {
    yy <- as.numeric(y) - 1; pv <- plogis(pv)  # fit.preval is on the link scale
    put(paste0(p, ".cv.auc"), as.numeric(pROC::auc(yy, pv, quiet = TRUE, direction = "<")))
    put(paste0(p, ".cv.accuracy"), mean((pv >= 0.5) == (yy == 1))); put(paste0(p, ".cv.brier"), mean((pv - yy)^2))
  }
}
xb <- as.matrix(bos[, setdiff(names(bos), c("medv", "fold"))])
run("bostonlasso", xb, bos$medv, "gaussian", 1, bos$fold)
run("bostonridge", xb, bos$medv, "gaussian", 0, bos$fold)
run("bostonelastic", xb, bos$medv, "gaussian", 0.5, bos$fold)
xp <- as.matrix(pima[, c("npreg", "glu", "bp", "skin", "bmi", "ped", "age")])
run("pimalasso", xp, pima$type, "binomial", 1, pima$fold)
run("pimaridge", xp, pima$type, "binomial", 0, pima$fold)
lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, ", glmnet ", packageVersion("glmnet"), '",'), paste(lines, collapse = ",\n"), "}"), "predict_reference.json")
