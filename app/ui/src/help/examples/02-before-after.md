# Worked example: before and after

Did students' scores rise from the pretest to the posttest? The same students took both tests, so this is a paired comparison.

**[Open the Study methods data](example:study_methods)**. Each student has a **pretest** and a **posttest** score, in separate columns.

## 1. Choose the analysis

In **Help me choose**: **Whether groups or conditions differ** › **Scores (interval or ratio)** › **No** › **The same people measured twice** › **Not sure**. It suggests the **Paired-Samples T Test**. Pick **pretest** as the first measurement and **posttest** as the second, and choose **Open**.

The independent-samples test would be wrong here. It would treat the 240 scores as if they came from 240 different students and ignore that each student's two scores go together.

## 2. Run it

**pretest − posttest** is in **Paired Variables**. The difference is first minus second, so a rise shows as a negative difference. Choose **Run**.

## 3. Read the output

- **Means**: the pretest averaged 55.77 (SD = 12.52) and the posttest 67.55 (SD = 13.73).
- **The test**: *t*(119) = −21.63, *p* < .001, with 120 pairs and so 119 degrees of freedom.
- **The difference**: −11.78 points, 95% CI [−12.86, −10.70]. The interval is narrow because the two scores are strongly correlated, so each student's change is measured precisely.
- **Effect size**: Cohen's *d*<sub>z</sub> = −1.97. This is the mean change divided by the standard deviation of the changes, not of the scores. That's why it is much larger than a *d* between two groups would be. Name it *d*<sub>z</sub> when you report it.

## 4. What the result doesn't show

Scores rose, but there was no group that went without the unit. Practice with the test, growing up, or anything else that happened between the tests could explain part of the rise. To show that the teaching caused the gain, you need a comparison group.

## Try next

- Run the **Wilcoxon Signed-Rank** test on the same pair, the rank-based alternative.
- Compute the gain with **Transform › Compute Variable…** (posttest − pretest). Then compare the gain between the two teaching methods with an independent-samples t test.
- Open the **Sleep and reaction time** example, which has four measurements per person, and try **Repeated-Measures ANOVA**.
