# Choosing an analysis

If you aren't sure which analysis fits your question, open **Help me choose**. It is in the **Analyze** and **Help** menus, on the toolbar and on the Home screen, and Ctrl+Shift+H opens and closes it. It opens beside your data, so you can look at the variables while you answer.

## How it works

1. **Answer the questions.** The first asks what you want to find out: whether groups differ, whether variables are related, how well variables predict an outcome, whether a scale or a set of ratings holds up, what your data look like, or how many participants you need. Each answer leads to the next question, such as what kind of outcome you have or whether the same people were measured more than once.
2. **Read the suggestion.** At the end you get one analysis, why it fits, and what to check in its output. **Also consider** lists the alternatives, such as the rank-based or robust version of a test; choose one to read about it instead.
3. **Pick the variables (optional).** Choose the outcome, the grouping variable and so on from the lists. The lists show the most likely variables first, using each variable's measure in Variable View.
4. **Open the analysis.** **Open** opens the analysis's dialog with your variables already in their boxes. Check them, set any options, and choose **Run**. If you left a variable unpicked, choose it in the dialog.

Your answers stay at the top of the panel. Choose **Change** beside one to answer it again, or **Start over** to begin again. **About this analysis** opens the analysis's help page.

## Some terms in the questions

- **Different people** means each case belongs to one group only, such as a treatment group and a control group in one column.
- **The same people** means each case was measured in every condition, with each condition in its own column, such as a pretest and a posttest.
- **The kind of variable** is its level of measurement. Questions about it offer three answers, and **Not sure which?** under them explains the levels:
  - **Nominal:** categories with no order, such as major or treatment group. A code is only a label, even when 1 = passed and 0 = failed.
  - **Ordinal:** categories in an order whose steps need not be equal, such as class rank or a single agree–disagree item. Ordinal outcomes lead to the rank-based tests.
  - **Interval or ratio (scores):** numbers with equal steps, such as test scores (interval) or reaction times and income (ratio, with a true zero). The two lead to the same analyses, so they are one answer. In Variable View both are "scale". A total or mean of several agree–disagree items is usually treated as a score.
- **Censored times.** A time until something happens is a ratio score. If some cases hadn't reached the event when the study ended, they are censored: we only know their time is longer than what was observed. Help me choose asks about this after you choose Scores, and sends censored times to the survival analyses (Kaplan-Meier and Cox regression). Counts of events are ratio scores too, but they get their own model, Poisson or negative binomial regression.
- The variable lists show each variable's measure from Variable View, so you can check the level of your own variables.
- **Roughly symmetric** means a histogram of the scores in each group has no long tail on one side and no extreme points. With about 30 cases per group, tests of means work well even when the scores are somewhat skewed. Not sure? Choose **Not sure**. The usual test checks its own assumptions in the output, and the suggestion names what to use if they fail. Analyze › Descriptive Statistics › Explore shows the shape of each group.

Help me choose suggests a starting point. Your design and your research question decide the analysis, so read the suggestion's help page if it isn't clearly right.
