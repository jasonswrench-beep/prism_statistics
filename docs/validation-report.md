# Validation report

Generated 2026-11-14 by `python tools/validate.py` at commit `fa60d3a` (engine 0.1.1).
R 4.6.1.

**152 of 152 reference tests passed**.

Each suite compares the engine with values computed independently: R scripts in `crates/prism-engine/tests/reference` (re-run them with `--regenerate`), and for SEM the product owner's reference packages. Tolerances are written in each test file: exact (to about 1e-8) where the reference is deterministic, and within the spread of repeated reference runs where it depends on random draws. The core methods shared with PrismQDA (t tests, ANOVA, correlation, regression, nonparametric tests and more) are validated against R in the `prismqda-stats` crate's own suite, and the engine's unit tests (`cargo test -p prism-engine --lib`) cover each analysis module.

## Suites

| Suite | Reference | R packages | Tests |
|---|---|---|---|
| `bmeta_reference` | `make_bmeta_reference.R` | bayesmeta 3.5, metafor 5.2.1 | 2/2 |
| `boost_reference` | `make_boost_reference.R` | gbm 2.3.1 | 3/3 |
| `descriptives_reference` | `make_descriptives_reference.R` | — | 4/4 |
| `doe_reference` | `make_doe_reference.R` | AlgDesign 1.2.1.2, BsMD 2023.920, FrF2 2.3.5, rcdd 1.6.1, rsm 2.10.6 | 10/10 |
| `forest_reference` | `make_forest_reference.R` | randomForest 4.7.1.2 | 3/3 |
| `gee_reference` | `make_gee_reference.R` | MASS 7.3.65, geepack 1.3.13 | 4/4 |
| `health` | — | — | 3/3 |
| `jasp_import` | — | — | 3/3 |
| `latent_reference` | `make_latent_reference.R` | mirt 1.48, poLCA 1.6.0.2 | 3/3 |
| `lpa_reference` | `make_lpa_reference.R` | mclust 6.1.3 | 2/2 |
| `meta_reference` | `make_meta_reference.R` | metafor 5.2.1 | 7/7 |
| `more_reference` | `make_more_reference.R` | BayesFactor 0.9.12.4.8, DescTools 0.99.60, GPArotation 2026.8.2, MASS 7.3.65, WRS2 1.1.7, afex 1.5.1, car 3.1.5, coin 1.4.6, emmeans 2.0.4, interactions 1.3.0, irr 0.85, lavaan 0.7.3, lme4 2.1.0, lmerTest 3.2.1, lmtest 0.9.40, mice 3.19.0, naniar 1.1.0, nnet 7.3.20, norm 1.0.11.1, pROC 1.19.1, psych 2.6.9, sandwich 3.1.3, survey 4.5, survival 3.8.6 | 37/37 |
| `owner_sem_reference` | `make_owner_sem_reference.do`, `make_owner_sem_reference.ps1` | — | 6/6 |
| `panel_reference` | `make_panel_reference.R` | plm 2.6.7 | 4/4 |
| `pgmm_reference` | `make_pgmm_reference.R` | plm 2.6.7 | 2/2 |
| `pls_reference` | `make_pls_reference.R` | seminr 2.6.0 | 4/4 |
| `predict_reference` | `make_predict_reference.R` | MASS 7.3.65, glmnet 5.2, pROC 1.19.1 | 3/3 |
| `preview_reference` | `make_preview_reference.R` | — | 8/8 |
| `reshape_reference` | `make_reshape_reference.R` | — | 4/4 |
| `sem_reference` | `make_sem_reference.R` | lavaan 0.7.3, semTools 0.5.10 | 15/15 |
| `spc_reference` | `make_spc_reference.R` | qcc 2.7 | 5/5 |
| `statfile_reference` | `make_statfiles.R` | haven 2.5.5 | 5/5 |
| `transform_reference` | `make_transform_reference.R` | — | 4/4 |
| `tree_reference` | `make_tree_reference.R` | MASS 7.3.65, rpart 4.1.27 | 4/4 |
| `ttest_reference` | `make_ttest_reference.R` | — | 7/7 |

### bmeta_reference

Bayesian random-effects meta-analysis against bayesmeta (reference/bmeta_reference.json, made by make_bmeta_reference.R): the posterior of τ as bayesmeta reports it (exact integrals), and the posteriors of μ, a new study's effect and each study's shrinkage estimate as exact mixtures over bayesmeta's own posterior of τ and conditional moments (bayesmeta's printed summaries of these come from its DIRECT grid approximation).

- ✓ `crins_with_three_priors`
- ✓ `bcg_and_normand`

### boost_reference

Gradient boosting against gbm (reference/boost_reference.json, made by make_boost_reference.R). With bag.fraction = 1 gbm is deterministic, so every iteration's training deviance, every fitted value and the relative influences are compared: squared error with one and three splits per tree (Boston), Bernoulli (Pima), missing predictor values (airquality) and a categorical predictor (birth weight).

- ✓ `bernoulli`
- ✓ `squared_error`
- ✓ `cross_validation_chooses_the_trees`

### descriptives_reference

Frequencies and Descriptives against R (reference/descriptives_reference.json), and the participants paragraph.

- ✓ `category_percentages_add_to_100`
- ✓ `frequencies_count_like_r_and_keep_missing_apart`
- ✓ `descriptives_match_r`
- ✓ `participants_paragraph_reports_what_was_run`

### doe_reference

Designed experiments against R (reference/doe_reference.json, made by make_doe_reference.R): fractional factorials (FrF2: every setting in standard order, resolution, word-length pattern, aliases), full factorials (DoE.base), central composite and Box-Behnken designs (rsm), factorial effects with Lenth's method (BsMD) and t tests (lm), and response-surface analysis (rsm: coefficients, sequential ANOVA, lack of fit, stationary point, eigenvalues).

- ✓ `constrained_mixture_designs`
- ✓ `factorial_effects`
- ✓ `fractional_factorials`
- ✓ `full_factorial_and_response_surface_designs`
- ✓ `mixture_models`
- ✓ `i_optimal_designs`
- ✓ `response_surface`
- ✓ `mixture_designs`
- ✓ `plackett_burman_designs`
- ✓ `d_optimal_designs`

### forest_reference

Random forests against randomForest (reference/forest_reference.json, made by make_forest_reference.R). Deterministic trees (every predictor tried, every case used once) must equal randomForest's exactly. Forests depend on random draws that cannot be reproduced, so their out-of-bag error and importances must fall within the spread of ten randomForest runs.

- ✓ `deterministic_trees_match_exactly`
- ✓ `classification_forests_agree_statistically`
- ✓ `regression_forest_agrees_statistically`

### gee_reference

Generalized estimating equations against geepack (reference/gee_reference.json, made by make_gee_reference.R): coefficients, robust and model-based standard errors, Wald tests, the scale, the working correlation and QIC, for the binomial, Gaussian and Poisson families and every working correlation.

- ✓ `input_errors_are_plain`
- ✓ `count_outcome`
- ✓ `gaussian_outcome_with_waves_and_a_factor`
- ✓ `binary_outcome_every_working_correlation`

### health

Data health and paste preview: every finding on a dataset built to have each problem once, the fixes (preview, then apply), and pastes with a non-number in a numeric variable, values past the last variable and new cases.

- ✓ `paste_preview`
- ✓ `fixes_preview_then_apply`
- ✓ `every_finding_is_reported`

### jasp_import

JASP data import.  The committed test builds a small .jasp file the way JASP 0.19 lays one out (a zip holding `internal.sqlite`), with a labelled numeric category, a text category with a missing-value string, a scale variable with a title and an empty cell, and a filtered row. The second test runs on the product owner's JASP data library when it is present in `samples-private/` (never committed) and compares every file with the CSV copy beside it.

- ✓ `local_data_library_matches_its_csv_copies`
- ✓ `old_and_damaged_files_are_refused_plainly`
- ✓ `reads_values_labels_and_missing_cells`

### latent_reference

Latent class analysis against poLCA and item response models against mirt (reference/latent_reference.json, made by make_latent_reference.R from the packages' example data).

- ✓ `latent_class_analysis`
- ✓ `item_response_models`
- ✓ `bayesian_sem`

### lpa_reference

Latent profile analysis against mclust (reference/lpa_reference.json, made by make_lpa_reference.R): for every covariance structure and number of classes on iris, Old Faithful and its eruption times alone. mclust starts EM from a hierarchical clustering, PrismStatistics from many seeded starts, so the test is that our log-likelihood is at least mclust's, and where both reach the same maximum, that the parameters, BIC and ICL agree.

- ✓ `iris_every_structure`
- ✓ `faithful_and_one_indicator`

### meta_reference

Meta-analysis against metafor (reference/meta_reference.json, made by make_meta_reference.R): effect sizes and variances, every τ² estimator, the Knapp-Hartung adjustment, meta-regression, heterogeneity and its Q-profile intervals, prediction intervals, Egger's test, trim and fill and leave-one-out.

- ✓ `input_errors_are_plain`
- ✓ `generic_effects_from_standard_errors`
- ✓ `odds_ratios_with_bias_and_leave_one_out`
- ✓ `means_and_correlations`
- ✓ `meta_regression`
- ✓ `every_tau2_estimator`
- ✓ `three_level_models`

### more_reference

The analyses added after the preview, against R (reference/more_reference.json, made by make_more_reference.R from preview.csv and scale.csv). As in preview_reference.rs, the data have empty cells and user-missing codes, and R gets the same exclusions written out by hand.

- ✓ `classic_nonparametric_tests`
- ✓ `canonical_correlation`
- ✓ `cochran_q`
- ✓ `crosstabs_risk_ordinal_and_layers`
- ✓ `curve_estimation`
- ✓ `data_changes`
- ✓ `explore_with_normality`
- ✓ `bayes_factor_formulas`
- ✓ `factorial_anova_type_iii`
- ✓ `factorial_follow_ups_and_ancova`
- ✓ `fisher_exact_in_crosstabs`
- ✓ `friedman_test`
- ✓ `hierarchical_regression_with_a_centered_interaction`
- ✓ `bootstrap_intervals`
- ✓ `logistic_blocks_and_roc`
- ✓ `logistic_regression`
- ✓ `mcnemar_test`
- ✓ `mediation_and_moderation`
- ✓ `linear_mixed_models`
- ✓ `noncentral_f_against_r`
- ✓ `one_sided_tests`
- ✓ `missing_patterns_and_multiple_imputation`
- ✓ `one_way_manova`
- ✓ `ordinal_multinomial_and_count_regression`
- ✓ `oneway_contrasts_and_adjusted_pairs`
- ✓ `partial_correlation`
- ✓ `power_and_sample_size`
- ✓ `rater_agreement`
- ✓ `repeated_measures_and_mixed_anova`
- ✓ `robust_errors_and_trimmed_means`
- ✓ `select_weight_and_split_cases`
- ✓ `survey_design`
- ✓ `survival_and_roc`
- ✓ `permutation_tests`
- ✓ `time_series`
- ✓ `cronbach_alpha`
- ✓ `factor_analysis`

### owner_sem_reference

Structural equation models against the product owner's two SEM reference packages (checklist C4b), on shared example models: a three-factor CFA, a path model, the political democracy model, configural and metric two-group CFAs, a latent growth model and the Satorra-Bentler correction.  - The command-driven package: reference/owner_sem_cd.json, made by make_owner_sem_reference.do in its batch mode. It always models means, so PrismStatistics is run with a mean structure; standard errors are from the expected information (as PrismStatistics'). - The graphical package: reference/owner_sem_gr.json, made by make_owner_sem_reference.ps1 through its scripting engine with maximum-likelihood moments and without a mean structure (except in the two-group and growth models). It reports χ² as (N − G)·F rather than N·F, and its standard errors are from an information matrix scaled by N − G rather than N; both are converted here.  Each package names parameters its own way; the functions below map the names to PrismStatistics' labels.

- ✓ `latent_growth_model`
- ✓ `path_model`
- ✓ `confirmatory_factor_analysis`
- ✓ `political_democracy_model`
- ✓ `satorra_bentler_correction`
- ✓ `two_group_models`

### panel_reference

Linear panel models against plm (reference/panel_reference.json, made by make_panel_reference.R): pooled, fixed effects (individual, time, two-way), random effects (Swamy-Arora), between and first differences, conventional and cluster-robust standard errors, R², the overall test, the variance components and the F, Hausman and Breusch-Pagan tests, on balanced (Grunfeld, Produc) and unbalanced (EmplUK) panels.

- ✓ `balanced_panel_with_more_predictors`
- ✓ `balanced_panel_every_model`
- ✓ `unbalanced_panel`
- ✓ `two_way_and_time_random_effects`

### pgmm_reference

Dynamic panel GMM against plm's pgmm (reference/pgmm_reference.json, made by make_pgmm_reference.R): Arellano and Bond's employment equation (one and two steps, individual and two-way effects, collapsed instruments, limited lags) and Grunfeld (where plm falls back on generalized inverses): coefficients, robust (Windmeijer) and conventional standard errors, Sargan, AR(1), AR(2) and Wald tests.

- ✓ `arellano_bond`
- ✓ `generalized_inverse_fallback`

### pls_reference

PLS-SEM against seminr (reference/pls_reference.json, made by make_pls_reference.R from seminr's mobi data): path coefficients, R², loadings, weights, reliability, HTMT, f², total effects, VIF, the factorial scheme, and each group of a two-group split; SRMR and Q²predict against independent R computations of their definitions.

- ✓ `model_errors`
- ✓ `pls_path_model`
- ✓ `factorial_scheme_and_groups`
- ✓ `bootstrap`

### predict_reference

Penalized regression with cross-validation against glmnet (reference/predict_reference.json, made by make_predict_reference.R): the λ sequence and where it stops, coefficients along the path, the CV error and its standard error at every λ (same folds), λ_min and λ_1se with their coefficients, and the out-of-fold performance.

- ✓ `logistic_lasso_and_ridge`
- ✓ `random_folds_are_reproducible`
- ✓ `linear_lasso_ridge_and_elastic_net`

### preview_reference

The preview analyses against R (reference/preview_reference.json, made by make_preview_reference.R from preview.csv). The data have an empty cell or a user-missing code in most variables, and R gets the same exclusions written out by hand, so these tests check the case handling as well as the numbers.

- ✓ `failures_say_why_and_never_interpret`
- ✓ `correlations`
- ✓ `crosstabs_and_chi_square`
- ✓ `linear_regression`
- ✓ `one_sample_t`
- ✓ `rank_tests`
- ✓ `paired_t`
- ✓ `one_way_anova_with_post_hoc`

### reshape_reference

The data tools against base R (reference/make_reshape_reference.R writes the inputs and the expected datasets): append with a source variable, merges (left, inner, all, one-to-many), aggregate (eleven summaries, and added to cases), and restructuring wide to long (with and without dropping missing rows) and back to wide.

- ✓ `append_cases`
- ✓ `aggregate`
- ✓ `restructure`
- ✓ `merges`

### sem_reference

Structural equation models against lavaan (reference/sem_reference.json, made by make_sem_reference.R): every parameter estimate, standard error, standardized estimate and its standard error, fit measure, R² and the chosen modification indices.

- ✓ `full_structural_model`
- ✓ `higher_order_factor`
- ✓ `latent_growth_curves`
- ✓ `confirmatory_factor_analysis`
- ✓ `model_errors_are_plain`
- ✓ `multigroup_models`
- ✓ `full_information_maximum_likelihood`
- ✓ `path_analysis_with_fixed_x`
- ✓ `ordinal_indicators_by_wlsmv`
- ✓ `path_diagram_figure`
- ✓ `reliability_and_validity`
- ✓ `robust_maximum_likelihood`
- ✓ `measurement_invariance`
- ✓ `two_level_models`
- ✓ `bootstrap_intervals`

### spc_reference

Control charts and process capability against qcc 2.7 (reference/spc_reference.json, made by make_spc_reference.R): every plotted statistic and limit, the center lines, σ, the points flagged beyond the limits and in runs, and the capability indices with their intervals.

- ✓ `individuals_and_moving_range`
- ✓ `capability`
- ✓ `attribute_charts`
- ✓ `cusum_and_ewma`
- ✓ `xbar_r_and_s_charts`

### statfile_reference

Imports of .sav, .dta and .sas7bdat files against R's haven package (reference/statfiles_reference.json, made with the fixtures by make_statfiles.R). Every format must give the same values; the .sav file also carries a user-missing code and a missing range, which must arrive as missing-value settings rather than as empty cells.

- ✓ `damaged_files_are_refused_plainly`
- ✓ `sas_catalog_labels_attach_by_format_name`
- ✓ `dta_matches_haven`
- ✓ `sas7bdat_matches_haven`
- ✓ `sav_matches_haven_and_keeps_missing_settings`

### transform_reference

Sort Cases, Rank Cases, Standardize, Reverse Score and Date and Time against R (reference/transform_reference.json, made by make_transform_reference.R from preview.csv). The data have user-missing codes, which R gets as NA.

- ✓ `z_scores_and_reverse_scores_match_r`
- ✓ `sorting_matches_r`
- ✓ `dates_match_r`
- ✓ `ranks_match_r`

### tree_reference

Decision trees against rpart (reference/tree_reference.json, made by make_tree_reference.R): the complexity table with its cross-validated error and standard error (same folds), every node of the tree (size, prediction, risk, split point) and the trees pruned at the minimum CV error and by the one-SE rule, for regression and classification (two and three classes), with numeric and categorical predictors.

- ✓ `classification_trees`
- ✓ `categorical_predictor`
- ✓ `missing_values_and_surrogates`
- ✓ `regression_tree`

### ttest_reference

The independent-samples t test against R (reference/ttest_reference.json, made by make_ttest_reference.R).  The data include every kind of case the test must leave out: an empty grouping cell, a user-missing group code, a third group, an empty score and a user-missing score. R gets the same exclusions written out by hand, so the test checks the case handling as well as the numbers.

- ✓ `small_groups_are_flagged`
- ✓ `student_at_90_percent_matches_r`
- ✓ `every_excluded_case_has_one_reason`
- ✓ `too_few_cases_fails_without_numbers`
- ✓ `template_paragraph_reads_correctly`
- ✓ `swapping_groups_flips_the_sign_only`
- ✓ `welch_matches_r`

## Sign-off

The product owner signs each suite in `docs/validation-signoff.md` after reviewing its comparison. This script adds a row there for any new suite and never changes a signed row.
