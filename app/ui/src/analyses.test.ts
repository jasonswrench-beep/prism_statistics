import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ANALYSES, restore } from './analyses';

// The arguments each dialog would send, as recorded in a project. Reopening a result (Settings…) must give a dialog that
// sends exactly these again.
const SAMPLES: Record<string, Record<string, unknown>> = {
  crosstabs: { row: 'sex', column: 'smoker', percentages: ['row', 'total'], chi_square: true, continuity: false, expected: true, residuals: false, fisher: true, risk: true, ordinal: false, layer: 'region', bayes: true, sampling: 'rows' },
  ttest_one_sample: { test_variables: ['score', 'hours'], test_value: 60, alternative: 'greater', bayes: true, level: 0.9 },
  ttest_paired: { pairs: [['pre', 'post']], alternative: 'less', bayes: false, level: 0.95 },
  anova_oneway: { dependent: ['score'], factor: 'cond', estimator: 'welch', post_hoc: 'games_howell', contrasts: [[1, -0.5, -0.5], [0, 1, -1]], polynomial: true, contrast_adjust: 'holm', plots: true, bayes: true, level: 0.99 },
  correlation: { variables: ['pre', 'post', 'score'], coefficients: ['pearson', 'kendall'], missing: 'pairwise', alternative: 'two_sided', adjust: 'holm', bayes: true, level: 0.95 },
  regression: { dependent: 'score', covariates: ['hours'], factors: [{ name: 'cond', reference: '2' }], level: 0.95, blocks: [{ covariates: ['pre'], factors: [{ name: 'sex', reference: null }] }], interactions: [['hours', 'cond']], center: true, plots: true, robust: 'hc3', bayes: false },
  mann_whitney: { test_variables: ['score'], grouping_variable: 'sex', groups: [1, 2], alternative: 'less' },
  wilcoxon: { pairs: [['pre', 'post'], ['hours', 'score']], alternative: 'greater' },
  kruskal: { dependent: ['score'], factor: 'cond', post_hoc: 'dunn', dunn_adjust: 'bonferroni' },
  partial_correlation: { variables: ['hours', 'score'], controls: ['pre'], zero_order: false, level: 0.95 },
  mcnemar: { pairs: [['before', 'after']] },
  friedman: { variables: ['i1', 'i2', 'i3'] },
  anova_factorial: { dependent: 'score', factors: ['cond', 'sex'], covariates: ['pre'], interactions: true, ss_type: '2', emmeans: true, post_hoc: 'tukey', simple_effects: true, plots: false, level: 0.95 },
  sem: { model: 'visual =~ x1 + x2 + x3\ntextual =~ x4 + x5 + x6', std_lv: false, meanstructure: true, growth: true, fixed_x: true, estimator: 'mlr', h1_information: 'unstructured', missing: 'fiml', group: 'school', cluster: null, group_equal: ['loadings', 'intercepts'], group_partial: ['x3 ~1'], reliability: false, bootstrap: 1000, bias_corrected: true, seed: 7, chains: 2, burnin: 400, samples: 800, modindices: false, level: 0.9 },
  forest: { dependent: 'medv', predictors: ['rm', 'lstat'], kind: 'regression', n_trees: 200, mtry: 1, nodesize: null, importance: false, seed: 4 },
  boost: { dependent: 'medv', predictors: ['rm', 'lstat'], fold_variable: 'fold', distribution: 'gaussian', n_trees: 300, depth: 2, shrinkage: 0.05, bag_fraction: 1, min_obs: 10, cv_folds: 5, seed: 4 },
  lpa: { indicators: ['eruptions', 'waiting'], min_classes: 2, max_classes: 4, models: ['EEI', 'VVV'], starts: 20, seed: 7 },
  lca: { variables: ['a', 'b', 'c'], classes: 3, starts: 40, seed: 5, compare: true },
  doe_effects: { response: 'rate', factors: ['A', 'B', 'C', 'D'], order: 2, level: 0.9 },
  doe_mixture: { response: 'elongation', components: ['x1', 'x2', 'x3'], model: 'special_cubic', level: 0.9 },
  rsm: { response: 'Yield', factors: ['x1', 'x2'], block: 'Block', order: 2, level: 0.95 },
  spc: { chart: 'xbar_s', measures: ['diameter'], subgroup: 'sample', nsigmas: 3, run_length: 8, lsl: 73.95, usl: 74.05, target: null },
  tree: { dependent: 'species', predictors: ['petal_length', 'petal_width'], fold_variable: null, kind: 'classification', prune: 'min', minsplit: 10, cp: 0.005, maxsurrogate: 3, maxdepth: 5, folds: 5, seed: 3 },
  penalized: { dependent: 'type', covariates: ['glu', 'bmi'], factors: [], fold_variable: 'fold', outcome: 'binary', penalty: 'elastic', alpha: 0.3, choice: 'min', folds: 5, repeats: 2, seed: 7 },
  pgmm: { dependent: 'lemp', covariates: ['lwage', 'lcap'], individual: 'firm', time: 'year', model: 'onestep', effect: 'individual', y_lags: 2, x_lags: 1, gmm_min: 2, gmm_max: 4, collapse: true, robust: false, level: 0.9 },
  panel: { dependent: 'inv', covariates: ['value', 'capital'], factors: [], individual: 'firm', time: 'year', model: 'random', effect: 'individual', robust: true, tests: false, level: 0.95 },
  gee: { dependent: 'resp', covariates: ['age'], factors: [{ name: 'smoke', reference: '1' }], cluster: 'id', wave: 'time', family: 'binomial', corstr: 'ar1', level: 0.9 },
  meta: { measure: 'or', events1: 'tpos', n1: 'n1', events2: 'cpos', n2: 'n2', study: 'label', moderators: ['ablat', 'alloc'], method: 'dl', knha: true, bias: false, leave_one_out: true, forest: true, funnel: false, level: 0.9 },
  irt: { items: ['a', 'b', 'c'], model: 'rasch', level: 0.9 },
  pls: { model: 'A =~ x1 + x2\nB <~ x3 + x4\nB ~ A', scheme: 'factorial', bootstrap: 500, seed: 9, folds: 5, group: 'g', permutations: 200, level: 0.9 },
  sem_invariance: { model: 'visual =~ x1 + x2 + x3', group: 'school', steps: ['configural', 'metric'], group_partial: [], std_lv: false, estimator: 'ml', missing: 'listwise' },
  survey_estimates: { variables: ['api00', 'sch_wide'], weight: 'pw', strata: 'stype', cluster: null, statistic: 'mean', level: 0.95 },
  survey_regression: { dependent: 'api00', covariates: ['ell', 'meals'], factors: [], weight: 'pw', strata: null, cluster: 'dnum', level: 0.95 },
  ts_acf: { variable: 'level', lags: 12 },
  ts_smooth: { variable: 'flow', method: 'exponential', alpha: null, window: 3, forecast: 4 },
  arima: { variable: 'flow', p: 0, d: 1, q: 1, mean: true, forecast: 3, level: 0.95 },
  kaplan_meier: { time: 'time', status: 'status', events: [2], group: 'sex', plot: true, level: 0.95 },
  cox_regression: { time: 'time', status: 'status', events: [2, 3], covariates: ['age'], factors: [{ name: 'sex', reference: null }], level: 0.95 },
  roc: { variables: ['score', 'hours'], state: 'smoker', positive: 2, plot: false, level: 0.9 },
  perm_two_groups: { test: 'two_groups', variables: ['score'], group: 'sex', groups: [1, 2], resamples: 5000, exact_limit: 100000, seed: 9 },
  perm_paired: { test: 'paired', variables: ['pre', 'post'], resamples: 10000, exact_limit: 100000, seed: 20260101 },
  perm_correlation: { test: 'correlation', variables: ['hours', 'score'], resamples: 10000, exact_limit: 50000, seed: 1 },
  perm_anova: { test: 'anova', variables: ['score'], group: 'cond', resamples: 20000, exact_limit: 100000, seed: 2 },
  missing_patterns: { variables: ['pre', 'post', 'score'] },
  mi_regression: { dependent: 'score', covariates: ['hours', 'pre'], auxiliary: ['post'], imputations: 30, iterations: 15, seed: 3, level: 0.95 },
  mixed_model: { dependent: 'Reaction', covariates: ['Days'], factors: [], group: 'Subject', random_slopes: ['Days'], reml: false, level: 0.95 },
  yuen: { test_variables: ['score'], grouping_variable: 'sex', groups: [1, 2], trim: 0.1, level: 0.95 },
  yuen_paired: { pairs: [['pre', 'post']], trim: 0.2, level: 0.9 },
  trimmed_anova: { dependent: ['score'], factor: 'cond', trim: 0.2 },
  ordinal_regression: { dependent: 'cond', covariates: ['hours'], factors: [{ name: 'sex', reference: '2' }], level: 0.95 },
  multinomial_regression: { dependent: 'cond', reference: '3', covariates: ['hours'], factors: [{ name: 'sex', reference: null }], level: 0.95 },
  count_regression: { dependent: 'cnt', family: 'negative_binomial', covariates: ['pre'], factors: [], exposure: 'hours', level: 0.95 },
  logistic: { dependent: 'smoker', event: '1', covariates: ['hours'], factors: [{ name: 'cond', reference: null }], level: 0.95, blocks: [{ covariates: ['pre'], factors: [] }], interactions: [['hours', 'cond']], center: false, roc: true },
  reliability: { items: ['i1', 'i2', 'i3', 'i4'], reverse: ['i4'], scale_min: 1, scale_max: 5, level: 0.95 },
  canonical: { set1: ['pre', 'post'], set2: ['hours', 'score'] },
  chisq_gof: { variable: 'region', expected: [1, 2, 2] },
  binomial: { variables: ['sex'], test_proportion: 0.5, level: 0.95 },
  runs: { variables: ['score'], cut: 'value', value: 60 },
  sign_test: { pairs: [['pre', 'post']] },
  median_test: { dependent: ['score'], factor: 'cond' },
  ks2: { test_variables: ['score'], grouping_variable: 'sex', groups: [1, 2] },
  cochran: { variables: ['before', 'after'] },
  explore: { dependent: ['score'], factor: 'cond', percentiles: true, normality: false, plots: true, level: 0.9 },
  mediation: { x: 'hours', mediators: ['pre', 'post'], y: 'score', covariates: ['sex'], resamples: 2000, seed: 7, level: 0.95 },
  moderation: { x: 'hours', w: 'pre', y: 'score', covariates: [], center: true, probe: 'mean_sd', johnson_neyman: true, plot: false, level: 0.9 },
  factor: { variables: ['i1', 'i2', 'i3', 'i4', 'i5'], extraction: 'paf', rotation: 'promax', factors: 2, parallel: true, scree: false, hide_below: 0.3 },
  manova: { dependent: ['pre', 'post', 'score'], factor: 'cond', test: 'wilks', univariate: true },
  rm_anova: { measures: ['pre', 'post', 'score'], between: 'sex', factor_name: 'phase', correction: 'huynh_feldt', post_hoc: 'holm', plots: true, level: 0.95 },
  bootstrap: { statistic: 'regression', variables: ['hours', 'pre'], dependent: 'score', coefficient: 'pearson', resamples: 1000, seed: 42, level: 0.95 },
  curve: { dependent: ['score'], independent: 'hours', models: ['linear', 'quadratic', 'power'], plot: true, level: 0.95 },
  kappa: { rater1: 'i1', rater2: 'i2', weights: 'linear', level: 0.95 },
  fleiss: { raters: ['i1', 'i2', 'i3'] },
  kripp: { raters: ['i1', 'i2', 'i3'], metric: 'ordinal' },
  icc: { raters: ['i1', 'i2', 'i3'], model: 'two_way', kind: 'consistency', unit: 'average', level: 0.95 },
  chart_histogram: { kind: 'histogram', variables: ['score'], normal: true },
  chart_boxplot: { kind: 'boxplot', variables: ['score'], group: 'cond' },
  chart_bar: { kind: 'bar', variables: ['cond'], bar_stat: 'mean', mean_of: 'score' },
  chart_scatter: { kind: 'scatter', x: 'hours', y: 'score', line: true },
  chart_qq: { kind: 'qq', variables: ['score', 'hours'] },
};

test('every analysis has a sample here', () => {
  assert.deepEqual(Object.keys(SAMPLES).sort(), Object.keys(ANALYSES).sort());
});

for (const [id, args] of Object.entries(SAMPLES)) {
  test(`${id}: reopening gives the same settings`, () => {
    const def = ANALYSES[id];
    assert.deepEqual(def.build(restore(def, args)), args);
  });
}

test('a reliability analysis without reversed items and with empty endpoints', () => {
  const def = ANALYSES.reliability;
  const args = { items: ['i1', 'i2'], reverse: [], scale_min: null, scale_max: null, level: 0.95 };
  assert.deepEqual(def.build(restore(def, args)), args);
});

test('a regression without later blocks or interactions sends empty lists', () => {
  const def = ANALYSES.regression;
  const args = { dependent: 'score', covariates: ['hours'], factors: [], level: 0.95, blocks: [], interactions: [], center: false, plots: false, robust: null, bayes: false };
  assert.deepEqual(def.build(restore(def, args)), args);
});
