// The analyses the generic dialog runs: which variables go where, the options with their defaults (05 §1, §3), and how
// the choices become the engine's arguments. Each entry is one menu item and one recorded command.

import { getSettings } from './settings';

export type Accepts = 'numeric' | 'any';

export interface RoleDef {
  key: string;
  label: string;
  hint?: string;
  /** many: a list; one: a single variable; pairs: an ordered list of (first, second). */
  kind: 'many' | 'one' | 'pairs';
  accepts: Accepts;
  /** For a grouping variable: also choose two groups by code. */
  twoGroups?: boolean;
  /** For categorical predictors: choose each one's reference category by code. */
  reference?: boolean;
  /** For a two-category outcome: choose the category modeled (by code; the last one unless changed). */
  event?: boolean;
  /** The role may be left empty. */
  optional?: boolean;
  max?: number;
  min?: number;
  /** Shown (and required) only while an option has one of these values. */
  showWhen?: { option: string; values: string[] };
}

/** The roles shown for the current options. */
export function visibleRoles(def: AnalysisDef, options: Record<string, OptionValue>): RoleDef[] {
  return def.roles.filter((r) => !r.showWhen || r.showWhen.values.includes(String(options[r.showWhen.option])));
}

export interface Choice {
  value: string;
  label: string;
  hint?: string;
}

export type OptionDef = (
  | { key: string; label: string; type: 'radio'; choices: Choice[]; default: string }
  | { key: string; label: string; type: 'checks'; choices: Choice[]; default: string[]; min?: number }
  | { key: string; label: string; type: 'number'; default: string; hint?: string }
  | { key: string; label: string; type: 'toggle'; default: boolean; hint?: string }
  | { key: string; label: string; type: 'text'; default: string; hint?: string }
  | { key: string; label: string; type: 'textarea'; default: string; hint?: string }
) & {
  /** Shown only while another option has one of these values. */
  showWhen?: { option: string; values: string[] };
};

/** Whether an option is shown for the current options. */
export function optionShown(o: OptionDef, options: Record<string, OptionValue>): boolean {
  return !o.showWhen || o.showWhen.values.includes(String(options[o.showWhen.option]));
}

export type OptionValue = string | string[] | boolean;

/** An option's starting value; the confidence level follows Tools > Settings. */
export function optionDefault(o: OptionDef): OptionValue {
  return o.key === 'level' ? String(getSettings().level) : o.default;
}

/** What the dialog collected, keyed by role and option. */
export interface Collected {
  roles: Record<string, string[]>;
  pairs: Record<string, [string, string][]>;
  groups: Record<string, [number, number]>;
  references: Record<string, Record<string, string>>;
  options: Record<string, OptionValue>;
}

export interface AnalysisDef {
  id: string;
  /** The engine command. */
  command: string;
  title: string;
  roles: RoleDef[];
  options: OptionDef[];
  build: (c: Collected) => unknown;
}

const LEVEL: OptionDef = { key: 'level', label: 'Confidence interval (%)', type: 'number', default: '95', hint: 'Tests are two-sided unless a one-sided hypothesis is chosen.' };
const level = (c: Collected) => Number(c.options.level) / 100;

/** The alternative hypothesis, in the words of the comparison: `higher` and `lower` finish "one-sided: …". */
const alternative = (higher: string, lower: string): OptionDef => ({
  key: 'alternative',
  label: 'Hypothesis',
  type: 'radio',
  choices: [
    { value: 'two_sided', label: 'Two-sided (default)' },
    { value: 'greater', label: `One-sided: ${higher}` },
    { value: 'less', label: `One-sided: ${lower}` },
  ],
  default: 'two_sided',
});

export const ANALYSES: Record<string, AnalysisDef> = {
  crosstabs: {
    id: 'crosstabs',
    command: 'crosstabs',
    title: 'Crosstabs',
    roles: [
      { key: 'row', label: 'Row', kind: 'one', accepts: 'any' },
      { key: 'column', label: 'Column', kind: 'one', accepts: 'any' },
      { key: 'layer', label: 'Layer', hint: 'optional: Cochran-Mantel-Haenszel across its categories', kind: 'one', accepts: 'any', optional: true },
    ],
    options: [
      {
        key: 'percentages',
        label: 'Cell percentages',
        type: 'checks',
        choices: [
          { value: 'row', label: 'Row', hint: 'default' },
          { value: 'column', label: 'Column' },
          { value: 'total', label: 'Total' },
        ],
        default: ['row'],
      },
      { key: 'chi_square', label: 'Chi-square test of independence', type: 'toggle', default: true, hint: 'With Cramér’s V (and phi for a 2 × 2 table).' },
      { key: 'continuity', label: 'Continuity correction (2 × 2 only)', type: 'toggle', default: false, hint: 'Off by default; stated in the output when on.' },
      { key: 'expected', label: 'Expected counts', type: 'toggle', default: false },
      { key: 'residuals', label: 'Adjusted residuals', type: 'toggle', default: false },
      { key: 'fisher', label: 'Fisher’s exact test (2 × 2 only)', type: 'toggle', default: false, hint: 'With the odds ratio; use it when expected counts are small.' },
      { key: 'risk', label: 'Risk estimates (2 × 2 only)', type: 'toggle', default: false, hint: 'Odds ratio and relative risks with 95% intervals.' },
      { key: 'bayes', label: 'Bayes factor for association', type: 'toggle', default: false, hint: 'Gunel and Dickey, as R’s BayesFactor.' },
      {
        key: 'sampling',
        label: 'How the table was sampled (for the Bayes factor)',
        type: 'radio',
        choices: [
          { value: 'joint_multinomial', label: 'Only the total fixed (default)' },
          { value: 'rows', label: 'Row totals fixed' },
          { value: 'columns', label: 'Column totals fixed' },
          { value: 'poisson', label: 'Nothing fixed (Poisson)' },
        ],
        default: 'joint_multinomial',
      },
      { key: 'ordinal', label: 'Ordinal measures', type: 'toggle', default: false, hint: 'Gamma, tau-b, tau-c and Somers’ d, for categories in a meaningful order.' },
    ],
    build: (c) => ({ row: c.roles.row[0], column: c.roles.column[0], percentages: c.options.percentages, chi_square: c.options.chi_square, continuity: c.options.continuity, expected: c.options.expected, residuals: c.options.residuals, fisher: c.options.fisher, risk: c.options.risk, ordinal: c.options.ordinal, layer: c.roles.layer[0] ?? null, bayes: c.options.bayes, sampling: c.options.sampling }),
  },
  ttest_one_sample: {
    id: 'ttest_one_sample',
    command: 'ttest_one_sample',
    title: 'One-Sample T Test',
    roles: [{ key: 'test', label: 'Test Variables', hint: 'one result each', kind: 'many', accepts: 'numeric' }],
    options: [{ key: 'test_value', label: 'Test value', type: 'number', default: '0', hint: 'The mean is compared with this value.' }, alternative('the mean is above the test value', 'the mean is below the test value'), { key: 'bayes', label: 'Bayes factor', type: 'toggle', default: false, hint: 'BF₁₀ with R’s BayesFactor default prior.' }, LEVEL],
    build: (c) => ({ test_variables: c.roles.test, test_value: Number(c.options.test_value), alternative: c.options.alternative, bayes: c.options.bayes, level: level(c) }),
  },
  ttest_paired: {
    id: 'ttest_paired',
    command: 'ttest_paired',
    title: 'Paired-Samples T Test',
    roles: [{ key: 'pairs', label: 'Paired Variables', hint: 'difference = first − second', kind: 'pairs', accepts: 'numeric' }],
    options: [alternative('the first is higher', 'the first is lower'), { key: 'bayes', label: 'Bayes factor', type: 'toggle', default: false, hint: 'BF₁₀ with R’s BayesFactor default prior.' }, LEVEL],
    build: (c) => ({ pairs: c.pairs.pairs, alternative: c.options.alternative, bayes: c.options.bayes, level: level(c) }),
  },
  anova_oneway: {
    id: 'anova_oneway',
    command: 'anova_oneway',
    title: 'One-Way ANOVA',
    roles: [
      { key: 'dependent', label: 'Dependent List', hint: 'one result each', kind: 'many', accepts: 'numeric' },
      { key: 'factor', label: 'Factor', kind: 'one', accepts: 'any' },
    ],
    options: [
      {
        key: 'estimator',
        label: 'Test',
        type: 'radio',
        choices: [
          { value: 'conventional', label: 'Conventional F (default)', hint: 'assumes equal spread; Welch’s F is shown beside it for reference' },
          { value: 'welch', label: 'Welch’s F', hint: 'doesn’t assume equal spread' },
        ],
        default: 'conventional',
      },
      {
        key: 'post_hoc',
        label: 'Post hoc comparisons',
        type: 'radio',
        choices: [
          { value: 'none', label: 'None (default)' },
          { value: 'tukey', label: 'Tukey HSD', hint: 'pooled variance; pairs with the conventional F' },
          { value: 'games_howell', label: 'Games-Howell', hint: 'unequal variances; pairs with Welch’s F' },
          { value: 'bonferroni', label: 'Bonferroni', hint: 't tests on the pooled variance' },
          { value: 'holm', label: 'Holm', hint: 'like Bonferroni, never less powerful' },
          { value: 'scheffe', label: 'Scheffé', hint: 'the most cautious' },
        ],
        default: 'none',
      },
      { key: 'contrasts', label: 'Planned contrasts', type: 'text', default: '', hint: 'Optional: one coefficient per group in code order, adding to zero, with contrasts separated by semicolons, such as 1 -0.5 -0.5; 0 1 -1.' },
      { key: 'polynomial', label: 'Trend contrasts (linear, quadratic, cubic)', type: 'toggle', default: false, hint: 'Treats the groups as equally spaced, in code order.' },
      {
        key: 'contrast_adjust',
        label: 'Adjust the contrasts’ p values',
        type: 'radio',
        choices: [
          { value: 'none', label: 'No (default)' },
          { value: 'bonferroni', label: 'Bonferroni' },
          { value: 'holm', label: 'Holm' },
        ],
        default: 'none',
      },
      { key: 'plots', label: 'Means and residual plots', type: 'toggle', default: false },
      { key: 'bayes', label: 'Bayes factor', type: 'toggle', default: false, hint: 'BF₁₀ with R’s BayesFactor default prior.' }, LEVEL,
    ],
    build: (c) => ({ dependent: c.roles.dependent, factor: c.roles.factor[0], estimator: c.options.estimator, post_hoc: c.options.post_hoc, contrasts: contrastRows(c.options.contrasts), polynomial: c.options.polynomial, contrast_adjust: c.options.contrast_adjust, plots: c.options.plots, bayes: c.options.bayes, level: level(c) }),
  },
  correlation: {
    id: 'correlation',
    command: 'correlation',
    title: 'Bivariate Correlations',
    roles: [{ key: 'variables', label: 'Variables', hint: 'at least two', kind: 'many', accepts: 'numeric', min: 2 }],
    options: [
      {
        key: 'coefficients',
        label: 'Coefficients',
        type: 'checks',
        choices: [
          { value: 'pearson', label: 'Pearson', hint: 'default' },
          { value: 'spearman', label: 'Spearman' },
          { value: 'kendall', label: 'Kendall’s tau-b' },
        ],
        default: ['pearson'],
        min: 1,
      },
      {
        key: 'missing',
        label: 'Missing values',
        type: 'radio',
        choices: [
          { value: 'listwise', label: 'Exclude cases listwise (default)', hint: 'every coefficient uses the same cases' },
          { value: 'pairwise', label: 'Exclude cases pairwise', hint: 'each pair uses every case it can; N is shown for each' },
        ],
        default: 'listwise',
      },
      alternative('a positive correlation', 'a negative correlation'),
      {
        key: 'adjust',
        label: 'Adjust p values for the number of pairs',
        type: 'radio',
        choices: [
          { value: 'none', label: 'No (default)' },
          { value: 'bonferroni', label: 'Bonferroni' },
          { value: 'holm', label: 'Holm' },
        ],
        default: 'none',
      },
      { key: 'bayes', label: 'Bayes factor', type: 'toggle', default: false, hint: 'BF₁₀ with R’s BayesFactor default prior.' }, LEVEL,
    ],
    build: (c) => ({ variables: c.roles.variables, coefficients: c.options.coefficients, missing: c.options.missing, alternative: c.options.alternative, adjust: c.options.adjust, bayes: c.options.bayes, level: level(c) }),
  },
  regression: {
    id: 'regression',
    command: 'regression',
    title: 'Linear Regression',
    roles: [
      { key: 'dependent', label: 'Dependent', kind: 'one', accepts: 'numeric' },
      { key: 'covariates', label: 'Numeric predictors', hint: 'entered as they are', kind: 'many', accepts: 'numeric' },
      { key: 'factors', label: 'Categorical predictors', hint: 'dummy-coded', kind: 'many', accepts: 'any', reference: true },
      { key: 'covariates2', label: 'Block 2: numeric predictors', hint: 'optional: entered after the first block, tested by R² change', kind: 'many', accepts: 'numeric', optional: true },
      { key: 'factors2', label: 'Block 2: categorical predictors', hint: 'optional', kind: 'many', accepts: 'any', reference: true, optional: true },
    ],
    options: [
      { key: 'interactions', label: 'Interactions', type: 'text', default: '', hint: 'Optional: pairs of predictors in the model, such as hours*cond, hours*sex. They enter as a final block.' },
      { key: 'center', label: 'Mean-center numeric predictors in interactions', type: 'toggle', default: false },
      { key: 'plots', label: 'Residual plots', type: 'toggle', default: false, hint: 'Residuals against predicted values, and a Q-Q plot and histogram of the standardized residuals.' },
      {
        key: 'robust',
        label: 'Standard errors',
        type: 'radio',
        choices: [
          { value: 'none', label: 'Usual (default)' },
          { value: 'hc3', label: 'Robust (HC3)', hint: 'heteroskedasticity-consistent' },
          { value: 'hc1', label: 'Robust (HC1)', hint: 'HC0 scaled by n / (n − k)' },
        ],
        default: 'none',
      },
      { key: 'bayes', label: 'Bayes factor', type: 'toggle', default: false, hint: 'For the model against no predictors (numeric predictors only).' },
      LEVEL,
    ],
    build: (c) => {
      const covs2 = c.roles.covariates2 ?? [];
      const facs2 = c.roles.factors2 ?? [];
      return {
        dependent: c.roles.dependent[0],
        covariates: c.roles.covariates,
        factors: c.roles.factors.map((name) => ({ name, reference: c.references.factors?.[name] ?? null })),
        level: level(c),
        blocks: covs2.length || facs2.length ? [{ covariates: covs2, factors: facs2.map((name) => ({ name, reference: c.references.factors2?.[name] ?? null })) }] : [],
        interactions: interactionPairs(c.options.interactions),
        center: c.options.center,
        plots: c.options.plots,
        robust: c.options.robust === 'none' ? null : c.options.robust,
        bayes: c.options.bayes,
      };
    },
  },
  mann_whitney: {
    id: 'mann_whitney',
    command: 'mann_whitney',
    title: 'Mann-Whitney U Test',
    roles: [
      { key: 'test', label: 'Test Variables', hint: 'one result each', kind: 'many', accepts: 'numeric' },
      { key: 'group', label: 'Grouping Variable', kind: 'one', accepts: 'numeric', twoGroups: true },
    ],
    options: [alternative('the first group tends to be higher', 'the first group tends to be lower')],
    build: (c) => ({ test_variables: c.roles.test, grouping_variable: c.roles.group[0], groups: c.groups.group, alternative: c.options.alternative }),
  },
  wilcoxon: {
    id: 'wilcoxon',
    command: 'wilcoxon',
    title: 'Wilcoxon Signed-Rank Test',
    roles: [{ key: 'pairs', label: 'Paired Variables', hint: 'difference = first − second', kind: 'pairs', accepts: 'numeric' }],
    options: [alternative('the first tends to be higher', 'the first tends to be lower')],
    build: (c) => ({ pairs: c.pairs.pairs, alternative: c.options.alternative }),
  },
  kruskal: {
    id: 'kruskal',
    command: 'kruskal',
    title: 'Kruskal-Wallis Test',
    roles: [
      { key: 'dependent', label: 'Dependent List', hint: 'one result each', kind: 'many', accepts: 'numeric' },
      { key: 'factor', label: 'Factor', kind: 'one', accepts: 'any' },
    ],
    options: [
      {
        key: 'post_hoc',
        label: 'Post hoc comparisons',
        type: 'radio',
        choices: [
          { value: 'none', label: 'None (default)' },
          { value: 'dunn', label: 'Dunn’s test' },
        ],
        default: 'none',
      },
      {
        key: 'dunn_adjust',
        label: 'Adjust Dunn’s p values',
        type: 'radio',
        choices: [
          { value: 'holm', label: 'Holm (default)' },
          { value: 'bonferroni', label: 'Bonferroni' },
          { value: 'none', label: 'No adjustment' },
        ],
        default: 'holm',
      },
    ],
    build: (c) => ({ dependent: c.roles.dependent, factor: c.roles.factor[0], post_hoc: c.options.post_hoc, dunn_adjust: c.options.dunn_adjust }),
  },
  partial_correlation: {
    id: 'partial_correlation',
    command: 'partial_correlation',
    title: 'Partial Correlations',
    roles: [
      { key: 'variables', label: 'Variables', hint: 'at least two', kind: 'many', accepts: 'numeric', min: 2 },
      { key: 'controls', label: 'Controlling for', kind: 'many', accepts: 'numeric' },
    ],
    options: [{ key: 'zero_order', label: 'Zero-order correlations', type: 'toggle', default: true, hint: 'The ordinary correlations of the same cases, for comparison.' }, LEVEL],
    build: (c) => ({ variables: c.roles.variables, controls: c.roles.controls, zero_order: c.options.zero_order, level: level(c) }),
  },
  mcnemar: {
    id: 'mcnemar',
    command: 'mcnemar',
    title: 'McNemar Test',
    roles: [{ key: 'pairs', label: 'Paired Variables', hint: 'the same two codes in both', kind: 'pairs', accepts: 'numeric' }],
    options: [],
    build: (c) => ({ pairs: c.pairs.pairs }),
  },
  friedman: {
    id: 'friedman',
    command: 'friedman',
    title: 'Friedman Test',
    roles: [{ key: 'variables', label: 'Test Variables', hint: 'measures of the same cases', kind: 'many', accepts: 'numeric', min: 2 }],
    options: [],
    build: (c) => ({ variables: c.roles.variables }),
  },
  anova_factorial: {
    id: 'anova_factorial',
    command: 'anova_factorial',
    title: 'Factorial ANOVA',
    roles: [
      { key: 'dependent', label: 'Dependent Variable', kind: 'one', accepts: 'numeric' },
      { key: 'factors', label: 'Fixed Factors', hint: 'two or three', kind: 'many', accepts: 'any', min: 2, max: 3 },
      { key: 'covariates', label: 'Covariates', hint: 'optional: ANCOVA', kind: 'many', accepts: 'numeric', optional: true },
    ],
    options: [
      { key: 'interactions', label: 'Include the interactions', type: 'toggle', default: true, hint: 'Off for a main-effects model.' },
      {
        key: 'ss_type',
        label: 'Sums of squares',
        type: 'radio',
        choices: [
          { value: '3', label: 'Type III (default)', hint: 'each effect after all the others' },
          { value: '2', label: 'Type II', hint: 'each effect after the others it doesn’t contain; usual for main-effects models' },
        ],
        default: '3',
      },
      { key: 'emmeans', label: 'Estimated marginal means', type: 'toggle', default: true },
      {
        key: 'post_hoc',
        label: 'Post hoc comparisons of main effects',
        type: 'radio',
        choices: [
          { value: 'none', label: 'None (default)' },
          { value: 'tukey', label: 'Tukey' },
          { value: 'bonferroni', label: 'Bonferroni' },
          { value: 'holm', label: 'Holm' },
        ],
        default: 'none',
      },
      { key: 'simple_effects', label: 'Simple effects', type: 'toggle', default: false, hint: 'Each factor within each category of the other (two factors with the interaction).' },
      { key: 'plots', label: 'Profile and residual plots', type: 'toggle', default: false },
      LEVEL,
    ],
    build: (c) => ({ dependent: c.roles.dependent[0], factors: c.roles.factors, covariates: c.roles.covariates, interactions: c.options.interactions, ss_type: c.options.ss_type, emmeans: c.options.emmeans, post_hoc: c.options.post_hoc, simple_effects: c.options.simple_effects, plots: c.options.plots, level: level(c) }),
  },
  sem: {
    id: 'sem',
    command: 'sem',
    title: 'Structural Equation Model',
    roles: [
      { key: 'group', label: 'Groups', hint: 'optional: a multigroup model', kind: 'one', accepts: 'any', optional: true },
      { key: 'cluster', label: 'Clusters', hint: 'optional: a two-level model (level: 1 and level: 2)', kind: 'one', accepts: 'any', optional: true },
    ],
    options: [
      {
        key: 'model',
        label: 'Model',
        type: 'textarea',
        default: '',
        hint: 'lavaan syntax: f =~ x1 + x2 + x3 (a latent variable), y ~ x (a path), a ~~ b (a covariance), a*x (a label), 1*x (a fixed value), ab := a*b (a defined effect).',
      },
      { key: 'std_lv', label: 'Identify factors by fixing their variances at 1', type: 'toggle', default: false, hint: '(instead of each first loading)' },
      { key: 'meanstructure', label: 'Estimate intercepts and means', type: 'toggle', default: false },
      { key: 'growth', label: 'Latent growth model', type: 'toggle', default: false, hint: '(as lavaan’s growth(): observed intercepts fixed at 0, growth factor means estimated)' },
      { key: 'fixed_x', label: 'Fix observed predictors at their sample moments', type: 'toggle', default: true, hint: '(lavaan’s default)' },
      {
        key: 'estimator',
        label: 'Estimator',
        type: 'radio',
        choices: [
          { value: 'ml', label: 'Maximum likelihood (default)' },
          { value: 'mlm', label: 'Robust: Satorra-Bentler (MLM)', hint: 'robust SEs and scaled χ²; complete data' },
          { value: 'mlr', label: 'Robust: Huber-White and Yuan-Bentler (MLR)', hint: 'robust SEs and scaled χ²; works with FIML' },
          { value: 'wlsmv', label: 'Ordinal indicators (WLSMV)', hint: 'polychoric correlations; every observed variable ordinal' },
          { value: 'bayes', label: 'Bayesian (MCMC)', hint: 'blavaan’s default priors; posterior summaries, PPP, DIC and WAIC' },
        ],
        default: 'ml',
      },
      {
        key: 'h1_information',
        label: 'Satorra-Bentler weight (MLM)',
        type: 'radio',
        choices: [
          { value: 'structured', label: 'Model-implied moments (default)', hint: 'as lavaan' },
          { value: 'unstructured', label: 'Sample moments', hint: 'as some other packages compute it' },
        ],
        default: 'structured',
      },
      {
        key: 'missing',
        label: 'Missing values',
        type: 'radio',
        choices: [
          { value: 'listwise', label: 'Complete cases only (default)' },
          { value: 'fiml', label: 'Full-information ML', hint: 'uses every case; assumes missing at random' },
        ],
        default: 'listwise',
      },
      {
        key: 'group_equal',
        label: 'Held equal across groups',
        type: 'checks',
        choices: [
          { value: 'loadings', label: 'Loadings' },
          { value: 'intercepts', label: 'Intercepts' },
          { value: 'residuals', label: 'Residual variances' },
          { value: 'residual.covariances', label: 'Residual covariances' },
          { value: 'lv.variances', label: 'Factor variances' },
          { value: 'lv.covariances', label: 'Factor covariances' },
          { value: 'means', label: 'Factor means' },
          { value: 'regressions', label: 'Regressions' },
        ],
        default: [],
      },
      { key: 'group_partial', label: 'Free across groups anyway', type: 'text', default: '', hint: 'Comma-separated, such as x3 ~1, visual =~ x2.' },
      { key: 'reliability', label: 'Composite reliability, AVE and HTMT', type: 'toggle', default: true },
      { key: 'bootstrap', label: 'Bootstrap resamples', type: 'number', default: '0', hint: '0 for none; 1000 or more for intervals of indirect effects.' },
      { key: 'bias_corrected', label: 'Bias-corrected bootstrap intervals', type: 'toggle', default: false, hint: '(instead of percentile)' },
      { key: 'seed', label: 'Seed (bootstrap and MCMC)', type: 'number', default: '20260101' },
      { key: 'chains', label: 'MCMC chains (Bayesian)', type: 'number', default: '3' },
      { key: 'burnin', label: 'Warm-up draws per chain (Bayesian)', type: 'number', default: '500' },
      { key: 'samples', label: 'Kept draws per chain (Bayesian)', type: 'number', default: '1000' },
      { key: 'modindices', label: 'Modification indices', type: 'toggle', default: true },
      LEVEL,
    ],
    build: (c) => ({
      model: String(c.options.model ?? ''),
      std_lv: c.options.std_lv,
      meanstructure: c.options.meanstructure,
      growth: c.options.growth,
      fixed_x: c.options.fixed_x,
      estimator: c.options.estimator,
      h1_information: c.options.h1_information,
      missing: c.options.missing,
      group: c.roles.group[0] ?? null,
      cluster: c.roles.cluster?.[0] ?? null,
      group_equal: c.options.group_equal,
      group_partial: String(c.options.group_partial ?? '').split(',').map((s) => s.trim()).filter(Boolean),
      reliability: c.options.reliability,
      bootstrap: Number(c.options.bootstrap),
      bias_corrected: c.options.bias_corrected,
      seed: Number(c.options.seed),
      chains: Number(c.options.chains),
      burnin: Number(c.options.burnin),
      samples: Number(c.options.samples),
      modindices: c.options.modindices,
      level: level(c),
    }),
  },
  forest: {
    id: 'forest',
    command: 'forest',
    title: 'Random Forest',
    roles: [
      { key: 'dependent', label: 'Outcome', hint: 'numeric or categorical', kind: 'one', accepts: 'any' },
      { key: 'predictors', label: 'Predictors', hint: 'numeric or categorical', kind: 'many', accepts: 'any' },
    ],
    options: [
      {
        key: 'kind',
        label: 'Forest',
        type: 'radio',
        choices: [
          { value: 'auto', label: 'From the outcome (default)', hint: 'classification for a text or nominal outcome' },
          { value: 'regression', label: 'Regression' },
          { value: 'classification', label: 'Classification' },
        ],
        default: 'auto',
      },
      { key: 'n_trees', label: 'Trees', type: 'number', default: '500' },
      { key: 'mtry', label: 'Predictors tried at each split (empty: p/3 or √p)', type: 'text', default: '' },
      { key: 'nodesize', label: 'Largest node left unsplit (empty: 5 or 1)', type: 'text', default: '' },
      { key: 'importance', label: 'Permutation importance', type: 'toggle', default: true },
      { key: 'seed', label: 'Seed', type: 'number', default: '20260101' },
    ],
    build: (c) => {
      const opt = (k: string) => {
        const v = String(c.options[k] ?? '').trim();
        return v === '' ? null : Number(v);
      };
      return { dependent: c.roles.dependent[0], predictors: c.roles.predictors, kind: c.options.kind, n_trees: Number(c.options.n_trees), mtry: opt('mtry'), nodesize: opt('nodesize'), importance: c.options.importance, seed: Number(c.options.seed) };
    },
  },
  boost: {
    id: 'boost',
    command: 'boost',
    title: 'Gradient Boosting',
    roles: [
      { key: 'dependent', label: 'Outcome', hint: 'numeric, or two categories', kind: 'one', accepts: 'any' },
      { key: 'predictors', label: 'Predictors', hint: 'numeric or categorical; missing values allowed', kind: 'many', accepts: 'any' },
      { key: 'fold_variable', label: 'Fold variable', hint: 'optional: your own folds', kind: 'one', accepts: 'numeric', optional: true },
    ],
    options: [
      {
        key: 'distribution',
        label: 'Loss',
        type: 'radio',
        choices: [
          { value: 'auto', label: 'From the outcome (default)', hint: 'Bernoulli for a categorical outcome' },
          { value: 'gaussian', label: 'Squared error' },
          { value: 'bernoulli', label: 'Bernoulli (logistic)' },
        ],
        default: 'auto',
      },
      { key: 'n_trees', label: 'Trees', type: 'number', default: '100' },
      { key: 'depth', label: 'Splits per tree (interaction depth)', type: 'number', default: '1' },
      { key: 'shrinkage', label: 'Shrinkage (learning rate)', type: 'number', default: '0.1' },
      { key: 'bag_fraction', label: 'Bag fraction', type: 'number', default: '0.5', hint: '1: deterministic, as gbm with bag.fraction = 1' },
      { key: 'min_obs', label: 'Smallest node', type: 'number', default: '10' },
      { key: 'cv_folds', label: 'Cross-validation folds to choose the trees (0: none)', type: 'number', default: '5' },
      { key: 'seed', label: 'Seed', type: 'number', default: '20260101' },
    ],
    build: (c) => ({
      dependent: c.roles.dependent[0],
      predictors: c.roles.predictors,
      fold_variable: c.roles.fold_variable[0] ?? null,
      distribution: c.options.distribution,
      n_trees: Number(c.options.n_trees),
      depth: Number(c.options.depth),
      shrinkage: Number(c.options.shrinkage),
      bag_fraction: Number(c.options.bag_fraction),
      min_obs: Number(c.options.min_obs),
      cv_folds: Number(c.options.cv_folds),
      seed: Number(c.options.seed),
    }),
  },
  lpa: {
    id: 'lpa',
    command: 'lpa',
    title: 'Latent Profile Analysis',
    roles: [{ key: 'indicators', label: 'Indicators', hint: 'continuous', kind: 'many', accepts: 'numeric' }],
    options: [
      { key: 'min_classes', label: 'Fewest classes', type: 'number', default: '1' },
      { key: 'max_classes', label: 'Most classes', type: 'number', default: '6' },
      {
        key: 'models',
        label: 'Covariance structures',
        type: 'checks',
        choices: [
          { value: 'EEI', label: 'EEI', hint: 'equal variances, no covariances (the classic profile model)' },
          { value: 'VVI', label: 'VVI', hint: 'variances differ by class, no covariances' },
          { value: 'EEE', label: 'EEE', hint: 'equal variances and covariances' },
          { value: 'VVV', label: 'VVV', hint: 'variances and covariances differ by class' },
        ],
        default: ['EEI', 'VVI', 'EEE', 'VVV'],
        min: 1,
      },
      { key: 'starts', label: 'Random starts per model', type: 'number', default: '50', hint: 'More starts guard against local maxima.' },
      { key: 'seed', label: 'Seed', type: 'number', default: '20260101' },
    ],
    build: (c) => ({ indicators: c.roles.indicators, min_classes: Number(c.options.min_classes), max_classes: Number(c.options.max_classes), models: c.options.models, starts: Number(c.options.starts), seed: Number(c.options.seed) }),
  },
  lca: {
    id: 'lca',
    command: 'lca',
    title: 'Latent Class Analysis',
    roles: [{ key: 'variables', label: 'Indicators', hint: 'categorical', kind: 'many', accepts: 'any' }],
    options: [
      { key: 'classes', label: 'Number of classes', type: 'number', default: '2' },
      { key: 'compare', label: 'Also compare 1 to that many classes (BIC)', type: 'toggle', default: false },
      { key: 'starts', label: 'Random starts', type: 'number', default: '30', hint: 'More starts guard against local maxima.' },
      { key: 'seed', label: 'Seed', type: 'number', default: '20260101' },
    ],
    build: (c) => ({ variables: c.roles.variables, classes: Number(c.options.classes), starts: Number(c.options.starts), seed: Number(c.options.seed), compare: c.options.compare }),
  },
  meta: {
    id: 'meta',
    command: 'meta',
    title: 'Meta-Analysis',
    roles: [
      { key: 'study', label: 'Study labels', hint: 'optional', kind: 'one', accepts: 'any', optional: true },
      { key: 'effect', label: 'Effect size', kind: 'one', accepts: 'numeric', showWhen: { option: 'measure', values: ['generic'] } },
      { key: 'variance', label: 'Sampling variance', hint: 'or give the standard error', kind: 'one', accepts: 'numeric', optional: true, showWhen: { option: 'measure', values: ['generic'] } },
      { key: 'se', label: 'Standard error', hint: 'or give the variance', kind: 'one', accepts: 'numeric', optional: true, showWhen: { option: 'measure', values: ['generic'] } },
      { key: 'mean1', label: 'Group 1 mean', kind: 'one', accepts: 'numeric', showWhen: { option: 'measure', values: ['smd', 'md'] } },
      { key: 'sd1', label: 'Group 1 SD', kind: 'one', accepts: 'numeric', showWhen: { option: 'measure', values: ['smd', 'md'] } },
      { key: 'events1', label: 'Group 1 events', kind: 'one', accepts: 'numeric', showWhen: { option: 'measure', values: ['or', 'rr', 'rd'] } },
      { key: 'n1', label: 'Group 1 size', kind: 'one', accepts: 'numeric', showWhen: { option: 'measure', values: ['smd', 'md', 'or', 'rr', 'rd'] } },
      { key: 'mean2', label: 'Group 2 mean', kind: 'one', accepts: 'numeric', showWhen: { option: 'measure', values: ['smd', 'md'] } },
      { key: 'sd2', label: 'Group 2 SD', kind: 'one', accepts: 'numeric', showWhen: { option: 'measure', values: ['smd', 'md'] } },
      { key: 'events2', label: 'Group 2 events', kind: 'one', accepts: 'numeric', showWhen: { option: 'measure', values: ['or', 'rr', 'rd'] } },
      { key: 'n2', label: 'Group 2 size', kind: 'one', accepts: 'numeric', showWhen: { option: 'measure', values: ['smd', 'md', 'or', 'rr', 'rd'] } },
      { key: 'r', label: 'Correlation', kind: 'one', accepts: 'numeric', showWhen: { option: 'measure', values: ['zcor'] } },
      { key: 'n', label: 'Sample size', kind: 'one', accepts: 'numeric', showWhen: { option: 'measure', values: ['zcor'] } },
      { key: 'moderators', label: 'Moderators', hint: 'optional: meta-regression', kind: 'many', accepts: 'any', optional: true },
      { key: 'cluster', label: 'Cluster', hint: 'optional: several effects per study or sample (three-level model)', kind: 'one', accepts: 'any', optional: true },
    ],
    options: [
      {
        key: 'measure',
        label: 'Effect size',
        type: 'radio',
        choices: [
          { value: 'generic', label: 'Already computed', hint: 'each study’s effect and its variance or SE' },
          { value: 'smd', label: 'Standardized mean difference (Hedges’ g)', hint: 'from means, SDs and sizes' },
          { value: 'md', label: 'Mean difference', hint: 'raw units' },
          { value: 'or', label: 'Odds ratio', hint: 'from events and group sizes' },
          { value: 'rr', label: 'Risk ratio' },
          { value: 'rd', label: 'Risk difference' },
          { value: 'zcor', label: 'Correlation (Fisher’s z)', hint: 'from r and n' },
        ],
        default: 'generic',
      },
      {
        key: 'method',
        label: 'Model',
        type: 'radio',
        choices: [
          { value: 'reml', label: 'Random effects, REML (default)' },
          { value: 'dl', label: 'Random effects, DerSimonian-Laird' },
          { value: 'pm', label: 'Random effects, Paule-Mandel' },
          { value: 'ml', label: 'Random effects, maximum likelihood' },
          { value: 'he', label: 'Random effects, Hedges' },
          { value: 'sj', label: 'Random effects, Sidik-Jonkman' },
          { value: 'fe', label: 'Fixed effect', hint: 'assumes one true effect' },
          { value: 'bayes', label: 'Bayesian random effects', hint: 'posterior distributions, as bayesmeta; no moderators' },
        ],
        default: 'reml',
      },
      {
        key: 'tau_prior',
        label: 'Prior for τ (heterogeneity)',
        type: 'radio',
        choices: [
          { value: 'half_normal', label: 'Half-normal', hint: 'weakly informative; set its scale below' },
          { value: 'half_cauchy', label: 'Half-Cauchy', hint: 'heavier tail' },
          { value: 'uniform', label: 'Uniform (improper)', hint: 'bayesmeta’s default; needs three or more studies' },
        ],
        default: 'half_normal',
        showWhen: { option: 'method', values: ['bayes'] },
      },
      { key: 'tau_prior_scale', label: 'Scale of the τ prior', type: 'number', default: '0.5', hint: '0.5 suits log odds and risk ratios', showWhen: { option: 'method', values: ['bayes'] } },
      { key: 'mu_prior_mean', label: 'Normal prior for μ: mean (empty: uniform)', type: 'text', default: '', showWhen: { option: 'method', values: ['bayes'] } },
      { key: 'mu_prior_sd', label: 'Normal prior for μ: SD', type: 'text', default: '', showWhen: { option: 'method', values: ['bayes'] } },
      { key: 'knha', label: 'Knapp-Hartung adjustment', type: 'toggle', default: false, hint: 't tests; recommended with few studies', showWhen: { option: 'method', values: ['reml', 'dl', 'pm', 'ml', 'he', 'sj', 'fe'] } },
      { key: 'bias', label: 'Small-study effects: Egger’s test and trim and fill', type: 'toggle', default: true, showWhen: { option: 'method', values: ['reml', 'dl', 'pm', 'ml', 'he', 'sj', 'fe'] } },
      { key: 'leave_one_out', label: 'Leave-one-out analysis', type: 'toggle', default: false, showWhen: { option: 'method', values: ['reml', 'dl', 'pm', 'ml', 'he', 'sj', 'fe'] } },
      { key: 'forest', label: 'Forest plot', type: 'toggle', default: true },
      { key: 'funnel', label: 'Funnel plot', type: 'toggle', default: true },
      LEVEL,
    ],
    build: (c) => {
      const one = (k: string) => c.roles[k]?.[0];
      const m = String(c.options.measure);
      const out: Record<string, unknown> = { measure: m };
      const keys: Record<string, string[]> = { generic: ['effect', 'variance', 'se'], smd: ['mean1', 'sd1', 'n1', 'mean2', 'sd2', 'n2'], md: ['mean1', 'sd1', 'n1', 'mean2', 'sd2', 'n2'], or: ['events1', 'n1', 'events2', 'n2'], rr: ['events1', 'n1', 'events2', 'n2'], rd: ['events1', 'n1', 'events2', 'n2'], zcor: ['r'] };
      for (const k of keys[m] ?? []) if (one(k)) out[k] = one(k);
      if (m === 'zcor' && one('n')) out.n1 = one('n');
      if (one('study')) out.study = one('study');
      if (one('cluster')) out.cluster = one('cluster');
      out.moderators = c.roles.moderators ?? [];
      if (c.options.method === 'bayes') {
        const num = (k: string) => {
          const v = String(c.options[k] ?? '').trim();
          return v === '' ? null : Number(v);
        };
        Object.assign(out, { tau_prior: c.options.tau_prior, tau_prior_scale: Number(c.options.tau_prior_scale), mu_prior_mean: num('mu_prior_mean'), mu_prior_sd: num('mu_prior_sd') });
      }
      return { ...out, method: c.options.method, knha: c.options.knha, bias: c.options.bias, leave_one_out: c.options.leave_one_out, forest: c.options.forest, funnel: c.options.funnel, level: level(c) };
    },
  },
  irt: {
    id: 'irt',
    command: 'irt',
    title: 'Item Response Theory',
    roles: [{ key: 'items', label: 'Items', hint: 'two or more ordered categories each', kind: 'many', accepts: 'any' }],
    options: [
      {
        key: 'model',
        label: 'Model',
        type: 'radio',
        choices: [
          { value: 'graded', label: '2PL / graded response (default)', hint: 'a discrimination for each item' },
          { value: 'rasch', label: 'Rasch', hint: 'two-category items; equal discriminations' },
        ],
        default: 'graded',
      },
      LEVEL,
    ],
    build: (c) => ({ items: c.roles.items, model: c.options.model, level: level(c) }),
  },
  pls: {
    id: 'pls',
    command: 'pls',
    title: 'PLS-SEM',
    roles: [{ key: 'group', label: 'Groups', hint: 'optional: two groups to compare', kind: 'one', accepts: 'any', optional: true }],
    options: [
      { key: 'model', label: 'Model', type: 'textarea', default: '', hint: 'Blocks: f =~ x1 + x2 (mode A, reflective) or f <~ x1 + x2 (mode B, formative); paths: y ~ f + g.' },
      {
        key: 'scheme',
        label: 'Inner weighting',
        type: 'radio',
        choices: [
          { value: 'path', label: 'Path weighting (default)' },
          { value: 'factorial', label: 'Factorial' },
          { value: 'centroid', label: 'Centroid' },
        ],
        default: 'path',
      },
      { key: 'bootstrap', label: 'Bootstrap resamples', type: 'number', default: '0', hint: '0 for none; 5000 is usual for final results.' },
      { key: 'seed', label: 'Seed', type: 'number', default: '20260101' },
      { key: 'folds', label: 'Q²predict folds', type: 'number', default: '10', hint: '0 for none.' },
      { key: 'permutations', label: 'Permutations (two groups)', type: 'number', default: '1000' },
      LEVEL,
    ],
    build: (c) => ({
      model: String(c.options.model ?? ''),
      scheme: c.options.scheme,
      bootstrap: Number(c.options.bootstrap),
      seed: Number(c.options.seed),
      folds: Number(c.options.folds),
      group: c.roles.group[0] ?? null,
      permutations: Number(c.options.permutations),
      level: level(c),
    }),
  },
  sem_invariance: {
    id: 'sem_invariance',
    command: 'sem_invariance',
    title: 'Measurement Invariance',
    roles: [{ key: 'group', label: 'Groups', kind: 'one', accepts: 'any' }],
    options: [
      { key: 'model', label: 'Measurement model', type: 'textarea', default: '', hint: 'lavaan syntax, such as f1 =~ x1 + x2 + x3 on one line per factor.' },
      {
        key: 'steps',
        label: 'Steps',
        type: 'checks',
        choices: [
          { value: 'configural', label: 'Configural' },
          { value: 'metric', label: 'Metric', hint: 'equal loadings' },
          { value: 'scalar', label: 'Scalar', hint: 'and intercepts' },
          { value: 'strict', label: 'Strict', hint: 'and residual variances' },
        ],
        default: ['configural', 'metric', 'scalar', 'strict'],
        min: 2,
      },
      { key: 'group_partial', label: 'Free in every step (partial invariance)', type: 'text', default: '', hint: 'Comma-separated, such as x3 ~1.' },
      { key: 'std_lv', label: 'Identify factors by fixing their variances at 1', type: 'toggle', default: false },
      {
        key: 'estimator',
        label: 'Estimator',
        type: 'radio',
        choices: [
          { value: 'ml', label: 'Maximum likelihood (default)' },
          { value: 'mlm', label: 'Robust: Satorra-Bentler (MLM)' },
          { value: 'mlr', label: 'Robust: Huber-White and Yuan-Bentler (MLR)' },
        ],
        default: 'ml',
      },
      {
        key: 'missing',
        label: 'Missing values',
        type: 'radio',
        choices: [
          { value: 'listwise', label: 'Complete cases only (default)' },
          { value: 'fiml', label: 'Full-information ML' },
        ],
        default: 'listwise',
      },
    ],
    build: (c) => ({
      model: String(c.options.model ?? ''),
      group: c.roles.group[0],
      steps: c.options.steps,
      group_partial: String(c.options.group_partial ?? '').split(',').map((s) => s.trim()).filter(Boolean),
      std_lv: c.options.std_lv,
      estimator: c.options.estimator,
      missing: c.options.missing,
    }),
  },
  survey_estimates: {
    id: 'survey_estimates',
    command: 'survey_estimates',
    title: 'Survey Estimates',
    roles: [
      { key: 'variables', label: 'Variables', hint: 'categorical ones give proportions', kind: 'many', accepts: 'any' },
      { key: 'weight', label: 'Sampling weight', kind: 'one', accepts: 'numeric' },
      { key: 'strata', label: 'Strata', hint: 'optional', kind: 'one', accepts: 'any', optional: true },
      { key: 'cluster', label: 'Clusters (PSUs)', hint: 'optional', kind: 'one', accepts: 'any', optional: true },
    ],
    options: [
      {
        key: 'statistic',
        label: 'Estimate',
        type: 'radio',
        choices: [
          { value: 'mean', label: 'Means and proportions (default)' },
          { value: 'total', label: 'Totals' },
        ],
        default: 'mean',
      },
      LEVEL,
    ],
    build: (c) => ({ variables: c.roles.variables, weight: c.roles.weight[0], strata: c.roles.strata[0] ?? null, cluster: c.roles.cluster[0] ?? null, statistic: c.options.statistic, level: level(c) }),
  },
  survey_regression: {
    id: 'survey_regression',
    command: 'survey_regression',
    title: 'Survey Regression',
    roles: [
      { key: 'dependent', label: 'Dependent', kind: 'one', accepts: 'numeric' },
      { key: 'covariates', label: 'Numeric predictors', kind: 'many', accepts: 'numeric', optional: true },
      { key: 'factors', label: 'Categorical predictors', hint: 'dummy-coded', kind: 'many', accepts: 'any', reference: true, optional: true },
      { key: 'weight', label: 'Sampling weight', kind: 'one', accepts: 'numeric' },
      { key: 'strata', label: 'Strata', hint: 'optional', kind: 'one', accepts: 'any', optional: true },
      { key: 'cluster', label: 'Clusters (PSUs)', hint: 'optional', kind: 'one', accepts: 'any', optional: true },
    ],
    options: [LEVEL],
    build: (c) => ({ dependent: c.roles.dependent[0], covariates: c.roles.covariates, factors: c.roles.factors.map((name) => ({ name, reference: c.references.factors?.[name] ?? null })), weight: c.roles.weight[0], strata: c.roles.strata[0] ?? null, cluster: c.roles.cluster[0] ?? null, level: level(c) }),
  },
  ts_acf: {
    id: 'ts_acf',
    command: 'ts_acf',
    title: 'Autocorrelation',
    roles: [{ key: 'variable', label: 'Series', hint: 'cases in row order, equally spaced', kind: 'one', accepts: 'numeric' }],
    options: [{ key: 'lags', label: 'Largest lag', type: 'number', default: '', hint: 'Empty: 10·log10(n).' }],
    build: (c) => ({ variable: c.roles.variable[0], lags: String(c.options.lags ?? '').trim() ? Number(c.options.lags) : null }),
  },
  ts_smooth: {
    id: 'ts_smooth',
    command: 'ts_smooth',
    title: 'Smoothing',
    roles: [{ key: 'variable', label: 'Series', hint: 'cases in row order, equally spaced', kind: 'one', accepts: 'numeric' }],
    options: [
      {
        key: 'method',
        label: 'Smoother',
        type: 'radio',
        choices: [
          { value: 'exponential', label: 'Exponential smoothing (default)' },
          { value: 'moving_average', label: 'Centered moving average' },
        ],
        default: 'exponential',
      },
      { key: 'alpha', label: 'α (exponential)', type: 'number', default: '', hint: 'Empty: chosen by least squares.' },
      { key: 'window', label: 'Window (moving average)', type: 'number', default: '3' },
      { key: 'forecast', label: 'Forecast this many steps (exponential)', type: 'number', default: '0' },
    ],
    build: (c) => ({ variable: c.roles.variable[0], method: c.options.method, alpha: String(c.options.alpha ?? '').trim() ? Number(c.options.alpha) : null, window: Number(c.options.window), forecast: Number(c.options.forecast) }),
  },
  arima: {
    id: 'arima',
    command: 'arima',
    title: 'ARIMA',
    roles: [{ key: 'variable', label: 'Series', hint: 'cases in row order, equally spaced', kind: 'one', accepts: 'numeric' }],
    options: [
      { key: 'p', label: 'AR order (p)', type: 'number', default: '1' },
      { key: 'd', label: 'Differences (d)', type: 'number', default: '0' },
      { key: 'q', label: 'MA order (q)', type: 'number', default: '0' },
      { key: 'mean', label: 'Include a mean (when d = 0)', type: 'toggle', default: true },
      { key: 'forecast', label: 'Forecast this many steps', type: 'number', default: '0' },
      LEVEL,
    ],
    build: (c) => ({ variable: c.roles.variable[0], p: Number(c.options.p), d: Number(c.options.d), q: Number(c.options.q), mean: c.options.mean, forecast: Number(c.options.forecast), level: level(c) }),
  },
  kaplan_meier: {
    id: 'kaplan_meier',
    command: 'kaplan_meier',
    title: 'Kaplan-Meier',
    roles: [
      { key: 'time', label: 'Time', kind: 'one', accepts: 'numeric' },
      { key: 'status', label: 'Status', kind: 'one', accepts: 'numeric' },
      { key: 'group', label: 'Factor', hint: 'optional: compare groups by the log-rank test', kind: 'one', accepts: 'any', optional: true },
    ],
    options: [{ key: 'events', label: 'Status codes that mean the event happened', type: 'text', default: '1', hint: 'Such as 1, or 1, 2. Every other code counts as censored.' }, { key: 'plot', label: 'Survival curves', type: 'toggle', default: true }, LEVEL],
    build: (c) => ({ time: c.roles.time[0], status: c.roles.status[0], events: numberList(c.options.events) ?? [], group: c.roles.group[0] ?? null, plot: c.options.plot, level: level(c) }),
  },
  cox_regression: {
    id: 'cox_regression',
    command: 'cox_regression',
    title: 'Cox Regression',
    roles: [
      { key: 'time', label: 'Time', kind: 'one', accepts: 'numeric' },
      { key: 'status', label: 'Status', kind: 'one', accepts: 'numeric' },
      { key: 'covariates', label: 'Numeric predictors', kind: 'many', accepts: 'numeric', optional: true },
      { key: 'factors', label: 'Categorical predictors', hint: 'dummy-coded', kind: 'many', accepts: 'any', reference: true, optional: true },
    ],
    options: [{ key: 'events', label: 'Status codes that mean the event happened', type: 'text', default: '1', hint: 'Such as 1, or 1, 2. Every other code counts as censored.' }, LEVEL],
    build: (c) => ({ time: c.roles.time[0], status: c.roles.status[0], events: numberList(c.options.events) ?? [], covariates: c.roles.covariates, factors: c.roles.factors.map((name) => ({ name, reference: c.references.factors?.[name] ?? null })), level: level(c) }),
  },
  roc: {
    id: 'roc',
    command: 'roc',
    title: 'ROC Curve',
    roles: [
      { key: 'variables', label: 'Test variables', hint: 'higher values should point to the positive state', kind: 'many', accepts: 'numeric' },
      { key: 'state', label: 'State variable', kind: 'one', accepts: 'numeric' },
    ],
    options: [
      { key: 'positive', label: 'Code of the positive state', type: 'number', default: '1' },
      { key: 'plot', label: 'ROC curve', type: 'toggle', default: true },
      LEVEL,
    ],
    build: (c) => ({ variables: c.roles.variables, state: c.roles.state[0], positive: Number(c.options.positive), plot: c.options.plot, level: level(c) }),
  },
  perm_two_groups: {
    id: 'perm_two_groups',
    command: 'permutation',
    title: 'Permutation Test: Two Groups',
    roles: [
      { key: 'test', label: 'Outcome', kind: 'one', accepts: 'numeric' },
      { key: 'group', label: 'Grouping Variable', kind: 'one', accepts: 'numeric', twoGroups: true },
    ],
    options: [
      { key: 'resamples', label: 'Random rearrangements', type: 'number', default: '10000', hint: 'Used when there are too many to list them all.' },
      { key: 'exact_limit', label: 'List every rearrangement up to', type: 'number', default: '100000' },
      { key: 'seed', label: 'Seed', type: 'number', default: '20260101' },
    ],
    build: (c) => ({ test: 'two_groups', variables: c.roles.test, group: c.roles.group[0], groups: c.groups.group, resamples: Number(c.options.resamples), exact_limit: Number(c.options.exact_limit), seed: Number(c.options.seed) }),
  },
  perm_paired: {
    id: 'perm_paired',
    command: 'permutation',
    title: 'Permutation Test: Paired Measures',
    roles: [
      { key: 'first', label: 'First measure', kind: 'one', accepts: 'numeric' },
      { key: 'second', label: 'Second measure', kind: 'one', accepts: 'numeric' },
    ],
    options: [
      { key: 'resamples', label: 'Random rearrangements', type: 'number', default: '10000', hint: 'Used when there are too many to list them all.' },
      { key: 'exact_limit', label: 'List every rearrangement up to', type: 'number', default: '100000' },
      { key: 'seed', label: 'Seed', type: 'number', default: '20260101' },
    ],
    build: (c) => ({ test: 'paired', variables: [c.roles.first[0], c.roles.second[0]], resamples: Number(c.options.resamples), exact_limit: Number(c.options.exact_limit), seed: Number(c.options.seed) }),
  },
  perm_correlation: {
    id: 'perm_correlation',
    command: 'permutation',
    title: 'Permutation Test: Correlation',
    roles: [
      { key: 'first', label: 'First variable', kind: 'one', accepts: 'numeric' },
      { key: 'second', label: 'Second variable', kind: 'one', accepts: 'numeric' },
    ],
    options: [
      { key: 'resamples', label: 'Random rearrangements', type: 'number', default: '10000', hint: 'Used when there are too many to list them all.' },
      { key: 'exact_limit', label: 'List every rearrangement up to', type: 'number', default: '100000' },
      { key: 'seed', label: 'Seed', type: 'number', default: '20260101' },
    ],
    build: (c) => ({ test: 'correlation', variables: [c.roles.first[0], c.roles.second[0]], resamples: Number(c.options.resamples), exact_limit: Number(c.options.exact_limit), seed: Number(c.options.seed) }),
  },
  perm_anova: {
    id: 'perm_anova',
    command: 'permutation',
    title: 'Permutation Test: One-Way ANOVA',
    roles: [
      { key: 'test', label: 'Outcome', kind: 'one', accepts: 'numeric' },
      { key: 'factor', label: 'Factor', kind: 'one', accepts: 'any' },
    ],
    options: [
      { key: 'resamples', label: 'Random rearrangements', type: 'number', default: '10000', hint: 'Used when there are too many to list them all.' },
      { key: 'exact_limit', label: 'List every rearrangement up to', type: 'number', default: '100000' },
      { key: 'seed', label: 'Seed', type: 'number', default: '20260101' },
    ],
    build: (c) => ({ test: 'anova', variables: c.roles.test, group: c.roles.factor[0], resamples: Number(c.options.resamples), exact_limit: Number(c.options.exact_limit), seed: Number(c.options.seed) }),
  },
  missing_patterns: {
    id: 'missing_patterns',
    command: 'missing_patterns',
    title: 'Missing Data Patterns',
    roles: [{ key: 'variables', label: 'Variables', kind: 'many', accepts: 'any' }],
    options: [],
    build: (c) => ({ variables: c.roles.variables }),
  },
  mi_regression: {
    id: 'mi_regression',
    command: 'mi_regression',
    title: 'Regression with Multiple Imputation',
    roles: [
      { key: 'dependent', label: 'Dependent', kind: 'one', accepts: 'numeric' },
      { key: 'covariates', label: 'Predictors', kind: 'many', accepts: 'numeric' },
      { key: 'auxiliary', label: 'Auxiliary variables', hint: 'optional: used only to impute', kind: 'many', accepts: 'numeric', optional: true },
    ],
    options: [
      { key: 'imputations', label: 'Imputations', type: 'number', default: '20' },
      { key: 'iterations', label: 'Iterations per imputation', type: 'number', default: '10' },
      { key: 'seed', label: 'Seed', type: 'number', default: '20260101', hint: 'The same seed gives the same imputations.' },
      LEVEL,
    ],
    build: (c) => ({ dependent: c.roles.dependent[0], covariates: c.roles.covariates, auxiliary: c.roles.auxiliary, imputations: Number(c.options.imputations), iterations: Number(c.options.iterations), seed: Number(c.options.seed), level: level(c) }),
  },
  mixed_model: {
    id: 'mixed_model',
    command: 'mixed_model',
    title: 'Linear Mixed Model',
    roles: [
      { key: 'dependent', label: 'Dependent', kind: 'one', accepts: 'numeric' },
      { key: 'covariates', label: 'Numeric fixed effects', kind: 'many', accepts: 'numeric', optional: true },
      { key: 'factors', label: 'Categorical fixed effects', hint: 'dummy-coded', kind: 'many', accepts: 'any', reference: true, optional: true },
      { key: 'group', label: 'Grouping variable', hint: 'such as participant or school', kind: 'one', accepts: 'any' },
      { key: 'random_slopes', label: 'Random slopes', hint: 'optional: numeric fixed effects whose slopes vary by group', kind: 'many', accepts: 'numeric', optional: true },
    ],
    options: [
      { key: 'reml', label: 'Restricted maximum likelihood (REML)', type: 'toggle', default: true, hint: 'Off for maximum likelihood, to compare models that differ in fixed effects.' },
      LEVEL,
    ],
    build: (c) => ({
      dependent: c.roles.dependent[0],
      covariates: c.roles.covariates,
      factors: c.roles.factors.map((name) => ({ name, reference: c.references.factors?.[name] ?? null })),
      group: c.roles.group[0],
      random_slopes: c.roles.random_slopes,
      reml: c.options.reml,
      level: level(c),
    }),
  },
  yuen: {
    id: 'yuen',
    command: 'yuen',
    title: 'Yuen’s Test (Trimmed Means)',
    roles: [
      { key: 'test', label: 'Test Variables', hint: 'one result each', kind: 'many', accepts: 'numeric' },
      { key: 'group', label: 'Grouping Variable', kind: 'one', accepts: 'numeric', twoGroups: true },
    ],
    options: [{ key: 'trim', label: 'Proportion trimmed from each end', type: 'number', default: '0.2', hint: '0.2 (20%) is usual.' }, LEVEL],
    build: (c) => ({ test_variables: c.roles.test, grouping_variable: c.roles.group[0], groups: c.groups.group, trim: Number(c.options.trim), level: level(c) }),
  },
  yuen_paired: {
    id: 'yuen_paired',
    command: 'yuen_paired',
    title: 'Paired Yuen Test (Trimmed Means)',
    roles: [{ key: 'pairs', label: 'Paired Variables', hint: 'difference = first − second', kind: 'pairs', accepts: 'numeric' }],
    options: [{ key: 'trim', label: 'Proportion trimmed from each end', type: 'number', default: '0.2', hint: '0.2 (20%) is usual.' }, LEVEL],
    build: (c) => ({ pairs: c.pairs.pairs, trim: Number(c.options.trim), level: level(c) }),
  },
  trimmed_anova: {
    id: 'trimmed_anova',
    command: 'trimmed_anova',
    title: 'Robust ANOVA (Trimmed Means)',
    roles: [
      { key: 'dependent', label: 'Dependent List', hint: 'one result each', kind: 'many', accepts: 'numeric' },
      { key: 'factor', label: 'Factor', kind: 'one', accepts: 'any' },
    ],
    options: [{ key: 'trim', label: 'Proportion trimmed from each end', type: 'number', default: '0.2', hint: '0.2 (20%) is usual.' }],
    build: (c) => ({ dependent: c.roles.dependent, factor: c.roles.factor[0], trim: Number(c.options.trim) }),
  },
  ordinal_regression: {
    id: 'ordinal_regression',
    command: 'ordinal_regression',
    title: 'Ordinal Logistic Regression',
    roles: [
      { key: 'dependent', label: 'Dependent', hint: 'ordered categories, in code order', kind: 'one', accepts: 'any' },
      { key: 'covariates', label: 'Numeric predictors', kind: 'many', accepts: 'numeric', optional: true },
      { key: 'factors', label: 'Categorical predictors', hint: 'dummy-coded', kind: 'many', accepts: 'any', reference: true, optional: true },
    ],
    options: [LEVEL],
    build: (c) => ({ dependent: c.roles.dependent[0], covariates: c.roles.covariates, factors: c.roles.factors.map((name) => ({ name, reference: c.references.factors?.[name] ?? null })), level: level(c) }),
  },
  multinomial_regression: {
    id: 'multinomial_regression',
    command: 'multinomial_regression',
    title: 'Multinomial Logistic Regression',
    roles: [
      { key: 'dependent', label: 'Dependent', hint: 'three or more categories; choose the reference', kind: 'one', accepts: 'any', event: true },
      { key: 'covariates', label: 'Numeric predictors', kind: 'many', accepts: 'numeric', optional: true },
      { key: 'factors', label: 'Categorical predictors', hint: 'dummy-coded', kind: 'many', accepts: 'any', reference: true, optional: true },
    ],
    options: [LEVEL],
    build: (c) => ({ dependent: c.roles.dependent[0], reference: c.references.dependent?.[c.roles.dependent[0]] ?? null, covariates: c.roles.covariates, factors: c.roles.factors.map((name) => ({ name, reference: c.references.factors?.[name] ?? null })), level: level(c) }),
  },
  count_regression: {
    id: 'count_regression',
    command: 'count_regression',
    title: 'Count Regression',
    roles: [
      { key: 'dependent', label: 'Dependent', hint: 'counts', kind: 'one', accepts: 'numeric' },
      { key: 'covariates', label: 'Numeric predictors', kind: 'many', accepts: 'numeric', optional: true },
      { key: 'factors', label: 'Categorical predictors', hint: 'dummy-coded', kind: 'many', accepts: 'any', reference: true, optional: true },
      { key: 'exposure', label: 'Exposure', hint: 'optional: time at risk, entered as a log offset', kind: 'one', accepts: 'numeric', optional: true },
    ],
    options: [
      {
        key: 'family',
        label: 'Model',
        type: 'radio',
        choices: [
          { value: 'poisson', label: 'Poisson (default)' },
          { value: 'negative_binomial', label: 'Negative binomial', hint: 'for overdispersed counts' },
        ],
        default: 'poisson',
      },
      LEVEL,
    ],
    build: (c) => ({ dependent: c.roles.dependent[0], family: c.options.family, covariates: c.roles.covariates, factors: c.roles.factors.map((name) => ({ name, reference: c.references.factors?.[name] ?? null })), exposure: c.roles.exposure[0] ?? null, level: level(c) }),
  },
  doe_effects: {
    id: 'doe_effects',
    command: 'doe_effects',
    title: 'Factorial Effects',
    roles: [
      { key: 'response', label: 'Response', kind: 'one', accepts: 'numeric' },
      { key: 'factors', label: 'Two-level factors', hint: 'two settings (center points allowed) or two categories', kind: 'many', accepts: 'any' },
    ],
    options: [
      { key: 'order', label: 'Highest interaction order (empty: all)', type: 'text', default: '' },
      LEVEL,
    ],
    build: (c) => ({ response: c.roles.response[0], factors: c.roles.factors, order: String(c.options.order ?? '').trim() === '' ? null : Number(c.options.order), level: level(c) }),
  },
  rsm: {
    id: 'rsm',
    command: 'rsm',
    title: 'Response Surface',
    roles: [
      { key: 'response', label: 'Response', kind: 'one', accepts: 'numeric' },
      { key: 'factors', label: 'Factors', hint: 'in coded units', kind: 'many', accepts: 'numeric' },
      { key: 'block', label: 'Block', hint: 'optional', kind: 'one', accepts: 'any', optional: true },
    ],
    options: [
      {
        key: 'order',
        label: 'Model',
        type: 'radio',
        choices: [
          { value: '2', label: 'Second order (default)', hint: 'linear, interactions and squares; finds the stationary point' },
          { value: '1', label: 'First order', hint: 'for steepest ascent' },
        ],
        default: '2',
      },
      LEVEL,
    ],
    build: (c) => ({ response: c.roles.response[0], factors: c.roles.factors, block: c.roles.block[0] ?? null, order: Number(c.options.order), level: level(c) }),
  },
  doe_mixture: {
    id: 'doe_mixture',
    command: 'doe_mixture',
    title: 'Mixture Model',
    roles: [
      { key: 'response', label: 'Response', kind: 'one', accepts: 'numeric' },
      { key: 'components', label: 'Mixture components', hint: 'proportions, or amounts with the same total in every run', kind: 'many', accepts: 'numeric', min: 2 },
    ],
    options: [
      {
        key: 'model',
        label: 'Scheffé model',
        type: 'radio',
        choices: [
          { value: 'quadratic', label: 'Quadratic (default)', hint: 'pure components and the binary blending terms' },
          { value: 'linear', label: 'Linear', hint: 'linear blending only' },
          { value: 'special_cubic', label: 'Special cubic', hint: 'adds the ternary blending terms (3 or more components)' },
        ],
        default: 'quadratic',
      },
      LEVEL,
    ],
    build: (c) => ({ response: c.roles.response[0], components: c.roles.components, model: String(c.options.model), level: level(c) }),
  },
  spc: {
    id: 'spc',
    command: 'spc',
    title: 'Control Charts',
    roles: [
      { key: 'measures', label: 'Measurement', hint: 'one variable with a subgroup, or one variable per measurement in a subgroup', kind: 'many', accepts: 'numeric', showWhen: { option: 'chart', values: ['xbar_r', 'xbar_s'] } },
      { key: 'smoothed', label: 'Measurement', hint: 'one variable: individual values, or subgroup means with a subgroup variable', kind: 'many', accepts: 'numeric', showWhen: { option: 'chart', values: ['cusum', 'ewma'] } },
      { key: 'subgroup', label: 'Subgroup', hint: 'with a single measurement variable', kind: 'one', accepts: 'any', optional: true, showWhen: { option: 'chart', values: ['xbar_r', 'xbar_s', 'cusum', 'ewma'] } },
      { key: 'single', label: 'Measurement', kind: 'one', accepts: 'numeric', showWhen: { option: 'chart', values: ['imr'] } },
      { key: 'count', label: 'Count', hint: 'defectives (p, np) or defects (c, u)', kind: 'one', accepts: 'numeric', showWhen: { option: 'chart', values: ['p', 'np', 'c', 'u'] } },
      { key: 'size', label: 'Sample size', kind: 'one', accepts: 'numeric', showWhen: { option: 'chart', values: ['p', 'np', 'u'] } },
    ],
    options: [
      {
        key: 'chart',
        label: 'Chart',
        type: 'radio',
        choices: [
          { value: 'xbar_r', label: 'X̄ and R (default)', hint: 'subgroup means and ranges' },
          { value: 'xbar_s', label: 'X̄ and S', hint: 'subgroup means and standard deviations; larger subgroups' },
          { value: 'imr', label: 'Individuals and moving range', hint: 'one measurement at a time' },
          { value: 'p', label: 'p', hint: 'proportion defective' },
          { value: 'np', label: 'np', hint: 'number defective, equal sample sizes' },
          { value: 'c', label: 'c', hint: 'defects per sample' },
          { value: 'u', label: 'u', hint: 'defects per unit' },
          { value: 'cusum', label: 'CUSUM', hint: 'cumulative sums: quick to detect small, sustained shifts in the mean' },
          { value: 'ewma', label: 'EWMA', hint: 'exponentially weighted moving average: small shifts in the mean' },
        ],
        default: 'xbar_r',
      },
      { key: 'nsigmas', label: 'Limits at ± this many standard deviations', type: 'number', default: '3', showWhen: { option: 'chart', values: ['xbar_r', 'xbar_s', 'imr', 'p', 'np', 'c', 'u', 'ewma'] } },
      { key: 'run_length', label: 'Flag runs of this many points on one side (0: off)', type: 'number', default: '7', showWhen: { option: 'chart', values: ['xbar_r', 'xbar_s', 'imr', 'p', 'np', 'c', 'u'] } },
      { key: 'decision_interval', label: 'Decision interval h (standard errors)', type: 'number', default: '5', showWhen: { option: 'chart', values: ['cusum'] } },
      { key: 'se_shift', label: 'Shift to detect (standard errors)', type: 'number', default: '1', showWhen: { option: 'chart', values: ['cusum'] } },
      { key: 'head_start', label: 'Head start (fast initial response; 0: none)', type: 'number', default: '0', showWhen: { option: 'chart', values: ['cusum'] } },
      { key: 'lambda', label: 'Smoothing weight λ (0 to 1)', type: 'number', default: '0.2', showWhen: { option: 'chart', values: ['ewma'] } },
      { key: 'center', label: 'Target mean (empty: estimate from the data)', type: 'text', default: '', showWhen: { option: 'chart', values: ['cusum', 'ewma'] } },
      { key: 'std_dev', label: 'Process standard deviation (empty: estimate)', type: 'text', default: '', showWhen: { option: 'chart', values: ['cusum', 'ewma'] } },
      { key: 'lsl', label: 'Lower specification limit (capability)', type: 'text', default: '', showWhen: { option: 'chart', values: ['xbar_r', 'xbar_s', 'imr'] } },
      { key: 'usl', label: 'Upper specification limit (capability)', type: 'text', default: '', showWhen: { option: 'chart', values: ['xbar_r', 'xbar_s', 'imr'] } },
      { key: 'target', label: 'Target (optional)', type: 'text', default: '', showWhen: { option: 'chart', values: ['xbar_r', 'xbar_s', 'imr'] } },
    ],
    build: (c) => {
      const chart = String(c.options.chart);
      const opt = (k: string) => {
        const v = String(c.options[k] ?? '').trim();
        return v === '' ? null : Number(v);
      };
      const out: Record<string, unknown> = { chart };
      if (chart === 'cusum' || chart === 'ewma') {
        out.measures = c.roles.smoothed ?? [];
        out.subgroup = c.roles.subgroup?.[0] ?? null;
        const extra = chart === 'cusum' ? { decision_interval: Number(c.options.decision_interval), se_shift: Number(c.options.se_shift), head_start: Number(c.options.head_start) } : { lambda: Number(c.options.lambda), nsigmas: Number(c.options.nsigmas) };
        return { ...out, ...extra, center: opt('center'), std_dev: opt('std_dev') };
      }
      if (chart === 'xbar_r' || chart === 'xbar_s') {
        out.measures = c.roles.measures ?? [];
        out.subgroup = c.roles.subgroup?.[0] ?? null;
      } else if (chart === 'imr') {
        out.measures = c.roles.single ?? [];
      } else {
        out.count = c.roles.count?.[0] ?? null;
        if (chart !== 'c') out.size = c.roles.size?.[0] ?? null;
      }
      return { ...out, nsigmas: Number(c.options.nsigmas), run_length: Number(c.options.run_length), lsl: opt('lsl'), usl: opt('usl'), target: opt('target') };
    },
  },
  tree: {
    id: 'tree',
    command: 'tree',
    title: 'Decision Tree',
    roles: [
      { key: 'dependent', label: 'Outcome', hint: 'numeric or categorical', kind: 'one', accepts: 'any' },
      { key: 'predictors', label: 'Predictors', hint: 'numeric or categorical', kind: 'many', accepts: 'any' },
      { key: 'fold_variable', label: 'Fold variable', hint: 'optional: your own folds', kind: 'one', accepts: 'numeric', optional: true },
    ],
    options: [
      {
        key: 'kind',
        label: 'Tree',
        type: 'radio',
        choices: [
          { value: 'auto', label: 'From the outcome (default)', hint: 'classification for a categorical outcome' },
          { value: 'regression', label: 'Regression tree' },
          { value: 'classification', label: 'Classification tree' },
        ],
        default: 'auto',
      },
      {
        key: 'prune',
        label: 'Size',
        type: 'radio',
        choices: [
          { value: '1se', label: 'Prune by the one-SE rule (default)', hint: 'the smallest tree about as good as the best' },
          { value: 'min', label: 'Prune to the smallest cross-validated error' },
          { value: 'none', label: 'Grow to the complexity parameter only' },
        ],
        default: '1se',
      },
      { key: 'minsplit', label: 'Smallest node to split', type: 'number', default: '20' },
      { key: 'cp', label: 'Complexity parameter (cp)', type: 'number', default: '0.01', hint: 'a split must improve the fit by this share' },
      { key: 'maxsurrogate', label: 'Surrogate splits per split', type: 'number', default: '5', hint: 'send cases missing a split’s predictor by the best-agreeing other predictors' },
      { key: 'maxdepth', label: 'Maximum depth', type: 'number', default: '30' },
      { key: 'folds', label: 'Cross-validation folds', type: 'number', default: '10' },
      { key: 'seed', label: 'Seed', type: 'number', default: '20260101' },
    ],
    build: (c) => ({ dependent: c.roles.dependent[0], predictors: c.roles.predictors, fold_variable: c.roles.fold_variable[0] ?? null, kind: c.options.kind, prune: c.options.prune, minsplit: Number(c.options.minsplit), cp: Number(c.options.cp), maxsurrogate: Number(c.options.maxsurrogate), maxdepth: Number(c.options.maxdepth), folds: Number(c.options.folds), seed: Number(c.options.seed) }),
  },
  penalized: {
    id: 'penalized',
    command: 'penalized',
    title: 'Penalized Regression (Cross-Validated)',
    roles: [
      { key: 'dependent', label: 'Dependent', hint: 'numeric, or two categories', kind: 'one', accepts: 'any' },
      { key: 'covariates', label: 'Numeric predictors', kind: 'many', accepts: 'numeric', optional: true },
      { key: 'factors', label: 'Categorical predictors', hint: 'dummy-coded', kind: 'many', accepts: 'any', reference: true, optional: true },
      { key: 'fold_variable', label: 'Fold variable', hint: 'optional: your own folds', kind: 'one', accepts: 'numeric', optional: true },
    ],
    options: [
      {
        key: 'outcome',
        label: 'Outcome',
        type: 'radio',
        choices: [
          { value: 'continuous', label: 'Continuous (linear)' },
          { value: 'binary', label: 'Two categories (logistic)', hint: 'models the second category' },
        ],
        default: 'continuous',
      },
      {
        key: 'penalty',
        label: 'Penalty',
        type: 'radio',
        choices: [
          { value: 'lasso', label: 'Lasso (default)', hint: 'shrinks and selects predictors' },
          { value: 'ridge', label: 'Ridge', hint: 'shrinks; keeps every predictor' },
          { value: 'elastic', label: 'Elastic net', hint: 'a mix; set α below' },
        ],
        default: 'lasso',
      },
      { key: 'alpha', label: 'Elastic-net α (0 = ridge, 1 = lasso)', type: 'number', default: '0.5' },
      {
        key: 'choice',
        label: 'Choose λ by',
        type: 'radio',
        choices: [
          { value: '1se', label: 'One-standard-error rule (default)', hint: 'the simplest model about as good as the best' },
          { value: 'min', label: 'Minimum cross-validated error' },
        ],
        default: '1se',
      },
      { key: 'folds', label: 'Folds', type: 'number', default: '10' },
      { key: 'repeats', label: 'Repeat the cross-validation', type: 'number', default: '1' },
      { key: 'seed', label: 'Seed', type: 'number', default: '20260101' },
    ],
    build: (c) => ({ dependent: c.roles.dependent[0], covariates: c.roles.covariates, factors: c.roles.factors.map((name) => ({ name, reference: c.references.factors?.[name] ?? null })), fold_variable: c.roles.fold_variable[0] ?? null, outcome: c.options.outcome, penalty: c.options.penalty, alpha: Number(c.options.alpha), choice: c.options.choice, folds: Number(c.options.folds), repeats: Number(c.options.repeats), seed: Number(c.options.seed) }),
  },
  pgmm: {
    id: 'pgmm',
    command: 'pgmm',
    title: 'Dynamic Panel (GMM)',
    roles: [
      { key: 'dependent', label: 'Outcome', kind: 'one', accepts: 'numeric' },
      { key: 'covariates', label: 'Covariates', hint: 'optional; treated as exogenous', kind: 'many', accepts: 'numeric', optional: true },
      { key: 'individual', label: 'Individual', hint: 'the panel unit (person, firm, country)', kind: 'one', accepts: 'any' },
      { key: 'time', label: 'Period', kind: 'one', accepts: 'any' },
    ],
    options: [
      {
        key: 'model',
        label: 'Estimator',
        type: 'radio',
        choices: [
          { value: 'twosteps', label: 'Two-step GMM (default)', hint: 'with Windmeijer-corrected robust standard errors' },
          { value: 'onestep', label: 'One-step GMM' },
        ],
        default: 'twosteps',
      },
      {
        key: 'effect',
        label: 'Effects',
        type: 'radio',
        choices: [
          { value: 'twoways', label: 'Individual and time (default)', hint: 'adds time dummies' },
          { value: 'individual', label: 'Individual only' },
        ],
        default: 'twoways',
      },
      { key: 'y_lags', label: 'Lags of the outcome', type: 'number', default: '1' },
      { key: 'x_lags', label: 'Lags of each covariate (0: current only)', type: 'number', default: '0' },
      { key: 'gmm_min', label: 'Instruments: from lag', type: 'number', default: '2' },
      { key: 'gmm_max', label: 'Instruments: to lag', type: 'number', default: '99' },
      { key: 'collapse', label: 'Collapse the instruments', type: 'toggle', default: false, hint: 'one column per lag; fewer instruments' },
      { key: 'robust', label: 'Robust standard errors', type: 'toggle', default: true },
      LEVEL,
    ],
    build: (c) => ({
      dependent: c.roles.dependent[0],
      covariates: c.roles.covariates ?? [],
      individual: c.roles.individual[0],
      time: c.roles.time[0],
      model: c.options.model,
      effect: c.options.effect,
      y_lags: Number(c.options.y_lags),
      x_lags: Number(c.options.x_lags),
      gmm_min: Number(c.options.gmm_min),
      gmm_max: Number(c.options.gmm_max),
      collapse: c.options.collapse,
      robust: c.options.robust,
      level: level(c),
    }),
  },
  panel: {
    id: 'panel',
    command: 'panel',
    title: 'Panel Data Regression',
    roles: [
      { key: 'dependent', label: 'Dependent', kind: 'one', accepts: 'numeric' },
      { key: 'covariates', label: 'Numeric predictors', kind: 'many', accepts: 'numeric', optional: true },
      { key: 'factors', label: 'Categorical predictors', hint: 'dummy-coded', kind: 'many', accepts: 'any', reference: true, optional: true },
      { key: 'individual', label: 'Individual (unit)', kind: 'one', accepts: 'any' },
      { key: 'time', label: 'Period', kind: 'one', accepts: 'any' },
    ],
    options: [
      {
        key: 'model',
        label: 'Model',
        type: 'radio',
        choices: [
          { value: 'within', label: 'Fixed effects (within) (default)', hint: 'controls for everything stable about each individual' },
          { value: 'random', label: 'Random effects', hint: 'Swamy-Arora; assumes the effects are unrelated to the predictors' },
          { value: 'pooling', label: 'Pooled OLS' },
          { value: 'fd', label: 'First differences' },
          { value: 'between', label: 'Between (individual means)' },
        ],
        default: 'within',
      },
      {
        key: 'effect',
        label: 'Effects for',
        type: 'radio',
        choices: [
          { value: 'individual', label: 'Individuals (default)' },
          { value: 'time', label: 'Periods' },
          { value: 'twoways', label: 'Both (two-way)', hint: 'random effects: balanced panels only' },
        ],
        default: 'individual',
        showWhen: { option: 'model', values: ['within', 'random'] },
      },
      { key: 'robust', label: 'Cluster-robust standard errors (by individual)', type: 'toggle', default: false },
      { key: 'tests', label: 'F, Hausman and Breusch-Pagan tests', type: 'toggle', default: true },
      LEVEL,
    ],
    build: (c) => ({ dependent: c.roles.dependent[0], covariates: c.roles.covariates, factors: c.roles.factors.map((name) => ({ name, reference: c.references.factors?.[name] ?? null })), individual: c.roles.individual[0], time: c.roles.time[0], model: c.options.model, effect: c.options.effect, robust: c.options.robust, tests: c.options.tests, level: level(c) }),
  },
  gee: {
    id: 'gee',
    command: 'gee',
    title: 'Generalized Estimating Equations',
    roles: [
      { key: 'dependent', label: 'Dependent', hint: '0/1 for binomial, counts for Poisson', kind: 'one', accepts: 'numeric' },
      { key: 'covariates', label: 'Numeric predictors', kind: 'many', accepts: 'numeric', optional: true },
      { key: 'factors', label: 'Categorical predictors', hint: 'dummy-coded', kind: 'many', accepts: 'any', reference: true, optional: true },
      { key: 'cluster', label: 'Cluster (subject)', kind: 'one', accepts: 'any' },
      { key: 'wave', label: 'Occasion', hint: 'optional: whole numbers; row order otherwise', kind: 'one', accepts: 'numeric', optional: true },
    ],
    options: [
      {
        key: 'family',
        label: 'Outcome',
        type: 'radio',
        choices: [
          { value: 'gaussian', label: 'Continuous (Gaussian, identity link)' },
          { value: 'binomial', label: 'Binary (binomial, logit link)', hint: 'odds ratios' },
          { value: 'poisson', label: 'Count (Poisson, log link)', hint: 'rate ratios' },
        ],
        default: 'gaussian',
      },
      {
        key: 'corstr',
        label: 'Working correlation',
        type: 'radio',
        choices: [
          { value: 'exchangeable', label: 'Exchangeable (default)', hint: 'the same correlation for every pair' },
          { value: 'ar1', label: 'AR(1)', hint: 'decays with the distance between occasions' },
          { value: 'unstructured', label: 'Unstructured', hint: 'a correlation for each pair of occasions' },
          { value: 'independence', label: 'Independence' },
        ],
        default: 'exchangeable',
      },
      LEVEL,
    ],
    build: (c) => ({ dependent: c.roles.dependent[0], covariates: c.roles.covariates, factors: c.roles.factors.map((name) => ({ name, reference: c.references.factors?.[name] ?? null })), cluster: c.roles.cluster[0], wave: c.roles.wave[0] ?? null, family: c.options.family, corstr: c.options.corstr, level: level(c) }),
  },
  logistic: {
    id: 'logistic',
    command: 'logistic',
    title: 'Binary Logistic Regression',
    roles: [
      { key: 'dependent', label: 'Dependent', hint: 'two categories', kind: 'one', accepts: 'any', event: true },
      { key: 'covariates', label: 'Numeric predictors', hint: 'entered as they are', kind: 'many', accepts: 'numeric' },
      { key: 'factors', label: 'Categorical predictors', hint: 'dummy-coded', kind: 'many', accepts: 'any', reference: true },
      { key: 'covariates2', label: 'Block 2: numeric predictors', hint: 'optional: tested by likelihood-ratio change', kind: 'many', accepts: 'numeric', optional: true },
      { key: 'factors2', label: 'Block 2: categorical predictors', hint: 'optional', kind: 'many', accepts: 'any', reference: true, optional: true },
    ],
    options: [
      { key: 'interactions', label: 'Interactions', type: 'text', default: '', hint: 'Optional: pairs of predictors in the model, such as hours*cond. They enter as a final block.' },
      { key: 'center', label: 'Mean-center numeric predictors in interactions', type: 'toggle', default: false },
      { key: 'roc', label: 'ROC curve', type: 'toggle', default: false, hint: 'With the area under the curve and its DeLong interval.' },
      LEVEL,
    ],
    build: (c) => {
      const covs2 = c.roles.covariates2 ?? [];
      const facs2 = c.roles.factors2 ?? [];
      return {
        dependent: c.roles.dependent[0],
        event: c.references.dependent?.[c.roles.dependent[0]] ?? null,
        covariates: c.roles.covariates,
        factors: c.roles.factors.map((name) => ({ name, reference: c.references.factors?.[name] ?? null })),
        level: level(c),
        blocks: covs2.length || facs2.length ? [{ covariates: covs2, factors: facs2.map((name) => ({ name, reference: c.references.factors2?.[name] ?? null })) }] : [],
        interactions: interactionPairs(c.options.interactions),
        center: c.options.center,
        roc: c.options.roc,
      };
    },
  },
  reliability: {
    id: 'reliability',
    command: 'reliability',
    title: 'Reliability Analysis',
    roles: [
      { key: 'items', label: 'Items', hint: 'scored in the scale’s direction', kind: 'many', accepts: 'numeric' },
      { key: 'reverse', label: 'Reverse-scored items', hint: 'worded the other way round; part of the scale too', kind: 'many', accepts: 'numeric' },
    ],
    options: [
      { key: 'scale_min', label: 'Lowest possible score', type: 'number', default: '', hint: 'For reversing items; leave empty to use the lowest value found.' },
      { key: 'scale_max', label: 'Highest possible score', type: 'number', default: '', hint: 'Leave empty to use the highest value found.' },
      LEVEL,
    ],
    build: (c) => {
      const num = (v: unknown) => (typeof v === 'string' && v.trim() !== '' ? Number(v) : null);
      return { items: [...c.roles.items, ...c.roles.reverse], reverse: c.roles.reverse, scale_min: num(c.options.scale_min), scale_max: num(c.options.scale_max), level: level(c) };
    },
  },
  canonical: {
    id: 'canonical',
    command: 'canonical',
    title: 'Canonical Correlation',
    roles: [
      { key: 'set1', label: 'First set', kind: 'many', accepts: 'numeric' },
      { key: 'set2', label: 'Second set', kind: 'many', accepts: 'numeric' },
    ],
    options: [],
    build: (c) => ({ set1: c.roles.set1, set2: c.roles.set2 }),
  },
  chart_histogram: {
    id: 'chart_histogram',
    command: 'chart',
    title: 'Histogram',
    roles: [{ key: 'variables', label: 'Variables', hint: 'one chart each', kind: 'many', accepts: 'numeric' }],
    options: [{ key: 'normal', label: 'Normal curve', type: 'toggle', default: false, hint: 'The normal distribution with the same mean and SD.' }],
    build: (c) => ({ kind: 'histogram', variables: c.roles.variables, normal: c.options.normal }),
  },
  chart_boxplot: {
    id: 'chart_boxplot',
    command: 'chart',
    title: 'Box Plot',
    roles: [
      { key: 'variables', label: 'Variables', hint: 'a box each, or a chart each when grouped', kind: 'many', accepts: 'numeric' },
      { key: 'group', label: 'Category axis', hint: 'optional: a box per category', kind: 'one', accepts: 'any', optional: true },
    ],
    options: [],
    build: (c) => ({ kind: 'boxplot', variables: c.roles.variables, group: c.roles.group[0] ?? null }),
  },
  chart_bar: {
    id: 'chart_bar',
    command: 'chart',
    title: 'Bar Chart',
    roles: [
      { key: 'variable', label: 'Category axis', kind: 'one', accepts: 'any' },
      { key: 'mean_of', label: 'Mean of', hint: 'for bars of means', kind: 'one', accepts: 'numeric', optional: true },
    ],
    options: [
      {
        key: 'bar_stat',
        label: 'Bars show',
        type: 'radio',
        choices: [
          { value: 'count', label: 'Counts (default)' },
          { value: 'percent', label: 'Percentages' },
          { value: 'mean', label: 'Means with 95% confidence intervals', hint: 'of the variable under Mean of' },
        ],
        default: 'count',
      },
    ],
    build: (c) => ({ kind: 'bar', variables: c.roles.variable, bar_stat: c.options.bar_stat, mean_of: c.roles.mean_of[0] ?? null }),
  },
  chart_scatter: {
    id: 'chart_scatter',
    command: 'chart',
    title: 'Scatterplot',
    roles: [
      { key: 'y', label: 'Vertical axis (Y)', kind: 'one', accepts: 'numeric' },
      { key: 'x', label: 'Horizontal axis (X)', kind: 'one', accepts: 'numeric' },
    ],
    options: [{ key: 'line', label: 'Fit line', type: 'toggle', default: false, hint: 'The least-squares line of Y on X.' }],
    build: (c) => ({ kind: 'scatter', x: c.roles.x[0], y: c.roles.y[0], line: c.options.line }),
  },
  chart_qq: {
    id: 'chart_qq',
    command: 'chart',
    title: 'Normal Q-Q Plot',
    roles: [{ key: 'variables', label: 'Variables', hint: 'one chart each', kind: 'many', accepts: 'numeric' }],
    options: [],
    build: (c) => ({ kind: 'qq', variables: c.roles.variables }),
  },
  chisq_gof: {
    id: 'chisq_gof',
    command: 'chisq_gof',
    title: 'Chi-Square Goodness of Fit',
    roles: [{ key: 'variable', label: 'Test Variable', kind: 'one', accepts: 'any' }],
    options: [{ key: 'expected', label: 'Expected proportions', type: 'text', default: '', hint: 'Leave empty for equal proportions, or give one number per category in code order, such as 1, 2, 2 (rescaled to add to 1).' }],
    build: (c) => ({ variable: c.roles.variable[0], expected: numberList(c.options.expected) }),
  },
  binomial: {
    id: 'binomial',
    command: 'binomial',
    title: 'Binomial Test',
    roles: [{ key: 'variables', label: 'Test Variables', hint: 'two categories each', kind: 'many', accepts: 'any' }],
    options: [{ key: 'test_proportion', label: 'Test proportion', type: 'number', default: '0.5', hint: 'For the first category (lowest code).' }, LEVEL],
    build: (c) => ({ variables: c.roles.variables, test_proportion: Number(c.options.test_proportion), level: level(c) }),
  },
  runs: {
    id: 'runs',
    command: 'runs',
    title: 'Runs Test',
    roles: [{ key: 'variables', label: 'Test Variables', hint: 'cases in their file order', kind: 'many', accepts: 'numeric' }],
    options: [
      {
        key: 'cut',
        label: 'Cut point',
        type: 'radio',
        choices: [
          { value: 'median', label: 'Median (default)' },
          { value: 'mean', label: 'Mean' },
          { value: 'value', label: 'A value', hint: 'given below' },
        ],
        default: 'median',
      },
      { key: 'value', label: 'Cut value', type: 'text', default: '', hint: 'Only used when the cut point is a value.' },
    ],
    build: (c) => ({ variables: c.roles.variables, cut: c.options.cut, value: c.options.cut === 'value' ? Number(c.options.value) : null }),
  },
  sign_test: {
    id: 'sign_test',
    command: 'sign_test',
    title: 'Sign Test',
    roles: [{ key: 'pairs', label: 'Paired Variables', hint: 'difference = first − second', kind: 'pairs', accepts: 'numeric' }],
    options: [],
    build: (c) => ({ pairs: c.pairs.pairs }),
  },
  median_test: {
    id: 'median_test',
    command: 'median_test',
    title: 'Median Test',
    roles: [
      { key: 'dependent', label: 'Test Variables', hint: 'one result each', kind: 'many', accepts: 'numeric' },
      { key: 'factor', label: 'Grouping Variable', kind: 'one', accepts: 'any' },
    ],
    options: [],
    build: (c) => ({ dependent: c.roles.dependent, factor: c.roles.factor[0] }),
  },
  ks2: {
    id: 'ks2',
    command: 'ks2',
    title: 'Two-Sample Kolmogorov-Smirnov Test',
    roles: [
      { key: 'test', label: 'Test Variables', hint: 'one result each', kind: 'many', accepts: 'numeric' },
      { key: 'group', label: 'Grouping Variable', kind: 'one', accepts: 'numeric', twoGroups: true },
    ],
    options: [],
    build: (c) => ({ test_variables: c.roles.test, grouping_variable: c.roles.group[0], groups: c.groups.group }),
  },
  mediation: {
    id: 'mediation',
    command: 'mediation',
    title: 'Mediation',
    roles: [
      { key: 'x', label: 'X (predictor)', kind: 'one', accepts: 'numeric' },
      { key: 'mediators', label: 'Mediators', hint: 'one or more, in parallel', kind: 'many', accepts: 'numeric' },
      { key: 'y', label: 'Y (outcome)', kind: 'one', accepts: 'numeric' },
      { key: 'covariates', label: 'Covariates', hint: 'optional', kind: 'many', accepts: 'numeric', optional: true },
    ],
    options: [
      { key: 'resamples', label: 'Bootstrap resamples', type: 'number', default: '5000' },
      { key: 'seed', label: 'Seed', type: 'number', default: '20260101', hint: 'The same seed gives the same intervals.' },
      LEVEL,
    ],
    build: (c) => ({ x: c.roles.x[0], mediators: c.roles.mediators, y: c.roles.y[0], covariates: c.roles.covariates, resamples: Number(c.options.resamples), seed: Number(c.options.seed), level: level(c) }),
  },
  moderation: {
    id: 'moderation',
    command: 'moderation',
    title: 'Moderation',
    roles: [
      { key: 'x', label: 'X (predictor)', kind: 'one', accepts: 'numeric' },
      { key: 'w', label: 'W (moderator)', kind: 'one', accepts: 'numeric' },
      { key: 'y', label: 'Y (outcome)', kind: 'one', accepts: 'numeric' },
      { key: 'covariates', label: 'Covariates', hint: 'optional', kind: 'many', accepts: 'numeric', optional: true },
    ],
    options: [
      { key: 'center', label: 'Mean-center X and W', type: 'toggle', default: false },
      {
        key: 'probe',
        label: 'Probe the slope of X at',
        type: 'radio',
        choices: [
          { value: 'percentiles', label: '16th, 50th and 84th percentiles of W (default)' },
          { value: 'mean_sd', label: 'Mean of W and ± 1 SD' },
        ],
        default: 'percentiles',
      },
      { key: 'johnson_neyman', label: 'Johnson-Neyman values', type: 'toggle', default: true },
      { key: 'plot', label: 'Simple slopes plot', type: 'toggle', default: true },
      LEVEL,
    ],
    build: (c) => ({ x: c.roles.x[0], w: c.roles.w[0], y: c.roles.y[0], covariates: c.roles.covariates, center: c.options.center, probe: c.options.probe, johnson_neyman: c.options.johnson_neyman, plot: c.options.plot, level: level(c) }),
  },
  factor: {
    id: 'factor',
    command: 'factor',
    title: 'Factor Analysis',
    roles: [{ key: 'variables', label: 'Variables', hint: 'three or more', kind: 'many', accepts: 'numeric', min: 3 }],
    options: [
      {
        key: 'extraction',
        label: 'Extraction',
        type: 'radio',
        choices: [
          { value: 'ml', label: 'Maximum likelihood (default)' },
          { value: 'paf', label: 'Principal axis factoring' },
          { value: 'minres', label: 'Minimum residual' },
          { value: 'pca', label: 'Principal components', hint: 'components, not factors' },
        ],
        default: 'ml',
      },
      {
        key: 'rotation',
        label: 'Rotation',
        type: 'radio',
        choices: [
          { value: 'oblimin', label: 'Oblimin (default)', hint: 'oblique: factors may correlate' },
          { value: 'promax', label: 'Promax', hint: 'oblique' },
          { value: 'varimax', label: 'Varimax', hint: 'orthogonal' },
          { value: 'none', label: 'None' },
        ],
        default: 'oblimin',
      },
      { key: 'factors', label: 'Number of factors', type: 'number', default: '', hint: 'Leave empty to let parallel analysis decide.' },
      { key: 'parallel', label: 'Parallel analysis', type: 'toggle', default: true },
      { key: 'scree', label: 'Scree plot', type: 'toggle', default: true },
      { key: 'hide_below', label: 'Hide loadings below', type: 'number', default: '0', hint: 'Such as 0.3; they are still computed.' },
    ],
    build: (c) => ({
      variables: c.roles.variables,
      extraction: c.options.extraction,
      rotation: c.options.rotation,
      factors: String(c.options.factors ?? '').trim() ? Number(c.options.factors) : null,
      parallel: c.options.parallel,
      scree: c.options.scree,
      hide_below: Number(c.options.hide_below || 0),
    }),
  },
  manova: {
    id: 'manova',
    command: 'manova',
    title: 'MANOVA',
    roles: [
      { key: 'dependent', label: 'Dependent variables', hint: 'two or more', kind: 'many', accepts: 'numeric', min: 2 },
      { key: 'factor', label: 'Factor', kind: 'one', accepts: 'any' },
    ],
    options: [
      {
        key: 'test',
        label: 'Test reported',
        type: 'radio',
        choices: [
          { value: 'pillai', label: 'Pillai’s trace (default)', hint: 'the most robust' },
          { value: 'wilks', label: 'Wilks’ lambda' },
          { value: 'hotelling_lawley', label: 'Hotelling-Lawley trace' },
          { value: 'roy', label: 'Roy’s largest root' },
        ],
        default: 'pillai',
      },
      { key: 'univariate', label: 'Each outcome’s ANOVA', type: 'toggle', default: true },
    ],
    build: (c) => ({ dependent: c.roles.dependent, factor: c.roles.factor[0], test: c.options.test, univariate: c.options.univariate }),
  },
  rm_anova: {
    id: 'rm_anova',
    command: 'rm_anova',
    title: 'Repeated-Measures ANOVA',
    roles: [
      { key: 'measures', label: 'Measures', hint: 'in order: the levels of the within-subjects factor', kind: 'many', accepts: 'numeric', min: 2 },
      { key: 'between', label: 'Between-subjects factor', hint: 'optional: a mixed ANOVA', kind: 'one', accepts: 'any', optional: true },
    ],
    options: [
      { key: 'factor_name', label: 'Within-subjects factor name', type: 'text', default: 'time' },
      {
        key: 'correction',
        label: 'Correction reported',
        type: 'radio',
        choices: [
          { value: 'greenhouse_geisser', label: 'Greenhouse-Geisser (default)' },
          { value: 'huynh_feldt', label: 'Huynh-Feldt' },
          { value: 'none', label: 'None (sphericity assumed)' },
        ],
        default: 'greenhouse_geisser',
      },
      {
        key: 'post_hoc',
        label: 'Paired comparisons of the measures',
        type: 'radio',
        choices: [
          { value: 'none', label: 'None (default)' },
          { value: 'bonferroni', label: 'Bonferroni' },
          { value: 'holm', label: 'Holm' },
        ],
        default: 'none',
      },
      { key: 'plots', label: 'Profile plot', type: 'toggle', default: false },
      LEVEL,
    ],
    build: (c) => ({ measures: c.roles.measures, between: c.roles.between[0] ?? null, factor_name: String(c.options.factor_name || 'time'), correction: c.options.correction, post_hoc: c.options.post_hoc, plots: c.options.plots, level: level(c) }),
  },
  bootstrap: {
    id: 'bootstrap',
    command: 'bootstrap',
    title: 'Bootstrap',
    roles: [
      { key: 'variables', label: 'Variables', hint: 'for a regression, the predictors', kind: 'many', accepts: 'numeric' },
      { key: 'dependent', label: 'Dependent', hint: 'only for a regression', kind: 'one', accepts: 'numeric', optional: true },
    ],
    options: [
      {
        key: 'statistic',
        label: 'Statistic',
        type: 'radio',
        choices: [
          { value: 'mean', label: 'Mean (default)' },
          { value: 'median', label: 'Median' },
          { value: 'trimmed_mean', label: '5% trimmed mean' },
          { value: 'sd', label: 'Standard deviation' },
          { value: 'correlation', label: 'Correlations', hint: 'every pair of the variables' },
          { value: 'regression', label: 'Regression coefficients', hint: 'the dependent on the variables' },
        ],
        default: 'mean',
      },
      {
        key: 'coefficient',
        label: 'Correlation coefficient',
        type: 'radio',
        choices: [
          { value: 'pearson', label: 'Pearson (default)' },
          { value: 'spearman', label: 'Spearman' },
        ],
        default: 'pearson',
      },
      { key: 'resamples', label: 'Resamples', type: 'number', default: '2000', hint: 'Between 100 and 100,000.' },
      { key: 'seed', label: 'Seed', type: 'number', default: '20260101', hint: 'The same seed gives the same intervals.' },
      LEVEL,
    ],
    build: (c) => ({
      statistic: c.options.statistic,
      variables: c.roles.variables,
      dependent: c.options.statistic === 'regression' ? (c.roles.dependent[0] ?? null) : null,
      coefficient: c.options.coefficient,
      resamples: Number(c.options.resamples),
      seed: Number(c.options.seed),
      level: level(c),
    }),
  },
  curve: {
    id: 'curve',
    command: 'curve',
    title: 'Curve Estimation',
    roles: [
      { key: 'dependent', label: 'Dependent(s)', hint: 'one result each', kind: 'many', accepts: 'numeric' },
      { key: 'independent', label: 'Independent', kind: 'one', accepts: 'numeric' },
    ],
    options: [
      {
        key: 'models',
        label: 'Models',
        type: 'checks',
        choices: [
          { value: 'linear', label: 'Linear', hint: 'default' },
          { value: 'quadratic', label: 'Quadratic' },
          { value: 'cubic', label: 'Cubic' },
          { value: 'logarithmic', label: 'Logarithmic', hint: 'needs positive x' },
          { value: 'inverse', label: 'Inverse', hint: 'needs nonzero x' },
          { value: 'power', label: 'Power', hint: 'needs positive x and y' },
          { value: 'exponential', label: 'Exponential', hint: 'needs positive y' },
          { value: 's_curve', label: 'S', hint: 'needs nonzero x and positive y' },
        ],
        default: ['linear'],
        min: 1,
      },
      { key: 'plot', label: 'Plot the curves over the data', type: 'toggle', default: true },
      LEVEL,
    ],
    build: (c) => ({ dependent: c.roles.dependent, independent: c.roles.independent[0], models: c.options.models, plot: c.options.plot, level: level(c) }),
  },
  explore: {
    id: 'explore',
    command: 'explore',
    title: 'Explore',
    roles: [
      { key: 'dependent', label: 'Dependent List', hint: 'one result each', kind: 'many', accepts: 'numeric' },
      { key: 'factor', label: 'Factor List', hint: 'optional: describe each group', kind: 'one', accepts: 'any', optional: true },
    ],
    options: [
      { key: 'percentiles', label: 'Percentiles', type: 'toggle', default: true },
      { key: 'normality', label: 'Normality tests', type: 'toggle', default: true, hint: 'Shapiro-Wilk and Kolmogorov-Smirnov (Lilliefors).' },
      { key: 'plots', label: 'Box plots and Q-Q plots', type: 'toggle', default: true },
      LEVEL,
    ],
    build: (c) => ({ dependent: c.roles.dependent, factor: c.roles.factor[0] ?? null, percentiles: c.options.percentiles, normality: c.options.normality, plots: c.options.plots, level: level(c) }),
  },
  cochran: {
    id: 'cochran',
    command: 'cochran',
    title: 'Cochran’s Q',
    roles: [{ key: 'variables', label: 'Test Variables', hint: 'yes/no measures with the same two codes', kind: 'many', accepts: 'numeric', min: 2 }],
    options: [],
    build: (c) => ({ variables: c.roles.variables }),
  },
  kappa: {
    id: 'kappa',
    command: 'kappa',
    title: 'Cohen’s Kappa',
    roles: [
      { key: 'rater1', label: 'Rater 1', kind: 'one', accepts: 'numeric' },
      { key: 'rater2', label: 'Rater 2', kind: 'one', accepts: 'numeric' },
    ],
    options: [
      {
        key: 'weights',
        label: 'Weights',
        type: 'radio',
        choices: [
          { value: 'none', label: 'Unweighted (default)' },
          { value: 'linear', label: 'Linear', hint: 'for ordered categories' },
          { value: 'quadratic', label: 'Quadratic', hint: 'for ordered categories' },
        ],
        default: 'none',
      },
      LEVEL,
    ],
    build: (c) => ({ rater1: c.roles.rater1[0], rater2: c.roles.rater2[0], weights: c.options.weights, level: level(c) }),
  },
  fleiss: {
    id: 'fleiss',
    command: 'fleiss',
    title: 'Fleiss’ Kappa',
    roles: [{ key: 'raters', label: 'Raters', hint: 'one variable per rater', kind: 'many', accepts: 'numeric', min: 2 }],
    options: [],
    build: (c) => ({ raters: c.roles.raters }),
  },
  kripp: {
    id: 'kripp',
    command: 'kripp',
    title: 'Krippendorff’s Alpha',
    roles: [{ key: 'raters', label: 'Raters', hint: 'one variable per rater; missing ratings allowed', kind: 'many', accepts: 'numeric', min: 2 }],
    options: [
      {
        key: 'metric',
        label: 'Level of the ratings',
        type: 'radio',
        choices: [
          { value: 'nominal', label: 'Nominal (default)' },
          { value: 'ordinal', label: 'Ordinal' },
          { value: 'interval', label: 'Interval' },
          { value: 'ratio', label: 'Ratio' },
        ],
        default: 'nominal',
      },
    ],
    build: (c) => ({ raters: c.roles.raters, metric: c.options.metric }),
  },
  icc: {
    id: 'icc',
    command: 'icc',
    title: 'Intraclass Correlation',
    roles: [{ key: 'raters', label: 'Raters', hint: 'one variable per rater', kind: 'many', accepts: 'numeric', min: 2 }],
    options: [
      {
        key: 'model',
        label: 'Model',
        type: 'radio',
        choices: [
          { value: 'two_way', label: 'Two-way (default)', hint: 'the same raters rate every subject' },
          { value: 'one_way', label: 'One-way', hint: 'different raters for different subjects' },
        ],
        default: 'two_way',
      },
      {
        key: 'kind',
        label: 'Type',
        type: 'radio',
        choices: [
          { value: 'agreement', label: 'Absolute agreement (default)' },
          { value: 'consistency', label: 'Consistency', hint: 'two-way only' },
        ],
        default: 'agreement',
      },
      {
        key: 'unit',
        label: 'Unit',
        type: 'radio',
        choices: [
          { value: 'single', label: 'Single rater (default)' },
          { value: 'average', label: 'Average of the raters' },
        ],
        default: 'single',
      },
      LEVEL,
    ],
    build: (c) => ({ raters: c.roles.raters, model: c.options.model, kind: c.options.kind, unit: c.options.unit, level: level(c) }),
  },
};

/** Interaction pairs typed as "a*b, c*d" (also × or :), or an empty list. */
export function interactionPairs(v: unknown): [string, string][] {
  if (typeof v !== 'string') return [];
  return v
    .split(/[,;]+/)
    .map((t) => t.split(/[*×:]/).map((x) => x.trim()))
    .filter((p) => p.length === 2 && p[0] && p[1])
    .map((p) => [p[0], p[1]] as [string, string]);
}

/** Contrasts typed as rows of coefficients separated by semicolons ("1 -0.5 -0.5; 0 1 -1"). */
export function contrastRows(v: unknown): number[][] {
  if (typeof v !== 'string') return [];
  return v
    .split(/[;\n]+/)
    .map((row) => row.split(/[,\s]+/).filter(Boolean).map(Number))
    .filter((row) => row.length > 0);
}

/** A list of numbers typed with commas or spaces ("1, 2, 2"), or null when the box is empty. */
export function numberList(v: unknown): number[] | null {
  if (typeof v !== 'string' || !v.trim()) return null;
  return v.split(/[,;\s]+/).filter(Boolean).map(Number);
}

/** The dialog for a recorded command: the command itself, or for a chart the dialog of its kind. */
export function dialogFor(command: string, args: Record<string, unknown>): string {
  if (command === 'chart') return `chart_${String(args.kind ?? '')}`;
  if (command === 'permutation') return `perm_${String(args.test ?? 'two_groups')}`;
  return command === 'ttest_independent' ? 'ttest' : command;
}

/** The first problem with what was collected, in words for the user, or null when the analysis can run. */
export function problem(def: AnalysisDef, c: Collected): string | null {
  for (const r of visibleRoles(def, c.options)) {
    if (r.kind === 'pairs') {
      const pairs = c.pairs[r.key] ?? [];
      if (!pairs.length) return `Add at least one pair to ${r.label}.`;
      continue;
    }
    const got = c.roles[r.key] ?? [];
    const optional = r.optional || ((def.id === 'regression' || def.id === 'logistic') && r.key !== 'dependent') || def.id === 'reliability';
    if (r.max && got.length > r.max) return `Move at most ${r.max} variables into ${r.label}.`;
    if (!optional && got.length < (r.min ?? 1)) return r.min && r.min > 1 ? `Move at least ${r.min} variables into ${r.label}.` : `Move a variable into ${r.label}.`;
    if (r.twoGroups && got.length) {
      const g = c.groups[r.key];
      if (!g) return 'The grouping variable needs at least two groups with data.';
      if (g[0] === g[1]) return 'Choose two different groups.';
    }
  }
  if ((def.id === 'regression' || def.id === 'logistic') && !(c.roles.covariates?.length || c.roles.factors?.length)) return 'Move at least one predictor into the model.';
  if (def.id === 'regression' || def.id === 'logistic') {
    const inModel = new Set([...(c.roles.covariates ?? []), ...(c.roles.factors ?? []), ...(c.roles.covariates2 ?? []), ...(c.roles.factors2 ?? [])]);
    const text = typeof c.options.interactions === 'string' ? c.options.interactions.trim() : '';
    const pairs = interactionPairs(c.options.interactions);
    if (text && !pairs.length) return 'Write interactions as pairs of predictors, such as hours*cond.';
    const bad = pairs.flat().find((n) => !inModel.has(n));
    if (bad) return `${bad} is in an interaction but not in the model; move it into a predictor box first.`;
  }
  if (def.id === 'reliability' && (c.roles.items?.length ?? 0) + (c.roles.reverse?.length ?? 0) < 2) return 'Move at least two items into the scale.';
  if (def.id === 'chisq_gof') {
    const e = numberList(c.options.expected);
    if (e && e.some((x) => !(Number.isFinite(x) && x > 0))) return 'Expected proportions must be numbers above zero, separated by commas.';
  }
  if (def.id === 'runs' && c.options.cut === 'value' && !Number.isFinite(Number(c.options.value as string)) ) return 'Give the cut value as a number.';
  if (def.id === 'runs' && c.options.cut === 'value' && !(c.options.value as string).trim()) return 'Give the cut value.';
  if (def.id === 'binomial' && !(Number(c.options.test_proportion) > 0 && Number(c.options.test_proportion) < 1)) return 'The test proportion must be between 0 and 1.';
  if (def.id === 'chart_bar' && c.options.bar_stat === 'mean' && !(c.roles.mean_of?.length)) return 'Move a numeric variable into Mean of, or choose counts or percentages.';
  if (def.id === 'canonical' && (c.roles.set1?.length ?? 0) + (c.roles.set2?.length ?? 0) < 3) return 'Put at least three variables in the two sets together (with one in each, use Bivariate Correlations).';
  for (const o of def.options) {
    const v = c.options[o.key];
    if (o.type === 'number') {
      const n = Number(v);
      if (def.id === 'reliability' && o.key !== 'level' && typeof v === 'string' && v.trim() === '') continue;
      if (typeof v !== 'string' || v.trim() === '' || !Number.isFinite(n)) return `${o.label} must be a number.`;
      if (o.key === 'level' && !(n > 0 && n < 100)) return 'The confidence level must be between 0 and 100.';
    }
    if (o.type === 'checks' && o.min && (v as string[]).length < o.min) return `Choose at least ${o.min === 1 ? 'one' : o.min} under ${o.label}.`;
  }
  return null;
}

// ---------------------------------------------------------------- reopening a result's dialog

type Args = Record<string, unknown>;
const strs = (v: unknown): string[] => (Array.isArray(v) ? v.map(String) : typeof v === 'string' ? [v] : []);
const pairList = (v: unknown): [string, string][] => (Array.isArray(v) ? v.filter((p) => Array.isArray(p) && p.length === 2).map((p) => [String(p[0]), String(p[1])]) : []);
const factorNames = (v: unknown): string[] => (Array.isArray(v) ? v.map((f) => (typeof f === 'string' ? f : String((f as Args).name))) : []);
const factorRefs = (v: unknown): Record<string, string> =>
  Object.fromEntries((Array.isArray(v) ? v : []).filter((f) => f && typeof f === 'object' && (f as Args).reference != null).map((f) => [String((f as Args).name), String((f as Args).reference)]));

/** Which roles each command's arguments fill. Options are restored generically by key. */
const ROLE_ARGS: Record<string, (a: Args) => Partial<Collected>> = {
  crosstabs: (a) => ({ roles: { row: strs(a.row), column: strs(a.column), layer: a.layer ? strs(a.layer) : [] } }),
  ttest_one_sample: (a) => ({ roles: { test: strs(a.test_variables) } }),
  ttest_paired: (a) => ({ pairs: { pairs: pairList(a.pairs) } }),
  anova_oneway: (a) => ({ roles: { dependent: strs(a.dependent), factor: strs(a.factor) } }),
  correlation: (a) => ({ roles: { variables: strs(a.variables) } }),
  regression: (a) => {
    const blk = Array.isArray(a.blocks) && a.blocks.length ? (a.blocks[0] as Args) : {};
    return {
      roles: { dependent: strs(a.dependent), covariates: strs(a.covariates), factors: factorNames(a.factors), covariates2: strs(blk.covariates), factors2: factorNames(blk.factors) },
      references: { factors: factorRefs(a.factors), factors2: factorRefs(blk.factors) },
      options: {},
    };
  },
  mann_whitney: (a) => ({ roles: { test: strs(a.test_variables), group: strs(a.grouping_variable) }, groups: Array.isArray(a.groups) ? ({ group: [Number(a.groups[0]), Number(a.groups[1])] } as Record<string, [number, number]>) : {} }),
  wilcoxon: (a) => ({ pairs: { pairs: pairList(a.pairs) } }),
  kruskal: (a) => ({ roles: { dependent: strs(a.dependent), factor: strs(a.factor) } }),
  partial_correlation: (a) => ({ roles: { variables: strs(a.variables), controls: strs(a.controls) } }),
  mcnemar: (a) => ({ pairs: { pairs: pairList(a.pairs) } }),
  friedman: (a) => ({ roles: { variables: strs(a.variables) } }),
  anova_factorial: (a) => ({ roles: { dependent: strs(a.dependent), factors: strs(a.factors), covariates: strs(a.covariates) } }),
  sem: (a) => ({ roles: { group: a.group ? strs(a.group) : [], cluster: a.cluster ? strs(a.cluster) : [] } }),
  lca: (a) => ({ roles: { variables: strs(a.variables) } }),
  lpa: (a) => ({ roles: { indicators: strs(a.indicators) } }),
  forest: (a) => ({ roles: { dependent: strs(a.dependent), predictors: strs(a.predictors) } }),
  boost: (a) => ({ roles: { dependent: strs(a.dependent), predictors: strs(a.predictors), fold_variable: a.fold_variable ? strs(a.fold_variable) : [] } }),
  doe_mixture: (a) => ({ roles: { response: strs(a.response), components: strs(a.components) } }),
  doe_effects: (a) => ({ roles: { response: strs(a.response), factors: strs(a.factors) } }),
  rsm: (a) => ({ roles: { response: strs(a.response), factors: strs(a.factors), block: a.block ? strs(a.block) : [] } }),
  spc: (a) => {
    const chart = String(a.chart ?? 'xbar_r');
    const one = (k: string) => (typeof a[k] === 'string' ? [a[k] as string] : []);
    return {
      roles: {
        measures: chart === 'xbar_r' || chart === 'xbar_s' ? strs(a.measures) : [],
        smoothed: chart === 'cusum' || chart === 'ewma' ? strs(a.measures) : [],
        subgroup: one('subgroup'),
        single: chart === 'imr' ? strs(a.measures) : [],
        count: one('count'),
        size: one('size'),
      },
    };
  },
  tree: (a) => ({ roles: { dependent: strs(a.dependent), predictors: strs(a.predictors), fold_variable: a.fold_variable ? strs(a.fold_variable) : [] } }),
  penalized: (a) => ({ roles: { dependent: strs(a.dependent), covariates: strs(a.covariates), factors: factorNames(a.factors), fold_variable: a.fold_variable ? strs(a.fold_variable) : [] }, references: { factors: factorRefs(a.factors) } }),
  pgmm: (a) => ({ roles: { dependent: strs(a.dependent), covariates: strs(a.covariates), individual: strs(a.individual), time: strs(a.time) } }),
  panel: (a) => ({ roles: { dependent: strs(a.dependent), covariates: strs(a.covariates), factors: factorNames(a.factors), individual: strs(a.individual), time: strs(a.time) }, references: { factors: factorRefs(a.factors) } }),
  gee: (a) => ({ roles: { dependent: strs(a.dependent), covariates: strs(a.covariates), factors: factorNames(a.factors), cluster: strs(a.cluster), wave: a.wave ? strs(a.wave) : [] }, references: { factors: factorRefs(a.factors) } }),
  meta: (a) => {
    const one = (k: string) => (typeof a[k] === 'string' ? [a[k] as string] : []);
    const zcor = a.measure === 'zcor';
    return {
      roles: Object.fromEntries([
        ...['study', 'effect', 'variance', 'se', 'mean1', 'sd1', 'mean2', 'sd2', 'events1', 'events2', 'n2', 'r'].map((k) => [k, one(k)]),
        ['n1', zcor ? [] : one('n1')],
        ['n', zcor ? one('n1') : []],
        ['moderators', strs(a.moderators)],
        ['cluster', one('cluster')],
      ]),
    };
  },
  irt: (a) => ({ roles: { items: strs(a.items) } }),
  pls: (a) => ({ roles: { group: a.group ? strs(a.group) : [] } }),
  sem_invariance: (a) => ({ roles: { group: strs(a.group) } }),
  survey_estimates: (a) => ({ roles: { variables: strs(a.variables), weight: strs(a.weight), strata: a.strata ? strs(a.strata) : [], cluster: a.cluster ? strs(a.cluster) : [] } }),
  survey_regression: (a) => ({ roles: { dependent: strs(a.dependent), covariates: strs(a.covariates), factors: factorNames(a.factors), weight: strs(a.weight), strata: a.strata ? strs(a.strata) : [], cluster: a.cluster ? strs(a.cluster) : [] }, references: { factors: factorRefs(a.factors) } }),
  ts_acf: (a) => ({ roles: { variable: strs(a.variable) } }),
  ts_smooth: (a) => ({ roles: { variable: strs(a.variable) } }),
  arima: (a) => ({ roles: { variable: strs(a.variable) } }),
  kaplan_meier: (a) => ({ roles: { time: strs(a.time), status: strs(a.status), group: a.group ? strs(a.group) : [] } }),
  cox_regression: (a) => ({ roles: { time: strs(a.time), status: strs(a.status), covariates: strs(a.covariates), factors: factorNames(a.factors) }, references: { factors: factorRefs(a.factors) } }),
  roc: (a) => ({ roles: { variables: strs(a.variables), state: strs(a.state) } }),
  perm_two_groups: (a) => ({ roles: { test: strs(a.variables), group: strs(a.group) }, groups: Array.isArray(a.groups) ? ({ group: [Number(a.groups[0]), Number(a.groups[1])] } as Record<string, [number, number]>) : {} }),
  perm_paired: (a) => ({ roles: { first: strs(a.variables).slice(0, 1), second: strs(a.variables).slice(1, 2) } }),
  perm_correlation: (a) => ({ roles: { first: strs(a.variables).slice(0, 1), second: strs(a.variables).slice(1, 2) } }),
  perm_anova: (a) => ({ roles: { test: strs(a.variables), factor: strs(a.group) } }),
  missing_patterns: (a) => ({ roles: { variables: strs(a.variables) } }),
  mi_regression: (a) => ({ roles: { dependent: strs(a.dependent), covariates: strs(a.covariates), auxiliary: strs(a.auxiliary) } }),
  mixed_model: (a) => ({ roles: { dependent: strs(a.dependent), covariates: strs(a.covariates), factors: factorNames(a.factors), group: strs(a.group), random_slopes: strs(a.random_slopes) }, references: { factors: factorRefs(a.factors) } }),
  yuen: (a) => ({ roles: { test: strs(a.test_variables), group: strs(a.grouping_variable) }, groups: Array.isArray(a.groups) ? ({ group: [Number(a.groups[0]), Number(a.groups[1])] } as Record<string, [number, number]>) : {} }),
  yuen_paired: (a) => ({ pairs: { pairs: pairList(a.pairs) } }),
  trimmed_anova: (a) => ({ roles: { dependent: strs(a.dependent), factor: strs(a.factor) } }),
  ordinal_regression: (a) => ({ roles: { dependent: strs(a.dependent), covariates: strs(a.covariates), factors: factorNames(a.factors) }, references: { factors: factorRefs(a.factors) } }),
  multinomial_regression: (a) => ({ roles: { dependent: strs(a.dependent), covariates: strs(a.covariates), factors: factorNames(a.factors) }, references: { factors: factorRefs(a.factors), ...(a.reference != null ? { dependent: { [String(a.dependent)]: String(a.reference) } } : {}) } }),
  count_regression: (a) => ({ roles: { dependent: strs(a.dependent), covariates: strs(a.covariates), factors: factorNames(a.factors), exposure: a.exposure ? strs(a.exposure) : [] }, references: { factors: factorRefs(a.factors) } }),
  logistic: (a) => {
    const blk = Array.isArray(a.blocks) && a.blocks.length ? (a.blocks[0] as Args) : {};
    return {
      roles: { dependent: strs(a.dependent), covariates: strs(a.covariates), factors: factorNames(a.factors), covariates2: strs(blk.covariates), factors2: factorNames(blk.factors) },
      references: { factors: factorRefs(a.factors), factors2: factorRefs(blk.factors), ...(a.event != null ? { dependent: { [String(a.dependent)]: String(a.event) } } : {}) },
    };
  },
  reliability: (a) => ({ roles: { items: strs(a.items).filter((i) => !strs(a.reverse).includes(i)), reverse: strs(a.reverse) } }),
  canonical: (a) => ({ roles: { set1: strs(a.set1), set2: strs(a.set2) } }),
  chisq_gof: (a) => ({ roles: { variable: strs(a.variable) } }),
  binomial: (a) => ({ roles: { variables: strs(a.variables) } }),
  runs: (a) => ({ roles: { variables: strs(a.variables) } }),
  sign_test: (a) => ({ pairs: { pairs: pairList(a.pairs) } }),
  median_test: (a) => ({ roles: { dependent: strs(a.dependent), factor: strs(a.factor) } }),
  ks2: (a) => ({ roles: { test: strs(a.test_variables), group: strs(a.grouping_variable) }, groups: Array.isArray(a.groups) ? ({ group: [Number(a.groups[0]), Number(a.groups[1])] } as Record<string, [number, number]>) : {} }),
  cochran: (a) => ({ roles: { variables: strs(a.variables) } }),
  explore: (a) => ({ roles: { dependent: strs(a.dependent), factor: a.factor ? strs(a.factor) : [] } }),
  mediation: (a) => ({ roles: { x: strs(a.x), mediators: strs(a.mediators), y: strs(a.y), covariates: strs(a.covariates) } }),
  moderation: (a) => ({ roles: { x: strs(a.x), w: strs(a.w), y: strs(a.y), covariates: strs(a.covariates) } }),
  factor: (a) => ({ roles: { variables: strs(a.variables) } }),
  manova: (a) => ({ roles: { dependent: strs(a.dependent), factor: strs(a.factor) } }),
  rm_anova: (a) => ({ roles: { measures: strs(a.measures), between: a.between ? strs(a.between) : [] } }),
  bootstrap: (a) => ({ roles: { variables: strs(a.variables), dependent: a.dependent ? strs(a.dependent) : [] } }),
  curve: (a) => ({ roles: { dependent: strs(a.dependent), independent: strs(a.independent) } }),
  kappa: (a) => ({ roles: { rater1: strs(a.rater1), rater2: strs(a.rater2) } }),
  fleiss: (a) => ({ roles: { raters: strs(a.raters) } }),
  kripp: (a) => ({ roles: { raters: strs(a.raters) } }),
  icc: (a) => ({ roles: { raters: strs(a.raters) } }),
  chart_histogram: (a) => ({ roles: { variables: strs(a.variables) } }),
  chart_boxplot: (a) => ({ roles: { variables: strs(a.variables), group: a.group ? strs(a.group) : [] } }),
  chart_bar: (a) => ({ roles: { variable: strs(a.variables), mean_of: a.mean_of ? strs(a.mean_of) : [] } }),
  chart_scatter: (a) => ({ roles: { x: strs(a.x), y: strs(a.y) } }),
  chart_qq: (a) => ({ roles: { variables: strs(a.variables) } }),
};

/** The dialog state that would run `args` again: the inverse of each analysis's build. */
export function restore(def: AnalysisDef, args: Args): Collected {
  const r = ROLE_ARGS[def.id]?.(args) ?? {};
  const options: Record<string, OptionValue> = {};
  for (const o of def.options) {
    const v = args[o.key];
    if (v === undefined) {
      options[o.key] = optionDefault(o);
    } else if (o.key === 'level' && typeof v === 'number') {
      options[o.key] = String(Math.round(v * 1e6) / 1e4);
    } else if (o.key === 'contrasts' && Array.isArray(v)) {
      options[o.key] = v.filter(Array.isArray).map((row) => row.join(' ')).join('; ');
    } else if (o.key === 'interactions' && Array.isArray(v)) {
      options[o.key] = v.filter(Array.isArray).map((p) => `${p[0]}*${p[1]}`).join(', ');
    } else if (o.type === 'number' || o.type === 'text' || o.type === 'textarea') {
      options[o.key] = v == null ? '' : Array.isArray(v) ? v.join(', ') : String(v);
    } else if (o.type === 'toggle') {
      options[o.key] = Boolean(v);
    } else if (o.type === 'checks') {
      options[o.key] = strs(v);
    } else {
      options[o.key] = v == null ? o.default : String(v);
    }
  }
  return { roles: r.roles ?? {}, pairs: r.pairs ?? {}, groups: r.groups ?? {}, references: r.references ?? {}, options };
}
