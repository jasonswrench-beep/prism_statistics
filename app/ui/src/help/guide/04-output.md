# Results and APA writing

Each result on the **Output** screen has the same parts:

- **Tables and figures**, formatted in APA style.
- **Checks**: the assumptions tested and anything worth knowing, such as a small expected count or a model that did not converge. A result that could not be computed says why.
- **Paragraph**: an APA-style description of the result, written from a template for that analysis. Edit it freely in your own document.
- **Method note**: exactly what was computed (the formula, the correction, and the R function it matches), for a methods section or a reviewer.

## Working with a result

- **Reopen** a result's dialog with the same settings to change something and run it again.
- **Run again** updates a result marked out of date.
- **Results › Clear output** removes every result. The data stay as they are.

## AI writing (optional)

**Tools › AI writing…** can draft a fuller paragraph from a result's numbers, using an AI model you connect. It is off until you set it up.

- The draft is labeled as written by AI.
- The numbers in the draft are checked against the result, and any number that doesn't match is flagged.
- The template paragraph is always shown as well.

## Long analyses

Some analyses take a while:

- bootstrap and permutation resamples
- random forests and gradient boosting with many trees
- latent profile models with many starts
- multiple imputation
- Bayesian estimation of a structural equation model

After a moment, a panel shows how far the analysis has come, with a **Cancel** button. A cancelled analysis adds nothing to the output and changes nothing in the project.
