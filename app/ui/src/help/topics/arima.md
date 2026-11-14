# ARIMA

Fits an ARIMA(p, d, q) model to a time series by exact maximum likelihood, tests its residuals, and can forecast ahead with intervals.

## When to use it
- To model a series whose values depend on earlier values (AR terms) or earlier shocks (MA terms).
- To forecast a series with prediction intervals.
- Use **Autocorrelation** first to see the pattern and choose the orders; for a simple level forecast, **Smoothing** may be enough.

## How to run it
Choose **Analyze › Time Series › ARIMA**. Move one numeric variable into **Series**. The cases are taken in row order and must be equally spaced in time; empty cells are allowed only at the start or end.

## Options
- **AR order (p)**: the number of autoregressive terms; 1 by default.
- **Differences (d)**: how many times to difference the series before fitting, to remove a trend; 0 by default.
- **MA order (q)**: the number of moving-average terms; 0 by default.
- **Include a mean (when d = 0)**: on by default.
- **Forecast this many steps**: 0 by default.
- **Confidence interval (%)**: 95, for the coefficients and the forecasts.

## What you get
- A coefficients table: each AR and MA coefficient and the mean, with standard errors and z tests.
- Beneath it, the residual variance (σ²), the log-likelihood and AIC. When comparing models of the same series with the same d, a smaller AIC is better.
- A residual check: the Ljung-Box test of the residuals. A large p value means no autocorrelation is left, which is what a good model leaves behind; a small one is flagged for review.
- With forecasting, a table of forecasts with their standard errors and intervals, which widen the further ahead you go.

## Checked against
R's arima (method "ML") and predict.
