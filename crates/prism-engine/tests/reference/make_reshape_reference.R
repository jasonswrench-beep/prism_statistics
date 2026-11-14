# Reference results for the data tools (prism-engine/tests/reshape_reference.rs): append, merge, aggregate and
# restructure, computed independently in base R and written as the datasets PrismStatistics must produce.
# Run from this folder: Rscript make_reshape_reference.R
set.seed(11)
n <- 12
people <- data.frame(id = 1:n, group = c("b", "a", "c", "a", NA, "b", "c", "a", "b", "d", "a", "c"),
                     age = round(runif(n, 18, 60)), score1 = round(rnorm(n, 50, 10), 1),
                     score2 = round(rnorm(n, 52, 10), 1), score3 = round(rnorm(n, 55, 10), 1))
people$score2[c(3, 8)] <- NA; people$score1[5] <- NA
extra <- data.frame(id = 13:16, age = c(33, 41, 29, 50), group = c("a", "b", "e", NA), score1 = c(48.5, NA, 61.2, 55), newvar = c(1, 2, 3, 4))
lookup <- data.frame(group = c("a", "b", "c", "z"), region = c("North", "South", "East", "West"), weight = c(1.5, 2, 0.5, 9))
visits <- data.frame(id = c(1, 1, 2, 4, 4, 4, 7, 20), visit = c(1, 2, 1, 1, 2, 3, 1, 1), bp = c(120, 118, 130, 125, 122, 119, 140, 135))
w <- function(x, f) write.csv(x, f, row.names = FALSE, na = "")
w(people, "people.csv"); w(extra, "extra.csv"); w(lookup, "lookup.csv"); w(visits, "visits.csv")

# Append (variables in either file; a source variable).
a <- people
for (v in setdiff(names(extra), names(a))) a[[v]] <- NA
e <- extra; for (v in setdiff(names(people), names(e))) e[[v]] <- NA
app <- rbind(a, e[names(a)]); app$source <- rep(1:2, c(n, nrow(extra)))
w(app, "exp_append.csv")

# Merge with the lookup on group: left, inner, all. A missing key matches nothing.
m <- match(people$group, lookup$group, incomparables = NA)
left <- cbind(people, lookup[m, c("region", "weight")]); rownames(left) <- NULL
w(left, "exp_merge_left.csv")
w(left[!is.na(m), ], "exp_merge_inner.csv")
un <- lookup[!(lookup$group %in% people$group), ]
all <- rbind(left, data.frame(id = NA, group = un$group, age = NA, score1 = NA, score2 = NA, score3 = NA, region = un$region, weight = un$weight))
w(all, "exp_merge_all.csv")
# One to many: each person repeated for each of their visits.
rows <- do.call(rbind, lapply(seq_len(n), function(i) {
  hit <- which(visits$id == people$id[i])
  if (length(hit)) cbind(people[rep(i, length(hit)), ], visits[hit, c("visit", "bp")]) else cbind(people[i, ], visit = NA, bp = NA)
}))
rownames(rows) <- NULL
w(rows, "exp_merge_many.csv")

# Aggregate by group (groups sorted, the missing group last).
g <- factor(people$group, levels = c(sort(unique(na.omit(people$group))), NA), exclude = NULL)
fs <- list(mean = function(v) if (all(is.na(v))) NA else mean(v, na.rm = TRUE), sum = function(v) if (all(is.na(v))) NA else sum(v, na.rm = TRUE),
           median = function(v) if (all(is.na(v))) NA else median(v, na.rm = TRUE), min = function(v) if (all(is.na(v))) NA else min(v, na.rm = TRUE),
           max = function(v) if (all(is.na(v))) NA else max(v, na.rm = TRUE), sd = function(v) if (sum(!is.na(v)) < 2) NA else sd(v, na.rm = TRUE),
           n = function(v) sum(!is.na(v)), nmiss = function(v) sum(is.na(v)), cases = length,
           first = function(v) if (all(is.na(v))) NA else v[!is.na(v)][1], last = function(v) if (all(is.na(v))) NA else rev(v[!is.na(v)])[1])
agg <- data.frame(group = levels(g))
for (f in names(fs)) agg[[paste0("s2_", f)]] <- as.numeric(tapply(people$score2, g, fs[[f]]))
w(agg, "exp_aggregate.csv")
add <- people
add$m1 <- ave(people$score1, g, FUN = fs$mean); add$c <- ave(people$score1, g, FUN = length)
w(add, "exp_aggregate_add.csv")

# Wide to long: score1..score3 into score, case by case, with index 1..3.
long <- do.call(rbind, lapply(seq_len(n), function(i) data.frame(id = people$id[i], group = people$group[i], age = people$age[i], index = 1:3, score = unlist(people[i, c("score1", "score2", "score3")]))))
rownames(long) <- NULL
w(long, "exp_long.csv")
w(long[!is.na(long$score), ], "exp_long_drop.csv")
# Long to wide: back again, sorted by id.
wide <- data.frame(id = people$id, score_1 = people$score1, score_2 = people$score2, score_3 = people$score3)
w(wide, "exp_wide.csv")
