// Help me choose: a few questions about the study's design that end at an analysis, with the reason it fits, the
// assumptions to check and the alternatives. Each ending names the variables it needs ("slots"); the panel lets the
// user pick them and opens the analysis's dialog with them already in their roles.
//
// Slot keys are shared across endings (outcome, group, first, second, ...) so that switching to an alternative keeps
// the variables already chosen.

import type { Measure } from './types';

export type Args = Record<string, unknown>;

export interface Answer {
  label: string;
  hint?: string;
  next: string;
}

export interface Question {
  id: string;
  ask: string;
  /** A sentence that helps the user answer. */
  help?: string;
  /** Offer the levels-of-measurement explainer under the answers. */
  levels?: boolean;
  answers: Answer[];
}

/** The levels of measurement, for the questions about what kind of variable something is. */
export const LEVELS: { level: string; means: string; examples: string }[] = [
  { level: 'Nominal', means: 'Categories with no order. Codes are only labels: 2 is not more than 1.', examples: 'major, treatment group, passed or failed' },
  { level: 'Ordinal', means: 'Categories in an order, but the steps between them need not be equal.', examples: 'class rank, education level, a single agree–disagree item' },
  { level: 'Interval', means: 'Numbers with equal steps, but zero is not “none”, so ratios mean nothing.', examples: 'temperature in °C, most test and scale scores' },
  { level: 'Ratio', means: 'Equal steps and a true zero, so twice as much means something.', examples: 'reaction time, income, age, number of absences' },
];

/** Why interval and ratio share an answer. */
export const LEVELS_NOTE = 'Interval and ratio scores lead to the same analyses, so they share one answer here; in Variable View both are “scale”. A total or mean of several agree–disagree items is usually treated as a score.';

export interface Slot {
  key: SlotKey;
  label: string;
  many?: boolean;
  /** numeric: only numeric variables can go here. */
  numeric?: boolean;
  /** The measure to list first. */
  prefer?: 'scale' | 'categorical';
}

export type SlotKey = 'outcome' | 'outcomes' | 'group' | 'factors' | 'first' | 'second' | 'measures' | 'predictors' | 'items' | 'raters' | 'variables' | 'controls' | 'time' | 'status' | 'mediator' | 'moderator' | 'set2';

export interface Ending {
  id: string;
  /** The dialog the ending opens (an analysis id, or ttest, frequencies, descriptives, power, cfa). */
  dialog: string;
  title: string;
  why: string;
  /** What to check in the output, and what to do if it fails. */
  check?: string;
  slots: Slot[];
  /** The dialog's starting arguments from the chosen variables (by slot). Missing slots are left for the dialog. */
  args?: (v: Picked) => Args;
  /** Endings to consider instead. */
  also?: string[];
  /** The dialog works without data (Power Analysis). */
  noData?: boolean;
}

/** The variables chosen for each slot, with their measures (to split predictors into numeric and categorical). */
export interface Picked {
  get: (key: SlotKey) => string[];
  measure: (name: string) => Measure | undefined;
}

const one = (v: Picked, k: SlotKey) => v.get(k)[0] ?? null;
const pair = (v: Picked) => (one(v, 'first') && one(v, 'second') ? [[one(v, 'first'), one(v, 'second')]] : []);
const numericPreds = (v: Picked) => v.get('predictors').filter((p) => v.measure(p) === 'scale');
const categoricalPreds = (v: Picked) => v.get('predictors').filter((p) => v.measure(p) !== 'scale').map((name) => ({ name, reference: null }));

const S = {
  outcome: { key: 'outcome', label: 'Outcome (the score you compare)', numeric: true, prefer: 'scale' },
  group: { key: 'group', label: 'Grouping variable', prefer: 'categorical' },
  first: { key: 'first', label: 'First measurement', numeric: true, prefer: 'scale' },
  second: { key: 'second', label: 'Second measurement', numeric: true, prefer: 'scale' },
  measures: { key: 'measures', label: 'The measurements, in order', many: true, numeric: true, prefer: 'scale' },
  predictors: { key: 'predictors', label: 'Predictors', many: true, prefer: 'scale' },
} satisfies Record<string, Slot>;

export const QUESTIONS: Question[] = [
  {
    id: 'start',
    ask: 'What do you want to find out?',
    answers: [
      { label: 'Whether groups or conditions differ', hint: 'such as treatment against control, or before against after', next: 'compare_outcome' },
      { label: 'Whether variables are related', hint: 'such as whether study hours go with exam scores', next: 'relate' },
      { label: 'How well some variables predict an outcome', hint: 'such as predicting a pass from attendance and grades', next: 'predict' },
      { label: 'Whether a scale or a set of ratings holds up', hint: 'questionnaire items, or agreement between raters', next: 'measure' },
      { label: 'What my data look like', hint: 'counts, averages and spread', next: 'describe' },
      { label: 'How many participants I need', hint: 'before collecting data', next: 'power' },
    ],
  },
  {
    id: 'compare_outcome',
    ask: 'What kind of outcome do you compare?',
    levels: true,
    answers: [
      { label: 'Categories with no order (nominal)', hint: 'such as passed or failed, or which option was chosen', next: 'compare_category' },
      { label: 'Ranked categories or ratings (ordinal)', hint: 'such as class rank, or a single 1–5 agreement item', next: 'compare_ordinal' },
      { label: 'Scores (interval or ratio)', hint: 'such as test scores, reaction times or income', next: 'compare_time' },
    ],
  },
  {
    id: 'compare_time',
    ask: 'Is the score the time until something happens, with some cases not reaching it by the end of the study?',
    help: 'Such as days until relapse, when some people had not relapsed when the study ended (they are “censored”). Ordinary tests treat their last day as the event time; survival analysis doesn’t.',
    answers: [
      { label: 'No', hint: 'an ordinary score, or a time every case reached', next: 'compare_design' },
      { label: 'Yes, some cases are censored', next: 'kaplan_meier' },
    ],
  },
  {
    id: 'compare_ordinal',
    ask: 'What are you comparing?',
    help: 'Ordinal outcomes are compared by ranks, which use only their order.',
    answers: [
      { label: 'Two groups of different people', next: 'mann_whitney' },
      { label: 'The same people measured twice', hint: 'or matched pairs', next: 'wilcoxon' },
      { label: 'Three or more groups of different people', next: 'kruskal' },
      { label: 'The same people measured three or more times', next: 'friedman' },
      { label: 'Groups formed by two or more factors, or with covariates', next: 'ordinal_regression' },
    ],
  },
  {
    id: 'compare_design',
    ask: 'What are you comparing?',
    help: '“Different people” means each case is in one group only. “The same people” means each case has a score in every condition, in separate columns.',
    answers: [
      { label: 'One group against a known value', hint: 'such as the class average against the national norm of 100', next: 'ttest_one_sample' },
      { label: 'Two groups of different people', next: 'shape_two_independent' },
      { label: 'The same people measured twice', hint: 'or matched pairs', next: 'shape_two_related' },
      { label: 'Three or more groups of different people', next: 'shape_k_independent' },
      { label: 'The same people measured three or more times', next: 'shape_k_related' },
      { label: 'Groups formed by two or three factors at once', hint: 'such as treatment by gender', next: 'anova_factorial' },
      { label: 'Groups on several outcomes at once', next: 'manova' },
    ],
  },
  ...(
    [
      ['shape_two_independent', 'ttest', 'mann_whitney'],
      ['shape_two_related', 'ttest_paired', 'wilcoxon'],
      ['shape_k_independent', 'anova_oneway', 'kruskal'],
      ['shape_k_related', 'rm_anova', 'friedman'],
    ] as const
  ).map(([id, means, ranks]) => ({
    id,
    ask: 'What is the outcome like?',
    help: 'Means work well for roughly symmetric scores, and for any shape once each group has about 30 cases. Ranks suit strong skew and extreme outliers.',
    answers: [
      { label: 'Roughly symmetric, or the groups are large', next: means },
      { label: 'Strongly skewed, or with extreme outliers', next: ranks },
      { label: 'Not sure', hint: 'start with the usual test; its output checks the assumptions', next: means },
    ],
  })),
  {
    id: 'compare_category',
    ask: 'How are the groups formed?',
    answers: [
      { label: 'Different people in each group', hint: 'such as pass rates in two classes', next: 'crosstabs_groups' },
      { label: 'The same people, a yes/no answered twice', hint: 'such as before and after a workshop', next: 'mcnemar' },
      { label: 'The same people, a yes/no answered three or more times', next: 'cochran' },
      { label: 'One group: a yes/no against an expected proportion', hint: 'such as whether more than half agree', next: 'binomial' },
      { label: 'One group: several categories against expected shares', next: 'chisq_gof' },
    ],
  },
  {
    id: 'relate',
    ask: 'What kind of variables?',
    levels: true,
    answers: [
      { label: 'Scores (interval or ratio)', hint: 'such as study hours and exam scores', next: 'relate_shape' },
      { label: 'Ranked categories or ratings (ordinal)', hint: 'or one ordinal variable and one score', next: 'spearman' },
      { label: 'Categories with no order (nominal)', hint: 'such as major by preferred study method', next: 'crosstabs_relate' },
      { label: 'Two scores, holding others constant', hint: 'partial correlation', next: 'partial_correlation' },
      { label: 'Two sets of scores', hint: 'such as three ability tests and two achievement tests', next: 'canonical' },
      { label: 'Whether an effect works through a third variable', hint: 'mediation', next: 'mediation' },
      { label: 'Whether a relationship depends on a third variable', hint: 'moderation, or an interaction', next: 'moderation' },
    ],
  },
  {
    id: 'relate_shape',
    ask: 'What are the scores like?',
    answers: [
      { label: 'Roughly symmetric, related in a straight line', next: 'pearson' },
      { label: 'Strongly skewed, or related in a curve that only rises or only falls', next: 'spearman' },
    ],
  },
  {
    id: 'predict',
    ask: 'What kind of outcome do you predict?',
    levels: true,
    answers: [
      { label: 'Two categories (nominal)', hint: 'such as passed or failed', next: 'logistic' },
      { label: 'Three or more categories with no order (nominal)', hint: 'such as which of four programs was chosen', next: 'multinomial_regression' },
      { label: 'Ranked categories (ordinal)', hint: 'such as low, medium and high', next: 'ordinal_regression' },
      { label: 'Scores (interval or ratio)', next: 'predict_kind' },
    ],
  },
  {
    id: 'predict_kind',
    ask: 'Is the score one of these?',
    help: 'Counts and censored times are ratio scores, but they need models of their own.',
    answers: [
      { label: 'No, an ordinary score', next: 'predict_score' },
      { label: 'A count of events', hint: 'such as the number of absences: whole numbers, often many zeros', next: 'count_regression' },
      { label: 'The time until something happens, with some cases not reaching it', hint: 'censored cases', next: 'cox_regression' },
    ],
  },
  {
    id: 'predict_score',
    ask: 'How are the cases organized?',
    answers: [
      { label: 'One row per person, people independent of one another', next: 'regression' },
      { label: 'In clusters or measured repeatedly', hint: 'such as students within classes, or several visits per patient', next: 'mixed_model' },
      { label: 'I mostly want accurate predictions from many predictors', next: 'forest' },
    ],
  },
  {
    id: 'measure',
    ask: 'What do you want to check?',
    answers: [
      { label: 'Whether a scale’s items hang together', hint: 'internal consistency', next: 'reliability' },
      { label: 'Which factors underlie a set of items', hint: 'exploring', next: 'factor' },
      { label: 'Whether items fit a factor structure you expect', hint: 'confirming', next: 'cfa' },
      { label: 'Agreement of two raters on categories', next: 'kappa' },
      { label: 'Agreement of three or more raters on categories', next: 'fleiss' },
      { label: 'Agreement of raters giving scores', next: 'icc' },
    ],
  },
  {
    id: 'describe',
    ask: 'What kind of variables?',
    levels: true,
    answers: [
      { label: 'Categories (nominal or ordinal)', hint: 'counts and percentages', next: 'frequencies' },
      { label: 'Scores (interval or ratio)', hint: 'mean, standard deviation, range', next: 'descriptives' },
      { label: 'Scores, by group, with plots and a check of their shape', next: 'explore' },
    ],
  },
  {
    id: 'power',
    ask: 'Which analysis will you run on the data?',
    answers: [
      { label: 'Independent-samples t test (two groups)', next: 'power_independent' },
      { label: 'Paired-samples t test', next: 'power_paired' },
      { label: 'One-sample t test', next: 'power_one_sample' },
      { label: 'One-way ANOVA', next: 'power_anova' },
      { label: 'Pearson correlation', next: 'power_correlation' },
      { label: 'Two proportions', next: 'power_two_proportions' },
      { label: 'One proportion', next: 'power_one_proportion' },
    ],
  },
];

const twoGroupArgs = (v: Picked) => ({ test_variables: v.get('outcome'), grouping_variable: one(v, 'group') });
const kGroupArgs = (v: Picked) => ({ dependent: v.get('outcome'), factor: one(v, 'group') });
const POWER_TESTS: [string, string][] = [
  ['independent', 'Independent-samples t test'],
  ['paired', 'Paired-samples t test'],
  ['one_sample', 'One-sample t test'],
  ['anova', 'One-way ANOVA'],
  ['correlation', 'Pearson correlation'],
  ['two_proportions', 'Two proportions'],
  ['one_proportion', 'One proportion'],
];

export const ENDINGS: Ending[] = [
  // Comparing scores
  {
    id: 'ttest_one_sample',
    dialog: 'ttest_one_sample',
    title: 'One-Sample T Test',
    why: 'It tests whether the mean of one group differs from a value you give, such as a norm or a midpoint.',
    check: 'Roughly symmetric scores, or about 30 cases or more.',
    slots: [{ ...S.outcome, label: 'Outcome' }],
    args: (v) => ({ test_variables: v.get('outcome') }),
  },
  {
    id: 'ttest',
    dialog: 'ttest',
    title: 'Independent-Samples T Test',
    why: 'It compares the means of two groups of different people. The Welch version, the default, doesn’t assume the groups have equal variances.',
    check: 'Roughly symmetric scores in each group, or about 30 cases per group. With strong skew or outliers, use the Mann-Whitney U test or Yuen’s test.',
    slots: [S.outcome, S.group],
    args: twoGroupArgs,
    also: ['mann_whitney', 'yuen', 'perm_two_groups'],
  },
  {
    id: 'mann_whitney',
    dialog: 'mann_whitney',
    title: 'Mann-Whitney U Test',
    why: 'It compares two groups of different people by ranks, so skew, outliers and ordinal ratings don’t distort it.',
    slots: [S.outcome, S.group],
    args: twoGroupArgs,
    also: ['ttest', 'yuen', 'ks2'],
  },
  {
    id: 'yuen',
    dialog: 'yuen',
    title: 'Yuen’s Test (Trimmed Means)',
    why: 'It compares the trimmed means of two groups, which resist outliers while still comparing typical scores.',
    slots: [S.outcome, S.group],
    args: twoGroupArgs,
    also: ['ttest', 'mann_whitney'],
  },
  {
    id: 'perm_two_groups',
    dialog: 'perm_two_groups',
    title: 'Permutation Test: Two Groups',
    why: 'It compares two groups by reshuffling the group labels, so it needs no assumption about the shape of the scores. Useful for small samples.',
    slots: [S.outcome, S.group],
    args: (v) => ({ variables: v.get('outcome'), group: one(v, 'group') }),
    also: ['ttest', 'mann_whitney'],
  },
  {
    id: 'ks2',
    dialog: 'ks2',
    title: 'Kolmogorov-Smirnov Test (Two Samples)',
    why: 'It tests whether two groups’ whole distributions differ, in spread or shape as well as location.',
    slots: [S.outcome, S.group],
    args: twoGroupArgs,
    also: ['mann_whitney'],
  },
  {
    id: 'ttest_paired',
    dialog: 'ttest_paired',
    title: 'Paired-Samples T Test',
    why: 'It tests whether the mean difference between two measurements of the same people is zero.',
    check: 'Roughly symmetric differences, or about 30 pairs. With skewed differences or outliers, use the Wilcoxon signed-rank test.',
    slots: [S.first, S.second],
    args: (v) => ({ pairs: pair(v) }),
    also: ['wilcoxon', 'yuen_paired', 'perm_paired'],
  },
  {
    id: 'wilcoxon',
    dialog: 'wilcoxon',
    title: 'Wilcoxon Signed-Rank Test',
    why: 'It compares two measurements of the same people by the ranks of their differences, so skew and outliers don’t distort it.',
    slots: [S.first, S.second],
    args: (v) => ({ pairs: pair(v) }),
    also: ['ttest_paired', 'sign_test'],
  },
  {
    id: 'sign_test',
    dialog: 'sign_test',
    title: 'Sign Test',
    why: 'It counts how many people went up and how many went down, ignoring by how much. It assumes the least of the paired tests.',
    slots: [S.first, S.second],
    args: (v) => ({ pairs: pair(v) }),
    also: ['wilcoxon'],
  },
  {
    id: 'yuen_paired',
    dialog: 'yuen_paired',
    title: 'Paired Yuen Test',
    why: 'It compares the trimmed means of two measurements of the same people, resisting outliers.',
    slots: [S.first, S.second],
    args: (v) => ({ pairs: pair(v) }),
    also: ['ttest_paired', 'wilcoxon'],
  },
  {
    id: 'perm_paired',
    dialog: 'perm_paired',
    title: 'Permutation Test: Paired Measures',
    why: 'It tests the mean difference by randomly flipping the sign of each difference, with no assumption about their shape.',
    slots: [S.first, S.second],
    args: (v) => ({ variables: [...v.get('first').slice(0, 1), ...v.get('second').slice(0, 1)] }),
    also: ['ttest_paired', 'wilcoxon'],
  },
  {
    id: 'anova_oneway',
    dialog: 'anova_oneway',
    title: 'One-Way ANOVA',
    why: 'It tests whether the means of three or more groups of different people differ, with Tukey or Games-Howell comparisons to find which groups do.',
    check: 'Roughly symmetric scores in each group and similar spreads (the output tests equal variances and gives Welch’s F if they differ).',
    slots: [S.outcome, S.group],
    args: kGroupArgs,
    also: ['kruskal', 'trimmed_anova', 'perm_anova'],
  },
  {
    id: 'kruskal',
    dialog: 'kruskal',
    title: 'Kruskal-Wallis Test',
    why: 'It compares three or more groups of different people by ranks, with Dunn’s comparisons between pairs of groups.',
    slots: [S.outcome, S.group],
    args: kGroupArgs,
    also: ['anova_oneway', 'median_test'],
  },
  {
    id: 'trimmed_anova',
    dialog: 'trimmed_anova',
    title: 'Robust ANOVA (Trimmed Means)',
    why: 'It compares the trimmed means of three or more groups, resisting outliers and unequal variances.',
    slots: [S.outcome, S.group],
    args: kGroupArgs,
    also: ['anova_oneway', 'kruskal'],
  },
  {
    id: 'perm_anova',
    dialog: 'perm_anova',
    title: 'Permutation Test: One-Way ANOVA',
    why: 'It tests the F statistic by reshuffling the group labels, with no assumption about the shape of the scores.',
    slots: [S.outcome, S.group],
    args: (v) => ({ variables: v.get('outcome'), group: one(v, 'group') }),
    also: ['anova_oneway'],
  },
  {
    id: 'median_test',
    dialog: 'median_test',
    title: 'Median Test',
    why: 'It tests whether the groups have the same median by counting the cases above and below the overall median.',
    slots: [S.outcome, S.group],
    args: kGroupArgs,
    also: ['kruskal'],
  },
  {
    id: 'rm_anova',
    dialog: 'rm_anova',
    title: 'Repeated-Measures ANOVA',
    why: 'It tests whether the means of three or more measurements of the same people differ, with a sphericity check and corrections.',
    check: 'Each measurement in its own column, in order. If people have missing measurements, a linear mixed model uses the ones they have.',
    slots: [S.measures, { ...S.group, label: 'Between-subjects factor (optional)' }],
    args: (v) => ({ measures: v.get('measures'), between: one(v, 'group') }),
    also: ['friedman', 'mixed_model'],
  },
  {
    id: 'friedman',
    dialog: 'friedman',
    title: 'Friedman Test',
    why: 'It compares three or more measurements of the same people by ranking each person’s scores.',
    slots: [S.measures],
    args: (v) => ({ variables: v.get('measures') }),
    also: ['rm_anova'],
  },
  {
    id: 'anova_factorial',
    dialog: 'anova_factorial',
    title: 'Factorial ANOVA',
    why: 'It tests the main effect of each factor and their interaction, such as whether a treatment works differently for different groups.',
    check: 'Roughly symmetric scores and similar spreads in each cell. Unequal cell sizes are handled by Type III sums of squares.',
    slots: [S.outcome, { key: 'factors', label: 'Factors (two or three)', many: true, prefer: 'categorical' }],
    args: (v) => ({ dependent: one(v, 'outcome'), factors: v.get('factors') }),
    also: ['regression'],
  },
  {
    id: 'manova',
    dialog: 'manova',
    title: 'MANOVA',
    why: 'It tests whether groups differ on several outcomes taken together, before looking at each outcome on its own.',
    slots: [{ key: 'outcomes', label: 'Outcomes (two or more)', many: true, numeric: true, prefer: 'scale' }, S.group],
    args: (v) => ({ dependent: v.get('outcomes'), factor: one(v, 'group') }),
    also: ['anova_oneway'],
  },
  {
    id: 'kaplan_meier',
    dialog: 'kaplan_meier',
    title: 'Kaplan-Meier Survival',
    why: 'It estimates the share of cases that have not yet had the event at each time, counting cases that left the study early (censored) correctly, and compares groups with the log-rank test.',
    check: 'A status variable that says whether the event happened (by default, 1 means it did).',
    slots: [
      { key: 'time', label: 'Time', numeric: true, prefer: 'scale' },
      { key: 'status', label: 'Status (event or censored)', numeric: true, prefer: 'categorical' },
      { ...S.group, label: 'Groups to compare (optional)' },
    ],
    args: (v) => ({ time: one(v, 'time'), status: one(v, 'status'), group: one(v, 'group') }),
    also: ['cox_regression'],
  },
  // Comparing categories
  {
    id: 'crosstabs_groups',
    dialog: 'crosstabs',
    title: 'Crosstabs with Chi-Square',
    why: 'It tables the outcome by group and tests whether the outcome’s categories are equally common in every group. With small expected counts in a 2 × 2 table, use Fisher’s exact test (an option in the dialog).',
    slots: [
      { ...S.group, label: 'Groups' },
      { key: 'outcome', label: 'Outcome (a category)', prefer: 'categorical' },
    ],
    args: (v) => ({ row: one(v, 'group'), column: one(v, 'outcome') }),
    also: ['logistic'],
  },
  {
    id: 'mcnemar',
    dialog: 'mcnemar',
    title: 'McNemar Test',
    why: 'It tests whether a yes/no answer changed between two occasions for the same people, using only the people who changed.',
    slots: [
      { key: 'first', label: 'First occasion', numeric: true, prefer: 'categorical' },
      { key: 'second', label: 'Second occasion', numeric: true, prefer: 'categorical' },
    ],
    args: (v) => ({ pairs: pair(v) }),
  },
  {
    id: 'cochran',
    dialog: 'cochran',
    title: 'Cochran’s Q Test',
    why: 'It tests whether the share answering yes is the same across three or more occasions for the same people.',
    slots: [{ key: 'measures', label: 'The yes/no measures', many: true, numeric: true, prefer: 'categorical' }],
    args: (v) => ({ variables: v.get('measures') }),
  },
  {
    id: 'binomial',
    dialog: 'binomial',
    title: 'Binomial Test',
    why: 'It tests whether the proportion in a yes/no variable differs from a proportion you give (0.5 unless changed).',
    slots: [{ key: 'variables', label: 'The yes/no variable', many: true, prefer: 'categorical' }],
    args: (v) => ({ variables: v.get('variables') }),
  },
  {
    id: 'chisq_gof',
    dialog: 'chisq_gof',
    title: 'Chi-Square Goodness of Fit',
    why: 'It tests whether the categories of one variable occur in the shares you expect (equal shares unless you give others).',
    slots: [{ key: 'variables', label: 'The categorical variable', prefer: 'categorical' }],
    args: (v) => ({ variable: one(v, 'variables') }),
  },
  // Relationships
  {
    id: 'pearson',
    dialog: 'correlation',
    title: 'Pearson Correlation',
    why: 'It measures how closely two scores follow a straight line, from −1 to 1, and tests whether the relationship is zero.',
    check: 'Look at a scatterplot first: a curve or a few extreme points can mislead.',
    slots: [{ key: 'variables', label: 'Variables (two or more)', many: true, numeric: true, prefer: 'scale' }],
    args: (v) => ({ variables: v.get('variables'), coefficients: ['pearson'] }),
    also: ['spearman', 'perm_correlation'],
  },
  {
    id: 'spearman',
    dialog: 'correlation',
    title: 'Spearman Correlation',
    why: 'It correlates the ranks of the scores, so it suits ordinal ratings, skew, and relationships that rise or fall without being straight.',
    slots: [{ key: 'variables', label: 'Variables (two or more)', many: true, numeric: true, prefer: 'scale' }],
    args: (v) => ({ variables: v.get('variables'), coefficients: ['spearman'] }),
    also: ['pearson'],
  },
  {
    id: 'perm_correlation',
    dialog: 'perm_correlation',
    title: 'Permutation Test: Correlation',
    why: 'It tests a correlation by reshuffling one variable against the other, with no assumption of normality. Useful for small samples.',
    slots: [S.first, S.second],
    args: (v) => ({ variables: [...v.get('first').slice(0, 1), ...v.get('second').slice(0, 1)] }),
    also: ['pearson'],
  },
  {
    id: 'partial_correlation',
    dialog: 'partial_correlation',
    title: 'Partial Correlation',
    why: 'It correlates two scores after removing what both share with the control variables.',
    slots: [
      { key: 'variables', label: 'Variables to correlate', many: true, numeric: true, prefer: 'scale' },
      { key: 'controls', label: 'Controlling for', many: true, numeric: true, prefer: 'scale' },
    ],
    args: (v) => ({ variables: v.get('variables'), controls: v.get('controls') }),
    also: ['regression'],
  },
  {
    id: 'crosstabs_relate',
    dialog: 'crosstabs',
    title: 'Crosstabs with Chi-Square',
    why: 'It tables one categorical variable by the other and tests whether they are independent, with Cramér’s V for the strength of the association.',
    slots: [
      { key: 'first', label: 'Rows', prefer: 'categorical' },
      { key: 'second', label: 'Columns', prefer: 'categorical' },
    ],
    args: (v) => ({ row: one(v, 'first'), column: one(v, 'second') }),
  },
  {
    id: 'canonical',
    dialog: 'canonical',
    title: 'Canonical Correlation',
    why: 'It finds the pairs of weighted sums, one from each set, that correlate most strongly.',
    slots: [
      { key: 'variables', label: 'First set', many: true, numeric: true, prefer: 'scale' },
      { key: 'set2', label: 'Second set', many: true, numeric: true, prefer: 'scale' },
    ],
    args: (v) => ({ set1: v.get('variables'), set2: v.get('set2') }),
  },
  {
    id: 'mediation',
    dialog: 'mediation',
    title: 'Mediation',
    why: 'It splits the effect of X on Y into the part that runs through the mediator (the indirect effect, with a bootstrap interval) and the rest.',
    check: 'Mediation is a causal claim: the design, not the test, has to justify the order X, then mediator, then Y.',
    slots: [
      { key: 'predictors', label: 'X (predictor)', numeric: true, prefer: 'scale' },
      { key: 'mediator', label: 'Mediator', many: true, numeric: true, prefer: 'scale' },
      { key: 'outcome', label: 'Y (outcome)', numeric: true, prefer: 'scale' },
    ],
    args: (v) => ({ x: one(v, 'predictors'), mediators: v.get('mediator'), y: one(v, 'outcome') }),
    also: ['moderation'],
  },
  {
    id: 'moderation',
    dialog: 'moderation',
    title: 'Moderation',
    why: 'It tests whether the effect of X on Y changes with the moderator W (their interaction), and probes the effect at chosen values of W.',
    slots: [
      { key: 'predictors', label: 'X (predictor)', numeric: true, prefer: 'scale' },
      { key: 'moderator', label: 'W (moderator)', numeric: true },
      { key: 'outcome', label: 'Y (outcome)', numeric: true, prefer: 'scale' },
    ],
    args: (v) => ({ x: one(v, 'predictors'), w: one(v, 'moderator'), y: one(v, 'outcome') }),
    also: ['mediation', 'regression'],
  },
  // Prediction
  {
    id: 'regression',
    dialog: 'regression',
    title: 'Linear Regression',
    why: 'It predicts a score from one or more predictors and tests each predictor while holding the others constant.',
    check: 'The residual plots: a straight-line pattern and an even spread. Scale predictors are entered as numbers and the others as categories; change them in the dialog.',
    slots: [{ ...S.outcome, label: 'Outcome' }, S.predictors],
    args: (v) => ({ dependent: one(v, 'outcome'), covariates: numericPreds(v), factors: categoricalPreds(v) }),
    also: ['mixed_model', 'forest'],
  },
  {
    id: 'mixed_model',
    dialog: 'mixed_model',
    title: 'Linear Mixed Model',
    why: 'It predicts a score when cases come in clusters (students in classes) or repeated measurements, letting each cluster have its own baseline.',
    check: 'The data in long form: one row per measurement, with a variable naming the person or cluster. Data → Restructure makes it.',
    slots: [{ ...S.outcome, label: 'Outcome' }, S.predictors, { ...S.group, label: 'Cluster or person' }],
    args: (v) => ({ dependent: one(v, 'outcome'), covariates: numericPreds(v), factors: categoricalPreds(v), group: one(v, 'group') }),
    also: ['regression', 'gee'],
  },
  {
    id: 'gee',
    dialog: 'gee',
    title: 'Generalized Estimating Equations',
    why: 'It estimates population-average effects for clustered or repeated data, for scores, yes/no outcomes and counts.',
    slots: [{ ...S.outcome, label: 'Outcome' }, S.predictors, { ...S.group, label: 'Cluster or person' }],
    args: (v) => ({ dependent: one(v, 'outcome'), covariates: numericPreds(v), factors: categoricalPreds(v), cluster: one(v, 'group') }),
    also: ['mixed_model'],
  },
  {
    id: 'forest',
    dialog: 'forest',
    title: 'Random Forest',
    why: 'It averages many decision trees and usually predicts well with little tuning, reporting cross-validated accuracy and which predictors matter.',
    check: 'It predicts rather than explains: it gives no coefficients or p values.',
    slots: [{ key: 'outcome', label: 'Outcome', prefer: 'scale' }, S.predictors],
    args: (v) => ({ dependent: one(v, 'outcome'), predictors: v.get('predictors') }),
    also: ['tree', 'boost', 'penalized'],
  },
  {
    id: 'tree',
    dialog: 'tree',
    title: 'Decision Tree',
    why: 'It splits the cases into groups by simple rules that are easy to read and explain.',
    slots: [{ key: 'outcome', label: 'Outcome', prefer: 'scale' }, S.predictors],
    args: (v) => ({ dependent: one(v, 'outcome'), predictors: v.get('predictors') }),
    also: ['forest'],
  },
  {
    id: 'boost',
    dialog: 'boost',
    title: 'Gradient Boosting',
    why: 'It builds trees one after another, each correcting the last. Often the most accurate, given some tuning.',
    slots: [{ key: 'outcome', label: 'Outcome', prefer: 'scale' }, S.predictors],
    args: (v) => ({ dependent: one(v, 'outcome'), predictors: v.get('predictors') }),
    also: ['forest'],
  },
  {
    id: 'penalized',
    dialog: 'penalized',
    title: 'Penalized Regression',
    why: 'Lasso, ridge or elastic net: a regression that shrinks the coefficients to predict well with many predictors, choosing the amount by cross-validation.',
    slots: [{ ...S.outcome, label: 'Outcome' }, S.predictors],
    args: (v) => ({ dependent: one(v, 'outcome'), covariates: numericPreds(v), factors: categoricalPreds(v) }),
    also: ['regression', 'forest'],
  },
  {
    id: 'logistic',
    dialog: 'logistic',
    title: 'Binary Logistic Regression',
    why: 'It predicts which of two categories a case falls in, giving each predictor’s odds ratio.',
    slots: [{ key: 'outcome', label: 'Outcome (two categories)', prefer: 'categorical' }, S.predictors],
    args: (v) => ({ dependent: one(v, 'outcome'), covariates: numericPreds(v), factors: categoricalPreds(v) }),
    also: ['tree', 'roc'],
  },
  {
    id: 'roc',
    dialog: 'roc',
    title: 'ROC Curve',
    why: 'It shows how well a score separates two groups at every cutoff, with the area under the curve.',
    slots: [
      { key: 'predictors', label: 'Score or predicted probability', many: true, numeric: true, prefer: 'scale' },
      { key: 'outcome', label: 'Actual state (two categories)', prefer: 'categorical' },
    ],
    args: (v) => ({ variables: v.get('predictors'), state: one(v, 'outcome') }),
    also: ['logistic'],
  },
  {
    id: 'ordinal_regression',
    dialog: 'ordinal_regression',
    title: 'Ordinal Logistic Regression',
    why: 'It predicts ordered categories, using their order without treating the gaps between them as equal.',
    check: 'The proportional-odds assumption: each predictor has the same effect at every cut between categories.',
    slots: [{ key: 'outcome', label: 'Outcome (ordered categories)', prefer: 'categorical' }, S.predictors],
    args: (v) => ({ dependent: one(v, 'outcome'), covariates: numericPreds(v), factors: categoricalPreds(v) }),
    also: ['multinomial_regression'],
  },
  {
    id: 'multinomial_regression',
    dialog: 'multinomial_regression',
    title: 'Multinomial Logistic Regression',
    why: 'It predicts which of three or more unordered categories a case falls in, comparing each with a reference category.',
    slots: [{ key: 'outcome', label: 'Outcome (categories)', prefer: 'categorical' }, S.predictors],
    args: (v) => ({ dependent: one(v, 'outcome'), covariates: numericPreds(v), factors: categoricalPreds(v) }),
    also: ['ordinal_regression'],
  },
  {
    id: 'count_regression',
    dialog: 'count_regression',
    title: 'Poisson and Negative Binomial Regression',
    why: 'It predicts a count (0, 1, 2, …). The negative binomial model allows more spread than Poisson, which counts usually have.',
    slots: [{ ...S.outcome, label: 'Outcome (a count)' }, S.predictors],
    args: (v) => ({ dependent: one(v, 'outcome'), covariates: numericPreds(v), factors: categoricalPreds(v) }),
  },
  {
    id: 'cox_regression',
    dialog: 'cox_regression',
    title: 'Cox Regression',
    why: 'It predicts the time until an event from several predictors, counting censored cases correctly, and gives each predictor’s hazard ratio.',
    slots: [
      { key: 'time', label: 'Time', numeric: true, prefer: 'scale' },
      { key: 'status', label: 'Status (event or censored)', numeric: true, prefer: 'categorical' },
      S.predictors,
    ],
    args: (v) => ({ time: one(v, 'time'), status: one(v, 'status'), covariates: numericPreds(v), factors: categoricalPreds(v) }),
    also: ['kaplan_meier'],
  },
  // Measurement
  {
    id: 'reliability',
    dialog: 'reliability',
    title: 'Reliability Analysis',
    why: 'It gives Cronbach’s alpha and omega for a scale’s items, with what alpha would be if each item were dropped.',
    check: 'Reverse-worded items go in the dialog’s Reverse-scored list.',
    slots: [{ key: 'items', label: 'The scale’s items', many: true, numeric: true }],
    args: (v) => ({ items: v.get('items') }),
    also: ['factor', 'cfa'],
  },
  {
    id: 'factor',
    dialog: 'factor',
    title: 'Factor Analysis',
    why: 'It finds a few underlying factors that account for the correlations among many items, with a parallel analysis to suggest how many.',
    slots: [{ key: 'items', label: 'Items (three or more)', many: true, numeric: true }],
    args: (v) => ({ variables: v.get('items') }),
    also: ['cfa', 'reliability'],
  },
  {
    id: 'cfa',
    dialog: 'cfa',
    title: 'Factor Analysis of a Scale (CFA)',
    why: 'It tests whether the items fit the factors you expect them to measure, with fit indices such as CFI and RMSEA. You assign the items to factors in the dialog.',
    slots: [],
    also: ['factor', 'reliability'],
  },
  {
    id: 'kappa',
    dialog: 'kappa',
    title: 'Cohen’s Kappa',
    why: 'It measures the agreement of two raters beyond what chance would give. Weighted kappa credits near misses for ordered categories.',
    slots: [
      { key: 'first', label: 'Rater 1', numeric: true, prefer: 'categorical' },
      { key: 'second', label: 'Rater 2', numeric: true, prefer: 'categorical' },
    ],
    args: (v) => ({ rater1: one(v, 'first'), rater2: one(v, 'second') }),
    also: ['kripp'],
  },
  {
    id: 'fleiss',
    dialog: 'fleiss',
    title: 'Fleiss’ Kappa',
    why: 'It measures the agreement of three or more raters on categories, beyond chance.',
    slots: [{ key: 'raters', label: 'Raters (one variable each)', many: true, numeric: true, prefer: 'categorical' }],
    args: (v) => ({ raters: v.get('raters') }),
    also: ['kripp'],
  },
  {
    id: 'kripp',
    dialog: 'kripp',
    title: 'Krippendorff’s Alpha',
    why: 'It measures agreement for any number of raters and any level of measurement, and allows missing ratings.',
    slots: [{ key: 'raters', label: 'Raters (one variable each)', many: true, numeric: true }],
    args: (v) => ({ raters: v.get('raters') }),
  },
  {
    id: 'icc',
    dialog: 'icc',
    title: 'Intraclass Correlation',
    why: 'It measures how consistently raters score the same cases, for numeric ratings.',
    slots: [{ key: 'raters', label: 'Raters (one variable each)', many: true, numeric: true, prefer: 'scale' }],
    args: (v) => ({ raters: v.get('raters') }),
    also: ['kripp'],
  },
  // Describing
  {
    id: 'frequencies',
    dialog: 'frequencies',
    title: 'Frequencies',
    why: 'It counts each category, with percentages and valid percentages that leave out missing values.',
    slots: [{ key: 'variables', label: 'Variables', many: true, prefer: 'categorical' }],
    args: (v) => ({ variables: v.get('variables') }),
    also: ['chisq_gof'],
  },
  {
    id: 'descriptives',
    dialog: 'descriptives',
    title: 'Descriptives',
    why: 'It gives the mean, standard deviation, minimum and maximum of each score.',
    slots: [{ key: 'variables', label: 'Variables', many: true, numeric: true, prefer: 'scale' }],
    args: (v) => ({ variables: v.get('variables') }),
    also: ['explore'],
  },
  {
    id: 'explore',
    dialog: 'explore',
    title: 'Explore',
    why: 'It describes each score overall or by group, with percentiles, normality tests, box plots and Q-Q plots, which is the way to check a test’s assumptions before running it.',
    slots: [{ key: 'outcomes', label: 'Scores', many: true, numeric: true, prefer: 'scale' }, { ...S.group, label: 'By group (optional)' }],
    args: (v) => ({ dependent: v.get('outcomes'), factor: one(v, 'group') }),
    also: ['descriptives'],
  },
  // Planning
  ...POWER_TESTS.map(([test, name]) => ({
    id: `power_${test}`,
    dialog: 'power',
    title: `Power Analysis: ${name}`,
    why: 'It finds the sample size that gives the power you want for the effect you expect, or the power of a sample size you already have. It needs no data.',
    check: 'The expected effect size: take it from earlier studies or the smallest effect that would matter, not from a pilot’s estimate alone.',
    slots: [],
    args: () => ({ test }),
    noData: true,
  })),
];

export const NODES: Record<string, Question | Ending> = Object.fromEntries([...QUESTIONS, ...ENDINGS].map((n) => [n.id, n]));

export const isEnding = (n: Question | Ending): n is Ending => 'dialog' in n;

/** The dialog's starting arguments: nulls and empty lists left out, so the dialog's own defaults apply. */
export function startArgs(e: Ending, chosen: Partial<Record<SlotKey, string[]>>, measure: (name: string) => Measure | undefined): Args {
  const visible = new Set(e.slots.map((s) => s.key));
  const picked: Picked = { get: (k) => (visible.has(k) ? chosen[k] ?? [] : []), measure };
  const raw = e.args?.(picked) ?? {};
  return Object.fromEntries(Object.entries(raw).filter(([, v]) => v != null && !(Array.isArray(v) && v.length === 0)));
}
