"""Makes the example datasets (File > Open Example Data) in crates/prism-api/data/examples/.

    python tools/make_examples.py

Each dataset is a CSV of simulated cases and a JSON codebook (title, description, and each variable's label, measure,
value labels and missing-value codes). The numbers are invented for teaching; the seed is fixed, so running this again
gives the same files, and the worked examples in the help quote numbers from them (checked by
crates/prism-api/src/examples.rs).
"""

import json
import os

import numpy as np

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "crates", "prism-api", "data", "examples")


def clip_round(x, lo, hi, digits=0):
    return np.round(np.clip(x, lo, hi), digits)


def write(name, meta, columns):
    os.makedirs(OUT, exist_ok=True)
    names = [v["name"] for v in meta["variables"]]
    assert names == list(columns), (names, list(columns))
    n = len(next(iter(columns.values())))
    with open(os.path.join(OUT, f"{name}.csv"), "w", encoding="utf-8", newline="\n") as f:
        f.write(",".join(names) + "\n")
        for i in range(n):
            cells = []
            for k in names:
                v = columns[k][i]
                if v is None or (isinstance(v, float) and np.isnan(v)):
                    cells.append("")
                elif float(v).is_integer():
                    cells.append(str(int(v)))
                else:
                    cells.append(f"{float(v):g}")
            f.write(",".join(cells) + "\n")
    meta = {"id": name, "rows": n, **meta}
    with open(os.path.join(OUT, f"{name}.json"), "w", encoding="utf-8", newline="\n") as f:
        json.dump(meta, f, indent=2, ensure_ascii=False)
        f.write("\n")


def study_methods(rng):
    n = 120
    method = np.repeat([1, 2], n // 2)
    rng.shuffle(method)
    section = rng.integers(1, 4, n)
    hours = clip_round(rng.gamma(4.0, 1.5, n), 0, 25, 1)
    ability = rng.normal(0, 1, n)
    pretest = clip_round(55 + 10 * ability + rng.normal(0, 6, n), 20, 100)
    gain = 6 + 4 * (method == 2) + 0.6 * hours + rng.normal(0, 5, n)
    posttest = clip_round(pretest + gain, 20, 100)
    exam = clip_round(58 + 8 * ability + 8 * (method == 2) + 1.1 * hours + rng.normal(0, 8, n), 0, 100)
    passed = (exam >= 70).astype(int)
    anxiety = rng.normal(0, 1, n) - 0.25 * ability
    items = {}
    for k in range(1, 6):
        raw = 3 + 0.9 * anxiety + rng.normal(0, 0.8, n)
        if k == 3:
            raw = 6 - raw  # worded the other way round
        items[f"anx{k}"] = clip_round(raw, 1, 5)
    sat_raw = 3.1 + 0.45 * (method == 2) + 0.02 * (exam - 70) + rng.normal(0, 0.9, n)
    satisfaction = clip_round(sat_raw, 1, 5).astype(float)
    satisfaction[rng.choice(n, 6, replace=False)] = -99  # no answer
    absences = rng.poisson(np.exp(1.1 - 0.35 * ability - 0.04 * hours), n)
    likert = [{"value": 1, "label": "Strongly disagree"}, {"value": 2, "label": "Disagree"}, {"value": 3, "label": "Neutral"}, {"value": 4, "label": "Agree"}, {"value": 5, "label": "Strongly agree"}]
    meta = {
        "title": "Study methods",
        "description": "120 students in three sections of an introductory course, taught by lecture or by active learning. Test scores before and after the unit, the final exam, weekly study hours, absences, satisfaction with the course, and a five-item test-anxiety scale.",
        "try": ["Compare exam scores between the two methods (independent-samples t test)", "Compare pretest and posttest scores (paired-samples t test)", "Relate study hours to exam scores (correlation, regression)", "Predict passing from hours and method (logistic regression)", "Check the anxiety scale (reliability; anx3 is reverse-worded)"],
        "variables": [
            {"name": "id", "label": "Student ID", "measure": "nominal"},
            {"name": "section", "label": "Course section", "measure": "nominal", "value_labels": [{"value": 1, "label": "Section A"}, {"value": 2, "label": "Section B"}, {"value": 3, "label": "Section C"}]},
            {"name": "method", "label": "Teaching method", "measure": "nominal", "value_labels": [{"value": 1, "label": "Lecture"}, {"value": 2, "label": "Active learning"}]},
            {"name": "hours", "label": "Study hours per week", "measure": "scale"},
            {"name": "pretest", "label": "Pretest score (0-100)", "measure": "scale"},
            {"name": "posttest", "label": "Posttest score (0-100)", "measure": "scale"},
            {"name": "exam", "label": "Final exam score (0-100)", "measure": "scale"},
            {"name": "passed", "label": "Passed the exam (70 or more)", "measure": "nominal", "value_labels": [{"value": 0, "label": "No"}, {"value": 1, "label": "Yes"}]},
            {"name": "satisfaction", "label": "I am satisfied with this course", "measure": "ordinal", "value_labels": likert + [{"value": -99, "label": "No answer"}], "missing_values": [-99]},
            *[{"name": f"anx{k}", "label": label, "measure": "ordinal", "value_labels": likert} for k, label in enumerate(["I worry before a test", "My mind goes blank in exams", "I feel calm during tests", "I lose sleep before an exam", "My heart races during a test"], 1)],
            {"name": "absences", "label": "Classes missed", "measure": "scale"},
        ],
    }
    write("study_methods", meta, {"id": np.arange(1, n + 1), "section": section, "method": method, "hours": hours, "pretest": pretest, "posttest": posttest, "exam": exam, "passed": passed, "satisfaction": satisfaction, **items, "absences": absences})


def sleep_reaction(rng):
    n = 40
    caffeine = np.repeat([0, 1], n // 2)
    base = rng.normal(250, 25, n)
    cols = {}
    for day in range(1, 5):
        slow = 18 * (day - 1) * (1 - 0.35 * caffeine)
        cols[f"rt_day{day}"] = clip_round(base + slow + rng.normal(0, 15, n), 150, 500)
    sleep = clip_round(rng.normal(6.8, 0.9, n), 4, 9.5, 1)
    meta = {
        "title": "Sleep and reaction time",
        "description": "40 volunteers whose sleep was cut to five hours a night for four days. Reaction time was measured each day; half took caffeine each morning. Their usual hours of sleep were recorded beforehand.",
        "try": ["Compare reaction times across the four days (repeated-measures ANOVA, Friedman)", "Compare day 1 with day 4 (paired-samples t test)", "Add caffeine as a between-subjects factor (mixed ANOVA)"],
        "variables": [
            {"name": "id", "label": "Participant", "measure": "nominal"},
            {"name": "caffeine", "label": "Caffeine each morning", "measure": "nominal", "value_labels": [{"value": 0, "label": "No"}, {"value": 1, "label": "Yes"}]},
            *[{"name": f"rt_day{d}", "label": f"Reaction time, day {d} (ms)", "measure": "scale"} for d in range(1, 5)],
            {"name": "usual_sleep", "label": "Usual hours of sleep", "measure": "scale"},
        ],
    }
    write("sleep_reaction", meta, {"id": np.arange(1, n + 1), "caffeine": caffeine, **cols, "usual_sleep": sleep})


def clinic_followup(rng):
    n = 80
    treatment = np.repeat([1, 2], n // 2)
    age = clip_round(rng.normal(42, 11, n), 19, 75)
    rate = 0.025 * np.exp(-0.55 * (treatment == 2) + 0.015 * (age - 42))
    t = rng.exponential(1 / rate)
    end = rng.uniform(26, 52, n)  # weeks each person was followed
    relapsed = (t <= end).astype(int)
    weeks = clip_round(np.minimum(t, end), 1, 52)
    meta = {
        "title": "Clinic follow-up",
        "description": "80 patients given the standard or a new treatment and followed for up to a year. Weeks until relapse, or until follow-up ended for those who had not relapsed (censored), and age at the start.",
        "try": ["Compare time to relapse between treatments (Kaplan-Meier with the log-rank test)", "Add age as a predictor (Cox regression)"],
        "variables": [
            {"name": "id", "label": "Patient", "measure": "nominal"},
            {"name": "treatment", "label": "Treatment", "measure": "nominal", "value_labels": [{"value": 1, "label": "Standard"}, {"value": 2, "label": "New"}]},
            {"name": "age", "label": "Age at start (years)", "measure": "scale"},
            {"name": "weeks", "label": "Weeks followed", "measure": "scale"},
            {"name": "relapsed", "label": "Relapsed", "measure": "nominal", "value_labels": [{"value": 0, "label": "No (censored)"}, {"value": 1, "label": "Yes"}]},
        ],
    }
    write("clinic_followup", meta, {"id": np.arange(1, n + 1), "treatment": treatment, "age": age, "weeks": weeks, "relapsed": relapsed})


if __name__ == "__main__":
    rng = np.random.default_rng(2027)
    study_methods(rng)
    sleep_reaction(rng)
    clinic_followup(rng)
    print("Wrote", ", ".join(sorted(os.listdir(OUT))))
