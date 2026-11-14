# Autocorrelation (Time Series)

Shows how strongly a time series is correlated with itself at earlier lags: the autocorrelation function (ACF), the partial autocorrelation function (PACF), and a Ljung-Box test of whether the series is just random noise.

## When to use it
- To check whether a series has memory (today's value related to yesterday's), a trend or a seasonal pattern.
- To choose the orders of an ARIMA model: a PACF that cuts off after lag p suggests an AR(p) model; an ACF that cuts off after lag q suggests an MA(q) model.
- To check the residuals of a fitted model; **ARIMA** reports this for its own residuals.

## How to run it
Choose **Analyze › Time Series › Autocorrelation**. Move one numeric variable into **Series**. The cases are taken in row order and must be equally spaced in time. Empty cells are allowed only at the start or end of the series, where they are trimmed.

## Options
- **Largest lag**: how many lags to show. Left empty, it is 10·log10(n), where n is the length of the series.

## What you get
- A table of autocorrelations and partial autocorrelations at lags 1 up to the largest lag, with a mark for those beyond the ±1.96/√n bounds.
- ACF and PACF bar charts. Correlations beyond the bounds are larger than chance alone would usually produce.
- The Ljung-Box Q test over all the lags shown. A small p value means the series is not white noise: there is autocorrelation worth modeling.

The partial autocorrelation at lag k is the correlation with the value k steps back after removing the effect of the lags in between (computed by the Durbin-Levinson method).

## Checked against
R's acf, pacf and Box.test (Ljung-Box).
