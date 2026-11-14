# Writes scale.csv, invented data for the reliability, Friedman and McNemar reference tests (make_more_reference.R).
# Run from this folder: Rscript make_scale_data.R
# i1 to i5 are 1-5 agreement items on one scale; i4 is worded the other way round. 9 is user-missing on the items.
# before and after are 1 = No, 2 = Yes answers to the same question asked twice.
set.seed(20261112)
n <- 80
trait <- rnorm(n)
item <- function(load, rev = FALSE) {
  x <- load * trait + rnorm(n, 0, 0.8)
  if (rev) x <- -x
  as.integer(cut(x, c(-Inf, -1.2, -0.4, 0.4, 1.2, Inf)))
}
i1 <- item(0.9); i2 <- item(0.8); i3 <- item(0.7); i4 <- item(0.6, TRUE); i5 <- item(0.5)
before <- ifelse(runif(n) < 0.35, 2, 1)
after <- ifelse(before == 2, ifelse(runif(n) < 0.85, 2, 1), ifelse(runif(n) < 0.3, 2, 1))
i2[5] <- 9
i3[12] <- NA
after[20] <- NA
d <- data.frame(id = 1:n, i1, i2, i3, i4, i5, before, after)
write.csv(d, "scale.csv", row.names = FALSE, na = "", quote = FALSE)
