# Reverse Score

Reverses items that are worded the other way round, so that every item of a scale points the same way.

## When to use it
- On a scale where most items agree with the construct but some disagree. For example, on a test-anxiety scale, "I worry before a test" and "I feel calm during tests" point opposite ways.
- Reverse those items before **Reliability Analysis** or **Factor Analysis**. A reverse-worded item that hasn't been reversed lowers Cronbach's alpha and can make it negative. (**Scale Score** can reverse items itself as it scores.)

## How to run it
Choose **Transform › Reverse Score…**. Tick the items, give the lowest and highest possible answers (such as 1 and 5), and choose **Reverse**.

## Options
- **Put the reversed answers**: **In new variables, `<name>_r` (default)**, which keeps the original items, or **In the same variables**.

## What you get
- Each answer becomes lowest + highest − answer: on a 1 to 5 scale, 1 becomes 5, 2 becomes 4 and 3 stays 3.
- Value labels move with their values, so "Strongly agree" stays on the answer that means it.
- Missing-value codes such as −99 are kept as they are and stay missing.
- A value outside the range you give stops the change. That way a mistyped range can't go unnoticed.

## Checked against
The exported R script (`lowest + highest − x`, leaving missing codes) gives the same values.
