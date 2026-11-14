# Meta-Analysis

Combines the results of several studies into one overall effect, estimates how much the studies disagree (heterogeneity), and checks for small-study effects. Each row of the data is one study (or one effect).

## When to use it
- Summarizing a literature: an average effect, and how much it varies.
- Meta-regression, three-level models, or a Bayesian model when there are few studies.

## How to run it
Choose **Analyze › Meta-Analysis** and pick the **Effect size**; the slots follow it:
- Already computed: **Effect size** with **Sampling variance** or **Standard error**.
- Standardized mean difference (Hedges' g) or Mean difference: each group's mean, SD and size.
- Odds ratio, Risk ratio or Risk difference: each group's events and size.
- Correlation (Fisher's z): **Correlation** and **Sample size**.

Optionally add **Study labels**, **Moderators** for meta-regression, and a **Cluster** for several effects per study or sample.

## Options
- **Model**: Random effects by REML (default), DerSimonian-Laird, Paule-Mandel, maximum likelihood, Hedges or Sidik-Jonkman; Fixed effect; or Bayesian random effects (no moderators).
- Bayesian: **Prior for τ (heterogeneity)** (Half-normal by default, Half-Cauchy, or Uniform), **Scale of the τ prior** (0.5), and an optional normal prior for μ.
- **Knapp-Hartung adjustment**: t tests, recommended with few studies; off.
- **Small-study effects: Egger's test and trim and fill**: on.
- **Leave-one-out analysis**: off.
- **Forest plot** and **Funnel plot**: both on.
- **Confidence interval (%)**: 95.

## What you get
- The pooled effect with its interval and test, and for random effects a prediction interval: where a new study's true effect would likely fall.
- Heterogeneity: Q, τ² and τ, I² and H², with intervals. I² is the share of variation due to real differences between studies.
- Moderator coefficients, or variance components for each level.
- Egger's test (a small p suggests funnel asymmetry) and trim and fill's adjusted estimate.
- The forest and funnel plots. Odds and risk ratios are analyzed on the log scale.
- Bayesian: posterior summaries for μ, τ and each study.

## Checked against
R's metafor (escalc, rma.uni, rma.mv) and bayesmeta.
