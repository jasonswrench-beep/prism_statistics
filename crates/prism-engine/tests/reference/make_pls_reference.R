# Reference values for PLS-SEM (prism-engine/tests/pls_reference.rs), from seminr.
# Run from this folder: Rscript make_pls_reference.R
# Writes seminr's public example data (the mobile-phone customer satisfaction survey, mobi) and pls_reference.json.
suppressMessages(library(seminr))
write.csv(mobi, "mobi.csv", row.names = FALSE)

v <- c()
put <- function(name, x) if (!is.na(x)) v[[name]] <<- unname(x)

mm <- constructs(
  composite("Image", multi_items("IMAG", 1:5)),
  composite("Expectation", multi_items("CUEX", 1:3)),
  composite("Quality", multi_items("PERQ", 1:7)),
  composite("Value", multi_items("PERV", 1:2)),
  composite("Satisfaction", multi_items("CUSA", 1:3)),
  composite("Complaints", single_item("CUSCO")),
  composite("Loyalty", multi_items("CUSL", 1:3), weights = mode_B)
)
sm <- relationships(
  paths(from = "Image", to = c("Expectation", "Satisfaction", "Loyalty")),
  paths(from = "Expectation", to = c("Quality", "Value", "Satisfaction")),
  paths(from = "Quality", to = c("Value", "Satisfaction")),
  paths(from = "Value", to = c("Satisfaction")),
  paths(from = "Satisfaction", to = c("Complaints", "Loyalty")),
  paths(from = "Complaints", to = "Loyalty")
)
# seminr stops when the outer weights change by less than 1e-7 in total; the references are converged further.
fit <- function(data, smod = sm, scheme = path_weighting) estimate_pls(data, mm, smod, inner_weights = scheme, stopCriterion = 12)
m <- suppressMessages(fit(mobi))
s <- summary(m)
cons <- m$constructs
pc <- m$path_coef
for (a in rownames(pc)) for (b in colnames(pc)) if (pc[a, b] != 0) put(paste0("pls.path.", a, ".", b), pc[a, b])
for (b in colnames(m$rSquared)) { put(paste0("pls.r2.", b), m$rSquared["Rsq", b]); put(paste0("pls.adjr2.", b), m$rSquared["AdjRsq", b]) }
L <- m$outer_loadings; W <- m$outer_weights
for (i in rownames(L)) { k <- which(L[i, ] != 0); put(paste0("pls.loading.", i), L[i, k]); put(paste0("pls.weight.", i), W[i, k]) }
rel <- s$reliability
for (k in rownames(rel)) { put(paste0("pls.alpha.", k), rel[k, "alpha"]); put(paste0("pls.rhoA.", k), rel[k, "rhoA"]); put(paste0("pls.rhoC.", k), rel[k, "rhoC"]); put(paste0("pls.ave.", k), rel[k, "AVE"]) }
h <- s$validity$htmt
for (a in rownames(h)) for (b in colnames(h)) if (!is.na(suppressWarnings(as.numeric(h[a, b]))) && a != b) put(paste0("pls.htmt.", b, ".", a), as.numeric(h[a, b]))
f2 <- s$fSquare
for (a in rownames(pc)) for (b in colnames(pc)) if (pc[a, b] != 0) put(paste0("pls.f2.", a, ".", b), f2[a, b])
te <- s$total_effects
for (a in rownames(te)) for (b in colnames(te)) if (te[a, b] != 0) put(paste0("pls.total.", a, ".", b), te[a, b])
for (dv in names(s$vif_antecedents)) { vv <- s$vif_antecedents[[dv]]; for (iv in names(vv)) put(paste0("pls.vif.", dv, ".", iv), vv[[iv]]) }

# SRMR of the saturated composite model (Henseler et al., 2014): indicator correlations implied by the loadings
# (composite loadings for formative blocks) and the observed construct correlations between blocks, the observed
# correlations within blocks.
items <- m$mmVariables
S <- cor(mobi[, items])
block <- sapply(items, function(i) which(L[i, ] != 0))
R <- cor(m$construct_scores)
Sig <- S
for (i in seq_along(items)) for (j in seq_along(items)) if (block[i] != block[j]) Sig[i, j] <- L[i, block[i]] * R[block[i], block[j]] * L[j, block[j]]
put("pls.srmr", sqrt(mean(((S - Sig)[lower.tri(S, diag = TRUE)])^2)))

# Q²predict (PLSpredict, Shmueli et al., 2019) with ten folds made by a fixed rule (case i in fold i mod 10), each
# training model estimated by seminr and its holdout predicted from direct antecedents.
fold <- (seq_len(nrow(mobi)) - 1) %% 10
endo_items <- unlist(lapply(c("Expectation", "Satisfaction", "Loyalty", "Quality", "Value", "Complaints"), function(k) rownames(L)[L[, k] != 0]))
sse <- setNames(rep(0, length(endo_items)), endo_items); sso <- sse
for (f in 0:9) {
  train <- mobi[fold != f, ]; test <- mobi[fold == f, ]
  mt <- suppressMessages(fit(train))
  pr <- predict(mt, test)
  for (it in endo_items) {
    sse[it] <- sse[it] + sum((test[[it]] - pr$predicted_items[, it])^2)
    sso[it] <- sso[it] + sum((test[[it]] - mean(train[[it]]))^2)
  }
}
for (it in endo_items) put(paste0("pls.q2.", it), 1 - sse[it] / sso[it])

# The factorial weighting scheme.
mf <- suppressMessages(fit(mobi, scheme = path_factorial))
for (a in rownames(mf$path_coef)) for (b in colnames(mf$path_coef)) if (mf$path_coef[a, b] != 0) put(paste0("plsf.path.", a, ".", b), mf$path_coef[a, b])

# Groups (for the multigroup analysis): odd and even cases, estimated separately.
grp <- ifelse(seq_len(nrow(mobi)) %% 2 == 1, "odd", "even")
for (g in c("odd", "even")) {
  mg <- suppressMessages(fit(mobi[grp == g, ]))
  for (a in rownames(mg$path_coef)) for (b in colnames(mg$path_coef)) if (mg$path_coef[a, b] != 0) put(paste0("mga.", g, ".path.", a, ".", b), mg$path_coef[a, b])
}

lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, ", seminr ", packageVersion("seminr"), '",'), paste(lines, collapse = ",\n"), "}"), "pls_reference.json")
