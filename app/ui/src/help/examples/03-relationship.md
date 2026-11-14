# Worked example: a relationship and a prediction

Do students who study more score higher on the exam, and by how much? This example uses a correlation for the first question and a regression for the second.

**[Open the Study methods data](example:study_methods)**. **hours** is weekly study time and **exam** the final exam score.

## 1. Look first

Make a scatterplot with **Graphs › Scatterplot…** (hours on X, exam on Y). Check that the points rise roughly in a straight line, without a curve, a few far-off points, or separate clusters. A correlation describes only a straight-line relationship.

## 2. The correlation

In **Help me choose**: **Whether variables are related** › **Scores (interval or ratio)** › **Roughly symmetric, related in a straight line**. It suggests the **Pearson Correlation**. Pick **hours** and **exam**, choose **Open**, then **Run**.

- *r*(118) = .36, *p* < .001, 95% CI [.19, .50]. Students who studied more tended to score higher.
- *r*² is about .13, so study hours and exam scores share about 13% of their variance. Most of the variation in scores has to do with other things.

## 3. The prediction

A regression gives the same relationship in the exam's own units. Choose **Analyze › Regression › Linear…**, with **exam** as the **Dependent** and **hours** in **Numeric predictors**, and choose **Run**.

- **Model summary**: *R*² = .13, *F*(1, 118) = 17.34, *p* < .001. With one predictor, *R* is the correlation, and the *p* is the same.
- **Coefficients**: the intercept is 58.91 and the slope for hours is *B* = 1.51, 95% CI [0.79, 2.23]. The predicted exam score is 58.91 + 1.51 × hours: each extra weekly hour goes with about 1.5 more points.
- The intercept is the predicted score for a student who studies 0 hours. Only use it if 0 hours is within the range of the data.

Turn on **Residual plots** in the dialog to check that the residuals spread evenly around zero.

## 4. What the result doesn't show

These students chose how much to study, so the data can't show that more study causes higher scores. Stronger students might both study more and score higher. Add other predictors, such as **pretest**, to see whether the hours still matter once prior ability is held constant.

## Try next

- Add **pretest** as a second predictor. How does the slope for hours change?
- Predict **passed** (yes or no) from **hours** with **Binary Logistic Regression**. Help me choose sends a two-category outcome there.
