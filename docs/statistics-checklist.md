# Statistics Build Checklist

What is still to be built, one line per piece, ticked as each is merged. A line is ticked only when the feature has its dialog, APA text, checks, MCP tool and R export, and its numbers match R in the test suite (plus the product owner's reference packages where noted). Details and reasons are in [stage0/12-statistics-roadmap.md](stage0/12-statistics-roadmap.md).

Last updated: 2026-11-13.

## Already built

- [x] Frequencies, Descriptives, Crosstabs with chi-square
- [x] One-sample, paired and independent t tests
- [x] One-way ANOVA with Welch, Tukey and Games-Howell
- [x] Pearson, Spearman and Kendall correlation
- [x] Mann-Whitney, Wilcoxon, Kruskal-Wallis with Dunn
- [x] Linear regression (dummy-coded factors, diagnostics)
- [x] Factorial ANOVA, 2–3 factors, Type III (PR #19)
- [x] Binary logistic regression (PR #19)
- [x] Cronbach's alpha with item statistics (PR #19)
- [x] Fisher's exact test (2 × 2) and McNemar (PR #19)
- [x] Friedman (PR #19)
- [x] Partial and canonical correlation (PR #19)
- [x] Power analysis (PR #20)
- [x] Charts: histogram, box plot, bar, scatter, normal Q-Q (PR #23)
- [x] Chi-square goodness of fit (PR #26)
- [x] Binomial, runs, sign, median and two-sample Kolmogorov-Smirnov tests (PR #26)
- [x] R packages for validation installed (lavaan, seminr, psych, car, emmeans, afex, lme4 and others)

## Part A. Finishing the 1.0 catalog

### Crosstabs and categorical
- [x] A3a Relative risk and odds ratio with intervals (2 × 2)
- [x] A3b Ordinal association: gamma, tau-b, tau-c, Somers' *d*
- [x] A3c Cochran-Mantel-Haenszel test across a layer variable
- [x] A2b Cochran's Q (k related yes/no measures)

### Agreement
- [x] A4a Cohen's kappa, unweighted and weighted
- [x] A4b Fleiss' kappa (more than two raters)
- [x] A4c Krippendorff's alpha
- [x] A4d Intraclass correlation (all six forms)

### Describing data
- [x] A5a Explore: descriptives by group, percentiles, skewness and kurtosis with SEs
- [x] A5b Normality: Shapiro-Wilk, and Kolmogorov-Smirnov with Lilliefors' correction
- [x] A5c Explore charts: box plots and Q-Q plots per group

### Regression and ANOVA
- [x] A6a Linear regression in blocks with *R*² change
- [x] A6b Interaction terms, with optional mean-centering
- [x] A6c Residual and influence plots; save predicted values and residuals as new variables
- [x] A7a Logistic regression in blocks with likelihood-ratio change
- [x] A7b Logistic interactions and the ROC curve with AUC interval
- [x] A8a Estimated marginal means with intervals (factorial ANOVA)
- [x] A8b Simple effects and post hoc tests on main effects
- [x] A8c Type II sums of squares for additive models
- [x] A8d Profile plots
- [x] A8e ANCOVA (covariates in factorial ANOVA)
- [x] A9a Planned contrasts for one-way ANOVA (custom coefficients and linear/quadratic/cubic trends, pooled and Welch tests)
- [x] A9b Bonferroni and Holm options wherever several tests run (one-way post hoc and contrasts, factorial post hoc, correlation matrix, Dunn)
- [x] A10 One-sided tests (t tests, correlations, rank tests)
- [x] A11 Diagnostic plots attached to regression and ANOVA results
- [x] A12 Curve estimation (linear, quadratic, cubic, log, exponential, power; also inverse and S)
- [x] A13 Bootstrap intervals (means, medians, correlations, coefficients; also trimmed means and SDs)

### Data features the statistics need
- [x] D1 Compute variable (expressions)
- [x] D2 Recode (into same or new variable)
- [x] D3 Scale scores: mean or sum of items, reverse scoring, minimum items answered
- [x] D4 Select cases (filter)
- [x] D5 Split file (run analyses by group)
- [x] D6 Weight cases (frequency weights)

## Part B. Research methods

- [x] B1a Repeated-measures ANOVA (one within factor), Mauchly, Greenhouse-Geisser and Huynh-Feldt
- [x] B1b Mixed ANOVA (between × within)
- [x] B2a ANCOVA with homogeneity-of-slopes check (done in A8e)
- [x] B2b MANOVA with Pillai, Wilks, Hotelling, Roy; Box's M
- [x] B3a PCA
- [x] B3b EFA: ML, principal axis, minimum residual; varimax, oblimin, promax
- [x] B3c Parallel analysis, scree plot, KMO and Bartlett's test
- [x] B3d McDonald's omega
- [x] B4a Mediation with bootstrapped indirect effects (simple and parallel)
- [x] B4b Moderation with simple slopes and Johnson-Neyman regions
- [x] B5a Ordinal logistic regression
- [x] B5b Multinomial logistic regression
- [x] B5c Poisson and negative binomial regression
- [x] B6a Robust standard errors (HC3)
- [x] B6b Robust t tests and ANOVA (trimmed means)
- [x] B7 Linear mixed models (random intercepts and slopes)
- [x] B8a Little's MCAR test and missing-data patterns
- [x] B8b Multiple imputation with pooled results
- [x] B9 Bayes factors (t tests, correlation, contingency tables, ANOVA, regression)
- [x] B10 Permutation tests
- [x] B11a Kaplan-Meier and log-rank
- [x] B11b Cox regression
- [x] B11c ROC analysis
- [x] B12 Time series basics (autocorrelation, smoothing, ARIMA)
- [x] B13 Survey design (weights, strata, clusters)

## Part C. Structural equation modeling

### C1. Engine: covariance-based SEM
- [x] C1a Model representation and the text model (lavaan syntax: `=~`, `~`, `~~`, labels, fixed values, `:=`)
- [x] C1b Path analysis with observed variables, maximum likelihood
- [x] C1c Confirmatory factor analysis (identification by marker or by fixed variance)
- [x] C1d Full latent-variable models and higher-order factors
- [x] C1e Fit indices: χ², CFI, TLI, RMSEA with 90% interval, SRMR, AIC and BIC
- [x] C1f Standardized solutions and *R*² for endogenous variables
- [x] C1g Modification indices
- [x] C1h Robust ML (Satorra-Bentler and Yuan-Bentler corrections)
- [x] C1i Missing data by full-information maximum likelihood (the FIML-corrected robust CFI/TLI/RMSEA of Zhang and Savalei are not computed; scaled ones are)
- [x] C1j Ordinal indicators (WLSMV) (all observed variables ordinal; the catML robust indices of Savalei are not computed, scaled ones are)
- [x] C1k Indirect and user-defined effects with bootstrapped intervals
- [x] C1l Multigroup models and measurement invariance (configural, metric, scalar, strict)
- [x] C1m Reliability and validity: composite reliability, AVE, HTMT

### C2. Engine: PLS-SEM
- [x] C2a Reflective and formative measurement models, path-weighting estimation
- [x] C2b Bootstrapped significance of paths and weights
- [x] C2c Quality criteria: *R*², *f*², *Q*² (as Q²predict, PLSpredict), SRMR, rho_A, composite reliability, AVE, HTMT
- [x] C2d Multigroup analysis

### C3. The visual tool
- [x] C3a Canvas: variables as rectangles, latent variables as ovals, drag and drop
- [x] C3b Drawing paths (arrows) and covariances (double-headed arrows); fixing and labeling parameters
- [x] C3c Automatic layout
- [x] C3d Diagram and text model kept in sync both ways
- [x] C3e Estimates shown on the diagram; a path selects its table row
- [x] C3f Keyboard-only model building (text model and a path list)
- [x] C3g APA output: diagram as a figure with alt text; fit, loading and path tables; template text

### C4. Validation and later extensions
- [x] C4a Every estimator and fit index checked against lavaan (CB-SEM) and seminr (PLS-SEM) (except the FIML-corrected and catML robust indices, which are not computed; SRMR and Q²predict of PLS against R computations of their definitions)
- [x] C4b Checked against the product owner's SEM packages on shared example models (a CFA, a path model, the political democracy model, two-group configural and metric CFAs, a latent growth model and the Satorra-Bentler correction; scripts and values in tests/reference/make_owner_sem_reference.*, compared in owner_sem_reference.rs. Found and fixed: free predictor covariances with fixed.x off; added the unstructured Satorra-Bentler weight that one of the packages uses)
- [x] C4c Latent growth models
- [x] C4d Multilevel SEM (two-level random-intercept models, as lavaan with cluster =)
- [x] C4e Latent class and mixture models (latent class analysis of categorical indicators, as poLCA; latent profile analysis of continuous indicators, as mclust)
- [x] C4f Item response theory (Rasch, 2PL, graded response) (item and M2 fit statistics are not computed)
- [x] C4g Bayesian SEM (NUTS with blavaan's default priors; one group, complete cases; LOOIC is not computed)

## Part D. Specialist methods

- [x] D-1 Meta-analysis (fixed and random effects, forest and funnel plots, moderators) (as metafor: effect sizes from summary data, six τ² estimators, Knapp-Hartung, Q-profile intervals, prediction interval, meta-regression, Egger and trim and fill, leave-one-out; three-level models as rma.mv; Bayesian random-effects models as bayesmeta)
- [x] D-2 Generalized estimating equations and panel models (GEE as geepack: Gaussian, binomial and Poisson, four working correlations, robust SEs, QIC; panel models as plm: pooled, fixed (individual, time, two-way), random (Swamy-Arora: individual, time and two-way), between and first differences, cluster-robust SEs, F, Hausman and Breusch-Pagan tests; Arellano-Bond difference GMM)
- [x] D-3 Predictive modeling with cross-validation (penalized regression as glmnet: lasso, ridge and elastic net, linear and logistic, k-fold CV with λ_min and λ_1se, out-of-fold performance; decision trees as rpart: regression and classification, the cross-validated complexity table and pruning, tree diagram, surrogate splits for missing values; random forests and gradient boosting, Part E)
- [x] D-4 Statistical process control and designed experiments (control charts and capability as qcc: X̄-R, X̄-S, individuals-MR, p, np, c, u, CUSUM and EWMA, run rules, Cp/Cpk/Cpm with intervals, Pp/Ppk; designs as FrF2, rsm and AlgDesign: full and minimum-aberration fractional factorials with aliases, Plackett-Burman (12–48 runs), central composite and Box-Behnken, simplex-lattice, simplex-centroid and extreme-vertices mixture designs, D- and I-optimal designs; factorial effects with Lenth’s method as BsMD; response surfaces as rsm; Scheffé mixture models)

## Part E. Deferred pieces

The parts that Parts C and D left out, in build order.

- [x] E-1 Two-way and time random-effects panel models (as plm, Swamy-Arora; two-way needs a balanced panel)
- [x] E-2 Multilevel meta-analysis (three-level models, effects nested in clusters, as metafor's rma.mv: REML or ML, I² per level, likelihood-ratio tests of each variance component)
- [x] E-3 Surrogate splits in decision trees (missing predictor values as rpart: splits on the cases with each predictor, up to five surrogates ranked by agreement, then the majority; rpart's variable importance)
- [x] E-4 Dynamic panel GMM (Arellano-Bond difference GMM as plm's pgmm: one and two steps, individual or two-way effects, collapsed or limited instruments, Windmeijer-corrected robust SEs, Sargan, AR(1), AR(2) and Wald tests; system GMM is not built)
- [x] E-5 Latent profile analysis of continuous indicators (Gaussian mixtures as mclust: EEI, VVI, EEE and VVV, EM from many seeded starts, BIC, AIC, ICL and entropy, profile plot)
- [x] E-6 Constrained mixture regions (lower limits by pseudo-components; extreme vertices of the region with upper and lower limits, with edge midpoints and the centroid, checked against cdd) and I-optimal designs (AlgDesign's I criterion)
- [x] E-7 Bayesian random-effects meta-analysis (as bayesmeta, by numerical integration: uniform, half-normal or half-Cauchy prior for τ, uniform or normal for μ; posterior mode, median, mean, SD and shortest intervals for τ, μ, a new study's effect and each study's shrinkage estimate)
- [x] E-8 Random forests and boosting (random forests as randomForest: regression and classification, out-of-bag error, permutation and impurity importance, exact for deterministic trees and within randomForest's run-to-run spread otherwise; gradient boosting as gbm: squared-error and Bernoulli loss, three-way splits for missing values, cross-validated number of trees, relative influence, exactly gbm's with bag fraction 1)
