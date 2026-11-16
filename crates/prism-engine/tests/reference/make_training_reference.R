# Reference values for the training evaluation report by Kirkpatrick level (prism-engine/tests/training_reference.rs).
# Run from this folder: Rscript make_training_reference.R
# Reads the Training evaluation example dataset (crates/prism-api/data/examples/training_evaluation.csv, made by
# tools/make_examples.py) and writes training_reference.json. Base R only: t.test for the paired and Welch tests, and
# the definitions in prism-engine/src/analysis/kirkpatrick.rs for the rest.
d <- read.csv("../../../prism-api/data/examples/training_evaluation.csv")
v <- c()
put <- function(name, x) if (length(x) == 1 && !is.na(x)) v[[name]] <<- unname(x)

# Level 1: four reaction items on a 1-5 scale; favorable is 4 or 5 (the top two points).
items <- paste0("react_", 1:4)
for (i in seq_along(items)) {
  x <- na.omit(d[[items[i]]])
  put(paste0("reaction.", i, ".n"), length(x)); put(paste0("reaction.", i, ".mean"), mean(x))
  put(paste0("reaction.", i, ".sd"), sd(x)); put(paste0("reaction.", i, ".favorable"), mean(x >= 4))
}
all <- unlist(d[items]); all <- all[!is.na(all)]
put("reaction.favorable", mean(all >= 4))
cc <- na.omit(d[items])
pm <- rowMeans(cc)
put("reaction.scale.n", length(pm)); put("reaction.scale.mean", mean(pm)); put("reaction.scale.sd", sd(pm))
k <- ncol(cc); put("reaction.alpha", k / (k - 1) * (1 - sum(apply(cc, 2, var)) / var(rowSums(cc))))

paired <- function(key, pre, post, max = NA) {
  ok <- !is.na(pre) & !is.na(post); a <- pre[ok]; z <- post[ok]
  t <- t.test(z, a, paired = TRUE)
  put(paste0(key, ".n"), length(a)); put(paste0(key, ".pre.mean"), mean(a)); put(paste0(key, ".pre.sd"), sd(a))
  put(paste0(key, ".post.mean"), mean(z)); put(paste0(key, ".post.sd"), sd(z))
  put(paste0(key, ".gain"), mean(z - a)); put(paste0(key, ".gain.sd"), sd(z - a))
  put(paste0(key, ".t"), t$statistic); put(paste0(key, ".df"), t$parameter); put(paste0(key, ".p"), t$p.value)
  put(paste0(key, ".ci.lower"), t$conf.int[1]); put(paste0(key, ".ci.upper"), t$conf.int[2])
  put(paste0(key, ".d"), mean(z - a) / sd(z - a))
  if (!is.na(max)) {
    put(paste0(key, ".hake"), (mean(z) - mean(a)) / (max - mean(a)))
    keep <- a < max
    put(paste0(key, ".g_mean"), mean((z[keep] - a[keep]) / (max - a[keep])))
  }
}
# Level 2: the 20-point knowledge test.
paired("learning.1", d$pre_test, d$post_test, max = 20)
# Level 2 as two groups (the before and after scores of everyone, as if from two unlinked administrations): Welch.
t <- t.test(d$post_test, d$pre_test)
key <- "learning.groups"
put(paste0(key, ".n"), 2 * nrow(d)); put(paste0(key, ".pre.mean"), mean(d$pre_test)); put(paste0(key, ".post.mean"), mean(d$post_test))
put(paste0(key, ".gain"), mean(d$post_test) - mean(d$pre_test)); put(paste0(key, ".t"), t$statistic); put(paste0(key, ".df"), t$parameter)
put(paste0(key, ".p"), t$p.value); put(paste0(key, ".ci.lower"), t$conf.int[1]); put(paste0(key, ".ci.upper"), t$conf.int[2])
put(paste0(key, ".d"), (mean(d$post_test) - mean(d$pre_test)) / sqrt((var(d$pre_test) + var(d$post_test)) / 2))
put(paste0(key, ".hake"), (mean(d$post_test) - mean(d$pre_test)) / (20 - mean(d$pre_test)))
# Level 3: supervisor ratings before and at three months (six not rated at follow-up).
paired("behavior.1", d$behav_pre, d$behav_post)
# Level 4: errors per month, before and after, and the after figure on its own.
paired("results.1", d$errors_pre, d$errors_post)
put("results.item.1.n", sum(!is.na(d$errors_post))); put("results.item.1.mean", mean(d$errors_post, na.rm = TRUE))
put("results.item.1.sd", sd(d$errors_post, na.rm = TRUE))

lines <- sprintf('  "%s": %s', names(v), sprintf("%.17g", unlist(v)))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, '",'), paste(lines, collapse = ",\n"), "}"), "training_reference.json")
