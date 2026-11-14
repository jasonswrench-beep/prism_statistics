// The glossary: plain-language definitions of the terms in the output and the help. Help › Glossary lists them, and
// Explain this result (explain.ts) uses the same entries to read the numbers of one result. An entry with `match`
// is found in result tables, by a column heading or by a row's label, and its `read` turns this result's value into
// a sentence.

export interface Term {
  id: string;
  term: string;
  /** Other names and symbols it goes by. */
  also?: string[];
  definition: string;
  /** Headings or row labels in result tables that show this statistic. */
  match?: RegExp;
  /** Where in a table it is found: a column heading (default) or a row label (effect-size tables list measures by row). */
  where?: 'column' | 'row' | 'both';
  /** One sentence about this result's value. */
  read?: (v: number) => string;
  /** The order Explain this result lists it in: test statistics, then p, then effect sizes, then the rest. */
  rank?: number;
}

const two = (x: number) => fmt(x, 2);
export function fmt(x: number, d: number): string {
  const s = Math.abs(x) < 0.5 * 10 ** -d ? (0).toFixed(d) : x.toFixed(d);
  return s.replace('-', '−');
}
/** APA style: no leading zero for statistics that can't exceed 1. */
export const noLead = (x: number, d = 2) => fmt(x, d).replace(/^(−?)0\./, '$1.');
const pct = (x: number) => `${fmt(100 * x, 1)}%`;

export function pText(p: number): string {
  return p < 0.001 ? '< .001' : `= ${noLead(p, 3)}`;
}

export const GLOSSARY: Term[] = [
  // Test statistics
  {
    id: 't',
    term: 't',
    definition: 'The test statistic of a t test: the difference being tested divided by its standard error. The further it is from 0, in either direction, the less likely the difference is to be chance.',
    match: /^t$/,
    rank: 10,
    read: (v) => `The difference is ${two(Math.abs(v))} times its standard error${v < 0 ? ' (negative: the first group or measure is lower)' : ''}.`,
  },
  {
    id: 'F',
    term: 'F',
    also: ['F ratio'],
    definition: 'The test statistic of an ANOVA or a regression model: the variation the groups or predictors explain, per degree of freedom, divided by the variation left over. Near 1 means they explain no more than chance would.',
    match: /^F$/,
    rank: 10,
    read: (v) => `The groups or predictors explain ${two(v)} times as much variation as chance alone would lead you to expect.`,
  },
  {
    id: 'chi2',
    term: 'Chi-square (χ²)',
    also: ['χ²', 'Pearson chi-square'],
    definition: 'A test statistic that compares observed counts with the counts expected if there were no association (or if the expected proportions were right). Larger values mean a bigger gap.',
    match: /^(Pearson )?chi-square$|^χ²$/i,
    where: 'both',
    rank: 10,
    read: (v) => `χ² = ${two(v)}: how far the observed counts are from the counts expected with no association.`,
  },
  {
    id: 'z',
    term: 'z',
    definition: 'A test statistic on the standard normal scale. Values beyond about ±1.96 are significant at the .05 level (two-sided).',
    match: /^z$/,
    rank: 11,
    read: (v) => `z = ${two(v)} on the standard normal scale; beyond ±1.96 is significant at .05 (two-sided).`,
  },
  {
    id: 'U',
    term: 'Mann-Whitney U',
    also: ['U'],
    definition: 'The rank-based statistic of the Mann-Whitney test: of all pairs with one case from each group, the number in which the first group’s value is higher (ties count half).',
    match: /^U$/,
    rank: 10,
    read: (v) => `U = ${two(v)}: the number of cross-group pairs in which the first group’s value is higher (ties count half).`,
  },
  {
    id: 'H',
    term: 'Kruskal-Wallis H',
    also: ['H'],
    definition: 'The rank-based statistic of the Kruskal-Wallis test: how far apart the groups’ mean ranks are. It is compared with a chi-square distribution.',
    match: /^H$/,
    rank: 10,
    read: (v) => `H = ${two(v)}: how far apart the groups’ mean ranks are.`,
  },
  {
    id: 'wald',
    term: 'Wald statistic',
    definition: 'A test of one coefficient: the squared ratio of the estimate to its standard error, compared with a chi-square distribution.',
    match: /^Wald$/,
    rank: 12,
  },
  {
    id: 'df',
    term: 'Degrees of freedom (df)',
    also: ['df'],
    definition: 'How many independent pieces of information a statistic rests on, usually the number of cases or groups minus the number of things estimated. It fixes which t, F or chi-square distribution the p value comes from. Welch’s t test gives fractional df.',
    match: /^df[12]?$/,
    rank: 13,
  },
  {
    id: 'p',
    term: 'p value',
    also: ['p', 'significance'],
    definition: 'If the null hypothesis were true (no difference, no relationship), the probability of a result at least as far from it as this one. A small p (below .05 by convention) counts as evidence against the null hypothesis. It is not the probability that the null hypothesis is true, and it says nothing about how large or important an effect is.',
    match: /^p( \(.*\))?$/,
    rank: 20,
    read: (p) =>
      `p ${pText(p)}. If there were no effect in the population, ${p < 0.001 ? 'fewer than 1 in 1,000' : p < 0.01 ? `about ${fmt(p * 1000, 0)} in 1,000` : `about ${fmt(p * 100, 0)} in 100`} samples would give a result at least this far from it. ${p < 0.05 ? 'That is below .05, the usual cutoff, so the result is called statistically significant.' : 'That is not below .05, the usual cutoff, so the result is not statistically significant. That doesn’t show there is no effect, only that these data can’t tell it from chance.'}`,
  },
  // Effect sizes
  {
    id: 'diff',
    term: 'Mean difference',
    definition: 'One mean minus the other, in the outcome’s own units. Its confidence interval shows how precisely the data pin down the difference in the population.',
    match: /^Mean difference$/,
    rank: 25,
    read: (v) => `The means differ by ${two(Math.abs(v))} in the outcome’s units${v < 0 ? ' (negative: the first is lower)' : ''}.`,
  },
  {
    id: 'd',
    term: 'Cohen’s d',
    also: ['d', 'd_z'],
    definition: 'A difference between means in standard-deviation units, so it can be compared across studies and scales. Its sign shows the direction. Compare it with effects in similar research: fixed labels such as “small” or “large” are only rough conventions.',
    match: /^Cohen's d/,
    where: 'row',
    rank: 30,
    read: (v) => `The means differ by ${two(Math.abs(v))} standard deviations.`,
  },
  {
    id: 'g',
    term: 'Hedges’ g',
    definition: 'Cohen’s d with a small-sample correction, which makes it slightly smaller and less biased. It is read the same way, in standard deviations.',
    match: /^Hedges' g/,
    where: 'row',
    rank: 30,
    read: (v) => `The means differ by ${two(Math.abs(v))} standard deviations (corrected for sample size).`,
  },
  {
    id: 'eta2',
    term: 'Eta squared (η²)',
    also: ['η²', 'partial η²'],
    definition: 'The share of the outcome’s variation that the groups (or one effect) account for, from 0 to 1. Partial η² leaves the other effects out of the total, so it is larger in designs with several factors.',
    match: /η²|Eta squared/,
    where: 'row',
    rank: 31,
    read: (v) => `The groups account for ${pct(v)} of the variation in the outcome in this sample.`,
  },
  {
    id: 'omega2',
    term: 'Omega squared (ω²)',
    also: ['ω²'],
    definition: 'Like η², the share of variation the groups account for, but adjusted to estimate the population value. It is smaller than η² and can fall just below 0 when the groups explain almost nothing.',
    match: /ω²|Omega squared/,
    where: 'row',
    rank: 31,
    read: (v) => `Estimated for the population, the groups account for ${v <= 0 ? 'none' : pct(v)} of the variation.`,
  },
  {
    id: 'epsilon2',
    term: 'Epsilon squared (ε²)',
    also: ['ε²'],
    definition: 'An effect size for the Kruskal-Wallis test: the share of variation in the ranks that the groups account for, from 0 to 1.',
    match: /ε²|Epsilon squared/,
    rank: 31,
    read: (v) => `The groups account for ${pct(v)} of the variation in the ranks.`,
  },
  {
    id: 'r',
    term: 'Correlation (r)',
    also: ['Pearson r', 'Spearman ρ'],
    definition: 'How closely two variables move together in a straight line, from −1 to 1. Positive: higher values of one go with higher values of the other. 0: no linear relationship. Squared, it is the share of variance they have in common. A correlation alone doesn’t show that one causes the other.',
    match: /^(Pearson )?r$/,
    rank: 30,
    read: (v) => `r = ${noLead(v)}: ${Math.abs(v) < 0.005 ? 'no linear relationship' : v > 0 ? 'a positive relationship, so higher values of one go with higher values of the other' : 'a negative relationship, so higher values of one go with lower values of the other'}. Squared (r² = ${noLead(v * v)}), the two share ${pct(v * v)} of their variance.`,
  },
  {
    id: 'rank_biserial',
    term: 'Rank-biserial correlation',
    definition: 'An effect size for rank tests, from −1 to 1: the share of pairs in which the first group is higher minus the share in which it is lower.',
    match: /^Rank-biserial/,
    where: 'row',
    rank: 31,
    read: (v) => `${pct(Math.abs(v))} more of the cross-group pairs favor the ${v < 0 ? 'second' : 'first'} group than the other.`,
  },
  {
    id: 'cramer',
    term: 'Cramér’s V',
    also: ['phi (φ)'],
    definition: 'The strength of association in a crosstab, from 0 (none) to 1 (each row predicts the column perfectly). For a 2 × 2 table it equals phi (φ).',
    match: /^Cramér's V/,
    where: 'row',
    rank: 31,
    read: (v) => `V = ${noLead(v)} on a scale from 0 (no association) to 1 (complete association).`,
  },
  {
    id: 'r2',
    term: 'R squared (R²)',
    also: ['adjusted R²'],
    definition: 'The share of the outcome’s variance that a regression model explains, from 0 to 1. Adjusted R² corrects it for the number of predictors, which helps when comparing models.',
    match: /^R²$/,
    rank: 30,
    read: (v) => `The model explains ${pct(v)} of the variance in the outcome.`,
  },
  {
    id: 'pseudo_r2',
    term: 'Pseudo R²',
    also: ['Nagelkerke R²', 'Cox & Snell R²', 'McFadden R²'],
    definition: 'Measures for logistic and other models that play the part of R² but are not shares of variance. Use them to compare models of the same outcome, not as “percent explained”.',
    match: /^Nagelkerke R²$/,
    rank: 32,
  },
  {
    id: 'B',
    term: 'Regression coefficient (B)',
    also: ['b', 'unstandardized coefficient'],
    definition: 'How much the predicted outcome changes for a one-unit increase in the predictor, holding the other predictors constant. For a categorical predictor, the difference from its reference category. In logistic regression it is on the log-odds scale; the odds ratio is easier to read.',
    match: /^B$/,
    rank: 33,
    read: (v) => `Each one-unit increase in the predictor goes with a change of ${two(v)} in the predicted outcome, holding any other predictors constant.`,
  },
  {
    id: 'beta',
    term: 'Standardized coefficient (β)',
    also: ['beta'],
    definition: 'A regression coefficient in standard-deviation units of both predictor and outcome, so predictors measured on different scales can be compared.',
    match: /^β$/,
    rank: 34,
    read: (v) => `A one-standard-deviation increase in the predictor goes with a change of ${two(v)} standard deviations in the outcome.`,
  },
  {
    id: 'or',
    term: 'Odds ratio',
    also: ['OR', 'Exp(B)'],
    definition: 'How the odds of the outcome change for a one-unit increase in the predictor. Above 1: higher odds. Below 1: lower odds. Exactly 1: no change. Odds are not probabilities: odds of 1 mean a 50% chance.',
    match: /^Odds ratio$/,
    rank: 33,
    read: (v) => `Each one-unit increase in the predictor multiplies the odds of the outcome by ${two(v)}${v > 1 ? ` (${fmt(100 * (v - 1), 0)}% higher odds)` : v < 1 ? ` (${fmt(100 * (1 - v), 0)}% lower odds)` : ''}.`,
  },
  {
    id: 'alpha',
    term: 'Cronbach’s alpha (α)',
    also: ['α', 'coefficient alpha'],
    definition: 'How consistently a scale’s items measure the same thing, from 0 to 1 (it can be negative when items work against each other, often because one wasn’t reverse-scored). It rises with the number of items, so a long scale can have a high alpha without measuring one thing.',
    match: /^Cronbach's α$/,
    rank: 30,
    read: (v) => `α = ${noLead(v)}${v < 0 ? ': negative, which usually means an item needs reverse-scoring' : ''}.`,
  },
  {
    id: 'omega',
    term: 'McDonald’s omega (ω)',
    definition: 'A reliability coefficient from a one-factor model of the items. Unlike alpha, it doesn’t assume every item measures the factor equally well.',
    match: /^McDonald's ω$/,
    rank: 31,
  },
  {
    id: 'auc',
    term: 'Area under the ROC curve (AUC)',
    also: ['AUC', 'C statistic'],
    definition: 'The chance that a randomly chosen case with the outcome gets a higher score or predicted probability than one without it. 0.5 is no better than a coin; 1 separates them perfectly.',
    match: /^AUC$/,
    rank: 32,
    read: (v) => `A case with the outcome scores higher than one without it ${pct(v)} of the time (50% would be chance).`,
  },
  // Intervals and spread
  {
    id: 'ci',
    term: 'Confidence interval (CI)',
    also: ['95% CI'],
    definition: 'A range of values for the population quantity that are consistent with the data. Over many studies, 95% of 95% intervals made this way contain the true value. An interval that leaves out 0 (for a difference) or 1 (for a ratio) matches a significant two-sided test at the same level. A wide interval means the estimate is imprecise.',
    rank: 40,
  },
  {
    id: 'se',
    term: 'Standard error (SE)',
    definition: 'How much an estimate, such as a mean or a coefficient, would vary from sample to sample. Smaller standard errors mean more precise estimates; they shrink as the sample grows.',
  },
  {
    id: 'sd',
    term: 'Standard deviation (SD)',
    definition: 'How spread out the values are around their mean, in the variable’s own units. In roughly normal data about two-thirds of the values lie within one SD of the mean.',
  },
  { id: 'mean', term: 'Mean (M)', definition: 'The average: the sum of the values divided by how many there are. Outliers pull it toward them.' },
  { id: 'median', term: 'Median (Mdn)', definition: 'The middle value when the values are put in order: half are above it and half below. Outliers barely move it.' },
  { id: 'mean_rank', term: 'Mean rank', definition: 'In a rank test, all values are ranked together (1 for the lowest). A group’s mean rank is the average of its cases’ ranks; a higher mean rank means its values tend to be higher.' },
  { id: 'n', term: 'N', definition: 'The number of cases. Results report how many were analyzed and why any were left out (missing values, case selection).' },
  { id: 'vif', term: 'Variance inflation factor (VIF)', definition: 'How much a coefficient’s variance is inflated because its predictor overlaps with the others (multicollinearity). 1 means no overlap; values above about 5 to 10 mean the coefficients are hard to separate.' },
  { id: 'aic', term: 'AIC', also: ['Akaike information criterion'], definition: 'A measure for comparing models of the same data: lower is better, balancing fit against the number of parameters. Only differences between models mean anything.' },
  { id: 'ss', term: 'Sum of squares (SS)', definition: 'Squared deviations added up: the raw amount of variation, split in an ANOVA into the part between groups and the part within them. A mean square is a sum of squares divided by its degrees of freedom.' },
  // Ideas
  { id: 'null', term: 'Null hypothesis', also: ['H₀'], definition: 'The claim a test checks the data against, usually that there is no difference or no relationship in the population. A test can reject it or fail to reject it; it never proves it.' },
  { id: 'alternative', term: 'One-sided and two-sided tests', definition: 'A two-sided test (the default) looks for a difference in either direction. A one-sided test looks in one direction only, chosen before seeing the data; it can’t detect an effect the other way.' },
  { id: 'significance', term: 'Statistical significance', definition: 'A result with p below the chosen level (usually .05). It means the result would be unlikely if there were no effect. It doesn’t mean the effect is large, important or certain to replicate.' },
  { id: 'effect_size', term: 'Effect size', definition: 'How large a difference or relationship is, on a scale that doesn’t depend on the sample size, such as d, r or η². Report it with its confidence interval alongside p.' },
  { id: 'power', term: 'Power', definition: 'The probability that a study detects an effect of a given size if it exists. It grows with the sample size and the size of the effect; 80% is a common target.' },
  { id: 'errors', term: 'Type I and Type II errors', definition: 'A Type I error rejects a null hypothesis that is true (a false positive; its rate is the significance level). A Type II error misses a real effect (a false negative; its rate is 1 − power).' },
  { id: 'multiple', term: 'Multiple comparisons', definition: 'Running many tests raises the chance that some are significant by luck. Post hoc tests (Tukey, Games-Howell, Dunn) and adjustments (Holm, Bonferroni) keep that chance at the chosen level.' },
  { id: 'welch', term: 'Welch’s test', definition: 'A version of the t test or one-way ANOVA that doesn’t assume the groups have equal variances. PrismStatistics uses it by default because it loses little when variances are equal and protects against error when they aren’t.' },
  { id: 'normality', term: 'Normality', definition: 'Whether values (or residuals) follow a bell-shaped normal distribution. Tests of means assume it, but with about 30 cases per group they work well even with some skew. Look at histograms and Q-Q plots rather than relying on a normality test alone.' },
  { id: 'homogeneity', term: 'Equal variances', also: ['homogeneity of variance'], definition: 'Whether groups have similar spread. Levene’s test checks it; Welch’s versions of the tests don’t need it.' },
  { id: 'outlier', term: 'Outlier', definition: 'A value far from the rest. Check whether it is an error before anything else. Means, SDs and correlations are sensitive to outliers; medians, ranks and trimmed means are not.' },
  { id: 'censored', term: 'Censored case', definition: 'In survival analysis, a case that had not had the event when its follow-up ended. Its time is known only to be longer than what was observed. Survival methods use that information instead of treating the last time as the event time.' },
  { id: 'levels', term: 'Levels of measurement', also: ['nominal', 'ordinal', 'interval', 'ratio', 'scale'], definition: 'Nominal: categories with no order. Ordinal: ordered categories whose steps need not be equal. Interval: equal steps but no true zero. Ratio: equal steps and a true zero. In Variable View, interval and ratio are both “scale”.' },
  { id: 'missing', term: 'Missing values', also: ['user-missing'], definition: 'Empty cells are system-missing. Codes such as −99 that mean “no answer” are user-missing once they are set in Variable View. Analyses leave both out and report how many cases they lost.' },
  { id: 'case', term: 'Case and variable', definition: 'A case is one row of the data: a person, a school, a day. A variable is one column: something measured or recorded for every case.' },
];

export const TERMS: Record<string, Term> = Object.fromEntries(GLOSSARY.map((t) => [t.id, t]));

/** Help › Glossary, as a help page. */
export function glossaryPage(): string {
  const sorted = [...GLOSSARY].sort((a, b) => a.term.localeCompare(b.term, 'en', { sensitivity: 'base' }));
  return ['# Glossary', '', 'The terms used in the output and the help. **Explain this result** under each result uses the same definitions to read its numbers.', '', ...sorted.map((t) => `- **${t.term}**${t.also?.length ? ` (also ${t.also.join(', ')})` : ''}: ${t.definition}`)].join('\n');
}
