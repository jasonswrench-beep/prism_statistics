# Reference values for test scoring and item analysis from letter answers (prism-engine/tests/item_analysis_reference.rs).
# Run from this folder: Rscript make_item_analysis_reference.R
# Writes item_letters.csv (a simulated class of 120 answering 12 four-option questions, with blanks, one sheet left
# entirely blank, one question with two right answers and one select-all question) and item_analysis_reference.json.
# Difficulty, the item-in-total point-biserial and KR-20 for the single-answer questions also come from psych's
# score.multiple.choice, an independent implementation; every other figure is base R from the definitions in
# prism-engine/src/analysis/item_analysis.rs.
suppressMessages(library(psych))
set.seed(20261115)
N <- 120; k <- 12
theta <- rnorm(N)
b <- seq(-1.5, 1.5, length.out = k); a <- c(rep(1.4, 10), 0.2, -0.6) # q11 weak, q12 runs the wrong way
key <- c("B", "A", "D", "C", "B", "A", "C", "D", "B", "A", "C", "D")
resp <- matrix("", N, k)
for (j in 1:k) {
  p <- plogis(a[j] * (theta - b[j]))
  right <- runif(N) < p
  wrong <- sample(setdiff(LETTERS[1:4], key[j]), N, replace = TRUE, prob = c(0.6, 0.3, 0.1))
  resp[, j] <- ifelse(right, key[j], wrong)
}
resp[sample(N * k, 40)] <- "" # blanks
# q10 is select-all, keyed AC; answers are sets of letters.
sets <- c("AC", "A", "C", "AB", "ACD", "BC")
resp[, 10] <- ifelse(theta + rnorm(N, 0, 0.8) > 0, "AC", sample(sets[-1], N, replace = TRUE))
resp[sample(N, 5), 10] <- ""
resp[7, ] <- "" # a sheet left blank: left out
colnames(resp) <- sprintf("q%02d", 1:k)
write.csv(as.data.frame(resp), "item_letters.csv", row.names = FALSE, na = "")

keys <- as.list(key); keys[[3]] <- c("D", "B"); keys[[10]] <- "AC"
keep <- rowSums(resp != "") > 0
R <- resp[keep, ]; n <- nrow(R)
X <- sapply(1:k, function(j) as.numeric(R[, j] %in% keys[[j]]))
tot <- rowSums(X)
v <- list()
put <- function(name, x) if (length(x) == 1 && !is.na(x)) v[[name]] <<- unname(x)
varn <- function(x) mean((x - mean(x))^2)
put("n", n); put("k", k); put("mean", mean(tot)); put("sd", sd(tot)); put("sd.n", sqrt(varn(tot)))
kr20 <- k / (k - 1) * (1 - sum(apply(X, 2, varn)) / varn(tot))
put("kr20", kr20); put("reliability", kr20)
put("kr21", k / (k - 1) * (1 - mean(tot) * (k - mean(tot)) / (k * varn(tot))))
put("sem", sd(tot) * sqrt(1 - kr20))
g <- (27 * n + 50) %/% 100
ord <- order(-tot, seq_len(n))
up <- ord[1:g]; lo <- ord[(n - g + 1):n]
put("group.size", g)
grp <- rep("middle", n); grp[up] <- "upper"; grp[lo] <- "lower"
for (j in 1:k) {
  key_j <- paste0("item.", j)
  put(paste0(key_j, ".p"), mean(X[, j]))
  put(paste0(key_j, ".rpb"), cor(X[, j], tot))
  put(paste0(key_j, ".rpb_corrected"), cor(X[, j], tot - X[, j]))
  Y <- X[, -j]; tt <- rowSums(Y); kk <- k - 1
  put(paste0(key_j, ".alpha_deleted"), kk / (kk - 1) * (1 - sum(apply(Y, 2, varn)) / varn(tt)))
  put(paste0(key_j, ".upper"), mean(X[up, j])); put(paste0(key_j, ".lower"), mean(X[lo, j]))
  put(paste0(key_j, ".D"), mean(X[up, j]) - mean(X[lo, j]))
  put(paste0(key_j, ".blank"), sum(R[, j] == ""))
  for (o in unique(c(R[, j], keys[[j]]))) {
    nm <- if (o == "") "blank" else o
    hit <- R[, j] == o
    put(paste0(key_j, ".opt.", nm, ".n"), sum(hit))
    for (gname in c("upper", "middle", "lower")) put(paste0(key_j, ".opt.", nm, ".", gname), sum(hit & grp == gname))
  }
}

# Subscores by a label per question (chapters "1", "2", "3", "10"; q10 and q12 in none; "3" has one item, so no
# reliability), listed in natural order, as item_analysis.rs does.
chap <- c("2", "2", "2", "10", "10", "10", "10", "1", "1", "", "3", "")
for (j in seq_along(c("1", "2", "3", "10"))) {
  g <- c("1", "2", "3", "10")[j]
  cols <- which(chap == g)
  Y <- X[, cols, drop = FALSE]; s <- rowSums(Y); kk <- length(cols)
  key_j <- paste0("subscore.", j)
  put(paste0(key_j, ".k"), kk); put(paste0(key_j, ".possible"), kk)
  put(paste0(key_j, ".mean"), mean(s)); put(paste0(key_j, ".sd"), sd(s)); put(paste0(key_j, ".mean.p"), mean(s) / kk)
  if (kk > 1) put(paste0(key_j, ".reliability"), kk / (kk - 1) * (1 - sum(apply(Y, 2, varn)) / varn(s)))
}
put("subscores", 4)

# psych's own scoring of the single-answer questions (all but q03 and q10), on the same sheets.
single <- setdiff(1:k, c(3, 10))
num <- matrix(match(R[, single], LETTERS[1:4]), n)
num[is.na(num)] <- 0 # a blank matches no option and scores 0
sc <- score.multiple.choice(match(key[single], LETTERS[1:4]), num, score = TRUE, short = FALSE, skew = FALSE)
for (s in seq_along(single)) put(paste0("psych.item.", single[s], ".p"), sc$item.stats[s, "mean"])
# (psych rounds its alpha to two places, so it is not used.)
put("psych.n", n)
# psych's item-whole r is with the total of these single-answer questions.
for (s in seq_along(single)) put(paste0("psych.item.", single[s], ".r"), sc$item.stats[s, "r"])

lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, ", psych ", packageVersion("psych"), '",'), paste(lines, collapse = ",\n"), "}"), "item_analysis_reference.json")
