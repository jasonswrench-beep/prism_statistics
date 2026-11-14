//! Graphs › Chart Builder (`graph.*`): histograms, box plots, bar charts, scatterplots and normal Q-Q plots, each drawn
//! as an APA-style figure with the numbers behind it in a table beside it, so the chart can be checked and read without
//! sight. The numbers come from `prismqda_stats::charts` (bins as R's `hist`, boxes as `boxplot.stats` with type 7
//! quartiles, points as `qqnorm` and `qqline`); cases are left out per variable (or per pair) and counted.

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::chart::{self, BoxDraw, Chart};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{ResultBundle, Table};

pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Histogram,
    Boxplot,
    Bar,
    Scatter,
    Qq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BarStat {
    #[default]
    Count,
    Percent,
    /// The mean of `mean_of` in each category, with a 95% confidence interval.
    Mean,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub kind: Kind,
    /// Histogram, Q-Q: one chart each. Box plot: one box each, or one chart each when grouped. Bar: the categories.
    #[serde(default)]
    pub variables: Vec<String>,
    /// Scatterplot axes.
    #[serde(default)]
    pub x: Option<String>,
    #[serde(default)]
    pub y: Option<String>,
    /// Box plot: one box per category of this variable.
    #[serde(default)]
    pub group: Option<String>,
    /// Histogram: draw the normal curve with the same mean and SD.
    #[serde(default)]
    pub normal: bool,
    /// Scatterplot: draw the least-squares line.
    #[serde(default)]
    pub line: bool,
    #[serde(default)]
    pub bar_stat: BarStat,
    /// Bar chart of means: the numeric variable.
    #[serde(default)]
    pub mean_of: Option<String>,
}

fn method_id(k: Kind) -> &'static str {
    match k {
        Kind::Histogram => "graph.histogram",
        Kind::Boxplot => "graph.boxplot",
        Kind::Bar => "graph.bar",
        Kind::Scatter => "graph.scatter",
        Kind::Qq => "graph.qq",
    }
}

fn label(ds: &Dataset, name: &str) -> Result<String> {
    Ok(ds.variable(name)?.display_name().to_string())
}

/// The valid values of a numeric variable, with the reasons the others were left out.
fn values(ds: &Dataset, name: &str) -> Result<(Vec<f64>, Tally, usize)> {
    let cells = numeric(ds, name)?;
    let mut tally = Tally::default();
    let v: Vec<f64> = cells.iter().filter(|c| !tally.check(*c, name)).map(|c| c.value().expect("checked")).collect();
    Ok((v, tally, cells.len()))
}

fn result(spec: &Spec, title: String, cases: crate::result::CaseAccounting, note: String) -> ResultBundle {
    common::bundle(method_id(spec.kind), METHOD_VERSION, title, serde_json::to_value(spec).expect("a spec always serializes"), cases, format!("{note} Drawn by PrismStatistics with prismqda-stats {}.", super::STATS_VERSION))
}

fn chart(id: &str, title: &str, alt: String, svg: String) -> Chart {
    Chart { id: id.into(), title: title.into(), alt, svg }
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<Vec<ResultBundle>> {
    match spec.kind {
        Kind::Histogram | Kind::Qq => {
            if spec.variables.is_empty() {
                return Err(Error::Invalid("Choose at least one variable.".into()));
            }
            spec.variables.iter().map(|v| if spec.kind == Kind::Histogram { histogram(ds, spec, v) } else { qq(ds, spec, v) }).collect()
        }
        Kind::Boxplot => boxplot(ds, spec),
        Kind::Bar => bar(ds, spec).map(|b| vec![b]),
        Kind::Scatter => scatter(ds, spec).map(|b| vec![b]),
    }
}

fn histogram(ds: &Dataset, spec: &Spec, name: &str) -> Result<ResultBundle> {
    let l = label(ds, name)?;
    let (v, tally, total) = values(ds, name)?;
    let mut b = result(spec, format!("Histogram: {l}"), tally.accounting(total, v.len()), format!("Bins of equal width at round numbers (Sturges' rule), each holding the values above its lower edge up to its upper edge, as R's hist does.{}", if spec.normal { " The curve is the normal distribution with the same mean and SD, scaled to the counts." } else { "" }));
    let bins = prismqda_stats::charts::histogram(&v);
    if bins.is_empty() {
        fail(&mut b, format!("{l} has no valid values to draw."));
        return Ok(b);
    }
    let d = prismqda_stats::describe::descriptives(&v, 0.95);
    let normal = if spec.normal { d.mean.zip(d.sd).filter(|(_, s)| *s > 0.0).map(|(m, s)| (m, s, v.len())) } else { None };
    let raw: Vec<(f64, f64, usize)> = bins.iter().map(|x| (x.lo, x.hi, x.count)).collect();
    let alt = format!("Histogram of {l} for {} cases, with bars from {} to {}{}. The tallest bar holds {} cases.", v.len(), apa::plain(raw[0].0), apa::plain(raw[raw.len() - 1].1), if normal.is_some() { " and a normal curve" } else { "" }, raw.iter().map(|r| r.2).max().unwrap_or(0));
    b.charts.push(chart("histogram", &format!("Distribution of {l}"), alt.clone(), chart::histogram(&raw, normal, &l, &alt)));
    b.tables.push(Table { id: "bins".into(), title: "Bins".into(), columns: vec!["Lower edge".into(), "Upper edge".into(), "Count".into()], rows: bins.iter().map(|x| vec![num(x.lo, 2), num(x.hi, 2), count(x.count)]).collect(), footnotes: vec![format!("N = {}, M = {}, SD = {}.", v.len(), apa::num(d.mean.unwrap_or(f64::NAN), 2), apa::num(d.sd.unwrap_or(f64::NAN), 2))] });
    Ok(b)
}

fn qq(ds: &Dataset, spec: &Spec, name: &str) -> Result<ResultBundle> {
    let l = label(ds, name)?;
    let (v, tally, total) = values(ds, name)?;
    let mut b = result(spec, format!("Normal Q-Q Plot: {l}"), tally.accounting(total, v.len()), "Each value against where it would fall if the values were normal (R's qqnorm positions), with the line through the quartiles (qqline). Points close to the line suggest a roughly normal shape.".into());
    let Some((pts, line)) = prismqda_stats::charts::normal_qq(&v) else {
        fail(&mut b, format!("A Q-Q plot needs at least three values; {l} has {}.", v.len()));
        return Ok(b);
    };
    let points: Vec<(f64, f64)> = pts.iter().map(|p| (p.theoretical, p.sample)).collect();
    let alt = format!("Normal Q-Q plot of {l} for {} cases: observed values against normal quantiles, with the reference line through the quartiles.", v.len());
    b.charts.push(chart("qq", &format!("Normal Q-Q plot of {l}"), alt.clone(), chart::scatter(&points, Some((line.intercept, line.slope)), "Normal quantile", &l, &alt)));
    b.tables.push(Table { id: "line".into(), title: "Reference line".into(), columns: vec!["N".into(), "Intercept".into(), "Slope".into()], rows: vec![vec![count(v.len()), num(line.intercept, 3), num(line.slope, 3)]], footnotes: vec!["The intercept and slope estimate the mean and SD when the values are normal.".into()] });
    Ok(b)
}

fn box_draw(label: String, v: &[f64]) -> Option<(BoxDraw, prismqda_stats::charts::BoxStats)> {
    let s = prismqda_stats::charts::box_stats(v)?;
    Some((BoxDraw { label, lower_whisker: s.lower_whisker, q1: s.q1, median: s.median, q3: s.q3, upper_whisker: s.upper_whisker, outliers: s.outliers.clone() }, s))
}

fn box_table(rows: &[(String, prismqda_stats::charts::BoxStats)], first: &str) -> Table {
    Table {
        id: "boxes".into(),
        title: "Box plot values".into(),
        columns: vec![first.into(), "N".into(), "Lower whisker".into(), "Q1".into(), "Median".into(), "Q3".into(), "Upper whisker".into(), "Outliers".into()],
        rows: rows.iter().map(|(l, s)| vec![text(l), count(s.n), num(s.lower_whisker, 2), num(s.q1, 2), num(s.median, 2), num(s.q3, 2), num(s.upper_whisker, 2), text(if s.outliers.is_empty() { "none".to_string() } else { s.outliers.iter().map(|o| apa::num(*o, 2)).collect::<Vec<_>>().join(", ") })]).collect(),
        footnotes: vec!["Quartiles by R's default (type 7). Whiskers reach the most extreme values within 1.5 interquartile ranges of the box; values beyond are drawn as circles.".into()],
    }
}

fn boxplot(ds: &Dataset, spec: &Spec) -> Result<Vec<ResultBundle>> {
    if spec.variables.is_empty() {
        return Err(Error::Invalid("Choose at least one variable.".into()));
    }
    let note = "Boxes from the first to the third quartile with the median as a line.";
    let Some(g) = &spec.group else {
        // One chart with a box per variable, each with its own valid values.
        let mut boxes = Vec::new();
        let mut rows = Vec::new();
        let mut tally = Tally::default();
        let mut total = 0;
        let mut analyzed = 0;
        for name in &spec.variables {
            let l = label(ds, name)?;
            let (v, t, n) = values(ds, name)?;
            total = total.max(n);
            analyzed = analyzed.max(v.len());
            for e in t.accounting(0, 0).excluded {
                for _ in 0..e.count {
                    tally.add(e.reason.clone());
                }
            }
            if let Some((d, s)) = box_draw(l.clone(), &v) {
                boxes.push(d);
                rows.push((l, s));
            }
        }
        let labels: Vec<String> = spec.variables.iter().map(|n| label(ds, n)).collect::<Result<_>>()?;
        let mut b = result(spec, format!("Box Plot: {}", labels.join(", ")), tally.accounting(total * spec.variables.len(), analyzed), format!("{note} Each variable uses its own valid values."));
        if boxes.is_empty() {
            fail(&mut b, "None of the variables has values to draw.");
            return Ok(vec![b]);
        }
        let alt = format!("Box plots of {}.", apa::series(&labels));
        b.charts.push(chart("boxplot", &format!("Box plots of {}", apa::series(&labels)), alt.clone(), chart::boxplot(&boxes, "", if labels.len() == 1 { &labels[0] } else { "Value" }, &alt)));
        b.tables.push(box_table(&rows, "Variable"));
        return Ok(vec![b]);
    };
    let gl = label(ds, g)?;
    let (gs, levels) = categories(ds, g)?;
    spec.variables
        .iter()
        .map(|name| {
            let l = label(ds, name)?;
            let ys = numeric(ds, name)?;
            let mut tally = Tally::default();
            let mut groups: Vec<Vec<f64>> = vec![Vec::new(); levels.len()];
            for (gc, y) in gs.iter().zip(&ys) {
                if tally.check(gc, "grouping variable") || tally.check(y, name) {
                    continue;
                }
                groups[gc.value().expect("checked")].push(y.value().expect("checked"));
            }
            let analyzed = groups.iter().map(Vec::len).sum();
            let mut b = result(spec, format!("Box Plot: {l} by {gl}"), tally.accounting(ys.len(), analyzed), format!("{note} One box for each category of {gl} with data."));
            let mut boxes = Vec::new();
            let mut rows = Vec::new();
            for (lv, v) in levels.iter().zip(&groups) {
                if let Some((d, s)) = box_draw(lv.label.clone(), v) {
                    boxes.push(d);
                    rows.push((lv.label.clone(), s));
                }
            }
            if boxes.is_empty() {
                fail(&mut b, format!("No category of {gl} has values of {l} to draw."));
                return Ok(b);
            }
            let alt = format!("Box plots of {l} for each category of {gl}: {}.", rows.iter().map(|(n, s)| format!("{n}, median {}", apa::num(s.median, 2))).collect::<Vec<_>>().join("; "));
            b.charts.push(chart("boxplot", &format!("{l} by {gl}"), alt.clone(), chart::boxplot(&boxes, &gl, &l, &alt)));
            b.tables.push(box_table(&rows, &gl));
            Ok(b)
        })
        .collect()
}

fn bar(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    let name = spec.variables.first().ok_or(Error::Invalid("Choose the variable whose categories make the bars.".into()))?;
    let l = label(ds, name)?;
    let (cs, levels) = categories(ds, name)?;
    if spec.bar_stat == BarStat::Mean {
        let m = spec.mean_of.as_ref().ok_or(Error::Invalid("Choose the numeric variable to average.".into()))?;
        let ml = label(ds, m)?;
        let ys = numeric(ds, m)?;
        let mut tally = Tally::default();
        let mut groups: Vec<Vec<f64>> = vec![Vec::new(); levels.len()];
        for (c, y) in cs.iter().zip(&ys) {
            if tally.check(c, name) || tally.check(y, m) {
                continue;
            }
            groups[c.value().expect("checked")].push(y.value().expect("checked"));
        }
        let analyzed = groups.iter().map(Vec::len).sum();
        let mut b = result(spec, format!("Bar Chart: mean {ml} by {l}"), tally.accounting(ys.len(), analyzed), "Each bar is the mean; the error bars are 95% confidence intervals of the mean (t-based).".into());
        let mut cats = Vec::new();
        let mut errs = Vec::new();
        let mut rows = Vec::new();
        for (lv, v) in levels.iter().zip(&groups) {
            if v.is_empty() {
                continue;
            }
            let d = prismqda_stats::describe::descriptives(v, 0.95);
            let mean = d.mean.unwrap_or(f64::NAN);
            let (lo, hi) = d.ci_mean.unwrap_or((f64::NAN, f64::NAN));
            cats.push((lv.label.clone(), mean));
            errs.push((lo, hi));
            rows.push(vec![text(&lv.label), count(v.len()), num(mean, 2), opt(d.sd, 2), num(lo, 2), num(hi, 2)]);
        }
        if cats.is_empty() {
            fail(&mut b, format!("No category of {l} has values of {ml}."));
            return Ok(b);
        }
        let alt = format!("Bar chart of the mean {ml} for each category of {l}, with 95% confidence intervals: {}.", cats.iter().map(|(n, v)| format!("{n} {}", apa::num(*v, 2))).collect::<Vec<_>>().join(", "));
        b.charts.push(chart("bar", &format!("Mean {ml} by {l}"), alt.clone(), chart::bars(&cats, Some(&errs), &l, &format!("Mean {ml}"), &alt)));
        b.tables.push(Table { id: "means".into(), title: "Means".into(), columns: vec![l.clone(), "N".into(), "Mean".into(), "SD".into(), "95% CI lower".into(), "95% CI upper".into()], rows, footnotes: Vec::new() });
        return Ok(b);
    }
    let mut tally = Tally::default();
    let mut counts = vec![0usize; levels.len()];
    for c in &cs {
        if !tally.check(c, name) {
            counts[c.value().expect("checked")] += 1;
        }
    }
    let n: usize = counts.iter().sum();
    let mut b = result(spec, format!("Bar Chart: {l}"), tally.accounting(cs.len(), n), format!("Each bar is the {} of valid cases in the category.", if spec.bar_stat == BarStat::Percent { "percentage" } else { "number" }));
    if n == 0 {
        fail(&mut b, format!("{l} has no valid values to draw."));
        return Ok(b);
    }
    let pct = spec.bar_stat == BarStat::Percent;
    let cats: Vec<(String, f64)> = levels.iter().zip(&counts).map(|(lv, c)| (lv.label.clone(), if pct { 100.0 * *c as f64 / n as f64 } else { *c as f64 })).collect();
    let alt = format!("Bar chart of {l} ({} of {n} cases): {}.", if pct { "percentages" } else { "counts" }, cats.iter().map(|(name, v)| format!("{name} {}", if pct { format!("{}%", apa::num(*v, 1)) } else { apa::plain(*v) })).collect::<Vec<_>>().join(", "));
    b.charts.push(chart("bar", &l, alt.clone(), chart::bars(&cats, None, &l, if pct { "Percent" } else { "Frequency" }, &alt)));
    b.tables.push(Table { id: "counts".into(), title: "Counts".into(), columns: vec![l, "Count".into(), "Percent".into()], rows: levels.iter().zip(&counts).map(|(lv, c)| vec![text(&lv.label), count(*c), num(100.0 * *c as f64 / n as f64, 1)]).collect(), footnotes: Vec::new() });
    Ok(b)
}

fn scatter(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    let (Some(xn), Some(yn)) = (&spec.x, &spec.y) else { return Err(Error::Invalid("Choose a variable for each axis.".into())) };
    let (xl, yl) = (label(ds, xn)?, label(ds, yn)?);
    let (x, y, tally, total) = super::ttest_paired::matched(ds, xn, yn)?;
    let mut b = result(spec, format!("Scatterplot: {yl} by {xl}"), tally.accounting(total, x.len()), format!("One point per case with both values.{}", if spec.line { " The line is the least-squares fit of the vertical variable on the horizontal one." } else { "" }));
    if x.is_empty() {
        fail(&mut b, "No case has both values.");
        return Ok(b);
    }
    let points: Vec<(f64, f64)> = x.iter().copied().zip(y.iter().copied()).collect();
    let line = if spec.line { prismqda_stats::charts::least_squares_line(&x, &y) } else { None };
    let alt = format!("Scatterplot of {yl} (vertical) against {xl} (horizontal) for {} cases{}.", x.len(), line.map(|l| format!(", with a fitted line of slope {}", apa::num(l.slope, 2))).unwrap_or_default());
    b.charts.push(chart("scatter", &format!("{yl} by {xl}"), alt.clone(), chart::scatter(&points, line.map(|l| (l.intercept, l.slope)), &xl, &yl, &alt)));
    if let Some(l) = line {
        let r = prismqda_stats::correlation::pearson(&x, &y, 0.95).map_or(f64::NAN, |c| c.r);
        b.tables.push(Table { id: "line".into(), title: "Fitted line".into(), columns: vec!["N".into(), "Intercept".into(), "Slope".into(), "r".into()], rows: vec![vec![count(x.len()), num(l.intercept, 3), num(l.slope, 3), num(r, 3)]], footnotes: Vec::new() });
    } else {
        b.tables.push(Table { id: "n".into(), title: "Cases".into(), columns: vec!["N".into()], rows: vec![vec![count(x.len())]], footnotes: Vec::new() });
    }
    Ok(b)
}
