# Reference values for Sort Cases, Rank Cases, Standardize, Reverse Score and Date and Time
# (prism-engine/tests/transform_reference.rs). Run from this folder: Rscript make_transform_reference.R
# Uses only base R and preview.csv, with its user-missing codes (cond 9, pre -99) set to NA by hand.
d <- read.csv("preview.csv")
cond <- d$cond; cond[cond %in% 9] <- NA
pre <- d$pre; pre[pre %in% -99] <- NA
v <- list()
put <- function(name, x) v[[name]] <<- unname(x)

# Rank Cases: each way of ranking ties, largest first, and within groups (no rank where the group is missing).
for (t in c("average", "min", "max", "first")) put(paste0("rank.", t), rank(d$score, ties.method = t, na.last = "keep"))
put("rank.desc", rank(-d$hours, ties.method = "average", na.last = "keep"))
r <- rep(NA_real_, nrow(d))
for (g in unique(na.omit(cond))) { i <- which(cond %in% g); r[i] <- rank(pre[i], ties.method = "min", na.last = "keep") }
put("rank.pre.by.cond", r)

# Standardize: R's scale(), sample SD.
put("z.pre", as.numeric(scale(pre)))
put("z.score", as.numeric(scale(d$score)))

# Reverse Score: 1..3 becomes 3..1; the missing code 9 is kept.
put("rev.cond", ifelse(is.na(cond), d$cond, 1 + 3 - d$cond))

# Sort Cases: by cond (missing last), then score from largest; and by the text variable region.
put("sort.cond.score", d$id[order(cond, -d$score, method = "radix", na.last = TRUE)])
put("sort.region", d$id[order(d$region, method = "radix", na.last = TRUE)])

# Date and Time.
start <- c("2000-02-29", "1999-12-31 23:30", "2024-03-15 13:45", "2026-11-14", "1970-01-01")
end <- c("2026-02-28", "2026-12-30", "2024-03-16 09:15", "2027-11-13", "2027-01-01")
tm <- function(x) as.POSIXct(ifelse(grepl(":", x), x, paste(x, "00:00")), format = "%Y-%m-%d %H:%M", tz = "UTC")
s <- as.POSIXlt(tm(start))
e <- as.POSIXlt(tm(end))
put("date.year", s$year + 1900); put("date.month", s$mon + 1); put("date.day", s$mday)
put("date.weekday", (s$wday + 6) %% 7 + 1); put("date.yday", s$yday + 1); put("date.hour", s$hour); put("date.minute", s$min)
put("date.days", as.numeric(difftime(as.POSIXct(e), as.POSIXct(s), units = "days")))
months <- (e$year - s$year) * 12 + (e$mon - s$mon) - (e$mday < s$mday)
put("date.months", months); put("date.years", months %/% 12)

json <- function(x) paste0("[", paste(ifelse(is.na(x), "null", sprintf("%.17g", x)), collapse = ", "), "]")
lines <- sprintf('  "%s": %s', names(v), vapply(v, json, ""))
writeLines(c("{", paste0('  "_made_with": "', R.version.string, '",'), paste(lines, collapse = ",\n"), "}"), "transform_reference.json")
