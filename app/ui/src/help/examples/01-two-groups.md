# Worked example: comparing two groups

Did students taught by active learning score higher on the final exam than students taught by lecture? This example walks through the question from choosing the test to writing it up.

**[Open the Study methods data](example:study_methods)**, or choose **File › Open Example Data…**. The data are simulated for teaching: 120 students, 60 taught each way.

## 1. Choose the analysis

If you aren't sure which test fits, open **Help me choose** (Ctrl+Shift+H) and answer:

1. **Whether groups or conditions differ**
2. **Scores (interval or ratio)**: the exam is scored 0 to 100.
3. **No**: it isn't a time until something happens.
4. **Two groups of different people**: each student was taught one way.
5. **Not sure** about the shape. That's fine: the t test checks its own assumptions.

It suggests the **Independent-Samples T Test**. Pick **exam** as the outcome and **method** as the grouping variable, then choose **Open**. You can also go straight to **Analyze › Compare Means › Independent-Samples T Test…**.

## 2. Run it

In the dialog, **exam** is in **Test Variables** and **method** in **Grouping Variable**. Choose the groups **1 (Lecture)** and **2 (Active learning)**. Leave **Welch (default)**, which doesn't assume the two groups have equal variances, and choose **Run**.

## 3. Read the output

- **Group statistics**: lecture students averaged 65.67 (SD = 13.33) and active-learning students 71.03 (SD = 12.31).
- **The test**: *t*(117.27) = −2.29, *p* = .024. The *t* is negative because the first group (lecture) is lower. The degrees of freedom aren't a whole number because Welch's test adjusts them.
- **The difference**: −5.37 points, 95% CI [−10.01, −0.73]. The interval leaves out 0, which matches *p* < .05. It is also wide: the population difference could be anywhere from about 1 point to 10.
- **Effect size**: Hedges' *g* = −0.42, 95% CI [−0.77, −0.06]. The groups differ by about four-tenths of a standard deviation.
- **Checks**: the result reports the group sizes and compares the two standard deviations. It doesn't judge shape and outliers for you, so make a box plot of exam by method (**Graphs › Box Plot…**) and look for long tails or far-off points. Independence of observations can't be checked from the data at all. It comes from the design: one score per student, and no student in both groups.

Choose **Explain this result** under the result for a plain-language reading of each of these numbers.

## 4. Write it up

The result ends with an APA paragraph you can copy and edit:

> An independent-samples *t* test (Welch's unequal-variances t test) compared Final exam score (0-100) between the Lecture and Active learning groups. Final exam score (0-100) was lower in the Lecture group (*M* = 65.67, *SD* = 13.33) than in the Active learning group (*M* = 71.03, *SD* = 12.31), *t*(117.27) = −2.29, *p* = .024, mean difference = −5.37, 95% CI [−10.01, −0.73], Hedges' *g* = −0.42, 95% CI [−0.77, −0.06].

Before you use it, say what the comparison means for your question. If students chose their own section, the groups may have differed before teaching began, so the design can't show that the method caused the difference.

## Try next

- Run **Mann-Whitney U** on the same variables (Also consider, in Help me choose). Does it agree?
- Compare **satisfaction**, an ordinal rating, between the methods. Help me choose sends ordinal outcomes to the rank-based tests.
- Compare **exam** across the three **sections** with a **One-Way ANOVA**.
