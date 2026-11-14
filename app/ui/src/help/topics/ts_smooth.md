# Smoothing (Time Series)

Smooths a time series to show its underlying level, by simple exponential smoothing or a centered moving average, and can forecast a few steps ahead.

## When to use it
- To see the general level of a noisy series without the up-and-down of single observations.
- For short-term forecasts of a series with no clear trend or seasonal pattern.
- For a series with structure to model (autocorrelation, trend after differencing), use **ARIMA**.

## How to run it
Choose **Analyze › Time Series › Smoothing**. Move one numeric variable into **Series**. The cases are taken in row order and must be equally spaced in time; empty cells are allowed only at the start or end.

## Options
- **Smoother**: Exponential smoothing (default) or Centered moving average.
- **α (exponential)**: the smoothing weight, from 0 to 1. Left empty, it is chosen to minimize the one-step-ahead squared errors. Larger values follow recent observations more closely; smaller values smooth more.
- **Window (moving average)**: the number of observations averaged around each point; 3 by default.
- **Forecast this many steps (exponential)**: 0 by default.

## What you get
- For exponential smoothing: the α used, the sum of squared one-step errors, a table of each observation with its one-step fitted value, and a chart of the series with the smoothed line.
- With forecasting, the forecasts for the steps requested. Simple exponential smoothing forecasts a flat line at the last smoothed level.
- For the moving average: the smoothed values and the chart. A centered average has no value for the first and last few points, where the window would run past the series.

## Checked against
R's HoltWinters, without trend or seasonal terms.
