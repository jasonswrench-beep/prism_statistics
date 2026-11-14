# Writes preview.csv, the dataset behind the preview analyses' reference tests (make_preview_reference.R).
# Run from this folder: Rscript make_preview_data.R
# Coding: cond 1 = Lecture, 2 = Discussion, 3 = Online, 9 = user-missing; sex 1 = Female, 2 = Male;
# smoker 1 = No, 2 = Yes; pre and post -99 = user-missing. region is text.
set.seed(20261111)
n <- 60
cond <- rep(1:3, length.out = n)
sex <- sample(1:2, n, replace = TRUE)
smoker <- ifelse(runif(n) < ifelse(sex == 1, 0.25, 0.5), 2, 1)
region <- sample(c("North", "South", "East"), n, replace = TRUE, prob = c(0.4, 0.35, 0.25))
pre <- round(rnorm(n, 50, 8), 1)
post <- round(pre + rnorm(n, 2.5, 4), 1)
hours <- round(runif(n, 1, 20), 1)
score <- round(55 + 0.8 * hours + c(0, 4, -3)[cond] + rnorm(n, 0, 6), 1)
cond[7] <- 9
cond[18] <- NA
pre[11] <- -99
post[23] <- NA
score[30] <- NA
hours[41] <- NA
d <- data.frame(id = 1:n, cond, sex, smoker, region, pre, post, hours, score)
write.csv(d, "preview.csv", row.names = FALSE, na = "", quote = FALSE)
