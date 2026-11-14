// The model text of Analyze › Structural Equation Modeling › Factor Analysis of a Scale (components/CfaDialog.tsx).

export interface Factor {
  name: string;
  items: string[];
}

export const validName = (s: string) => /^[A-Za-z_.][A-Za-z0-9_.]*$/.test(s);

/** The lavaan model of the factors: one line per factor, then zero covariances when factors are uncorrelated. */
export function cfaModel(factors: Factor[], correlated: boolean): string {
  const lines = factors.filter((f) => f.items.length).map((f) => `${f.name} =~ ${f.items.join(' + ')}`);
  if (!correlated) {
    const used = factors.filter((f) => f.items.length);
    for (let i = 0; i < used.length; i++) for (let j = i + 1; j < used.length; j++) lines.push(`${used[i].name} ~~ 0*${used[j].name}`);
  }
  return lines.join('\n');
}
