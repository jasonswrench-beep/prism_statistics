//! Charts as SVG, drawn the way APA figures are: white ground, black axes and text, no grid lines or chartjunk, axis
//! titles from the variable labels, tick values at round numbers, and greys rather than colors so a chart prints. The
//! same SVG is shown in the window, embedded in HTML, and (with its text turned into outlines) placed in Word and PDF.
//!
//! Text is set in Arial at 13 px (Helvetica and other sans serifs as fallbacks), per APA's advice for figures. Every
//! value written into the SVG is escaped.

use std::fmt::Write as _;

pub const WIDTH: f64 = 640.0;
pub const HEIGHT: f64 = 420.0;
const LEFT: f64 = 78.0;
const RIGHT: f64 = 24.0;
const TOP: f64 = 20.0;
const BOTTOM: f64 = 64.0;
const FONT: &str = "Arial, Helvetica, 'Liberation Sans', sans-serif";
const FILL: &str = "#BDBDBD";
const DARK: &str = "#4D4D4D";

/// A chart in a result: its SVG, a title (the figure title in exports) and alternative text for screen readers.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Chart {
    pub id: String,
    pub title: String,
    pub alt: String,
    pub svg: String,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Round tick values covering [lo, hi]: steps of 1, 2 or 5 times a power of ten, about `n` of them.
pub fn ticks(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    if !(lo.is_finite() && hi.is_finite()) {
        return Vec::new();
    }
    let (lo, hi) = if hi > lo { (lo, hi) } else { (lo - 1.0, hi + 1.0) };
    let raw = (hi - lo) / n.max(2) as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|m| m * mag).find(|s| *s >= raw).unwrap_or(10.0 * mag);
    let start = (lo / step).floor() * step;
    let mut out = Vec::new();
    let mut k = 0.0;
    loop {
        let v = start + k * step;
        // Snap away floating-point noise (0.30000000000000004).
        let v = (v / step).round() * step;
        out.push(if v.abs() < step * 1e-9 { 0.0 } else { v });
        if v >= hi - step * 1e-9 {
            break;
        }
        k += 1.0;
        if k > 1000.0 {
            break;
        }
    }
    out
}

/// A tick label: as few decimals as the step needs, with a true minus sign.
fn tick_text(v: f64, step: f64) -> String {
    // As many decimals as the step has (0.25 -> 2, 0.2 -> 1, 5 -> 0).
    let st = format!("{:.6}", step.abs());
    let decimals = st.trim_end_matches('0').split('.').nth(1).map_or(0, str::len);
    let s = format!("{v:.decimals$}");
    if s.trim_start_matches('-').chars().all(|c| c == '0' || c == '.') {
        return s.trim_start_matches('-').to_string();
    }
    s.replace('-', "\u{2212}")
}

/// A linear scale from data values to pixels.
#[derive(Clone, Copy)]
struct Scale {
    d0: f64,
    d1: f64,
    p0: f64,
    p1: f64,
}

impl Scale {
    fn at(&self, v: f64) -> f64 {
        if self.d1 == self.d0 {
            return (self.p0 + self.p1) / 2.0;
        }
        self.p0 + (v - self.d0) / (self.d1 - self.d0) * (self.p1 - self.p0)
    }
}

/// The frame every chart shares: the SVG element, axes, ticks and axis titles. Returns the SVG so far and the scales.
struct Frame {
    out: String,
    x: Scale,
    y: Scale,
}

fn open(alt: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {WIDTH} {HEIGHT}\" width=\"{WIDTH}\" height=\"{HEIGHT}\" role=\"img\" aria-label=\"{a}\" font-family=\"{FONT}\" font-size=\"13\"><title>{a}</title><rect width=\"{WIDTH}\" height=\"{HEIGHT}\" fill=\"#FFFFFF\"/>",
        a = esc(alt)
    )
}

/// A frame with numeric axes. `xt`/`yt` are the tick values; the axes span the first to the last tick.
fn frame(alt: &str, xt: &[f64], yt: &[f64], xlabel: &str, ylabel: &str) -> Frame {
    let (x0, x1) = (*xt.first().unwrap_or(&0.0), *xt.last().unwrap_or(&1.0));
    let (y0, y1) = (*yt.first().unwrap_or(&0.0), *yt.last().unwrap_or(&1.0));
    let x = Scale { d0: x0, d1: x1, p0: LEFT, p1: WIDTH - RIGHT };
    let y = Scale { d0: y0, d1: y1, p0: HEIGHT - BOTTOM, p1: TOP };
    let mut out = open(alt);
    axes(&mut out, Some((xt, x)), (yt, y), xlabel, ylabel);
    Frame { out, x, y }
}

/// Draws the axes, ticks and titles. A None x axis is drawn by the caller (categories).
fn axes(out: &mut String, x: Option<(&[f64], Scale)>, y: (&[f64], Scale), xlabel: &str, ylabel: &str) {
    let (yt, ys) = y;
    let base = HEIGHT - BOTTOM;
    let ystep = if yt.len() > 1 { yt[1] - yt[0] } else { 1.0 };
    let _ = write!(out, "<g stroke=\"#000\" stroke-width=\"1\" fill=\"none\"><line x1=\"{LEFT}\" y1=\"{TOP}\" x2=\"{LEFT}\" y2=\"{base}\"/><line x1=\"{LEFT}\" y1=\"{base}\" x2=\"{}\" y2=\"{base}\"/>", WIDTH - RIGHT);
    for v in yt {
        let p = ys.at(*v);
        let _ = write!(out, "<line x1=\"{}\" y1=\"{p:.2}\" x2=\"{LEFT}\" y2=\"{p:.2}\"/>", LEFT - 5.0);
    }
    if let Some((xt, xs)) = x {
        for v in xt {
            let p = xs.at(*v);
            let _ = write!(out, "<line x1=\"{p:.2}\" y1=\"{base}\" x2=\"{p:.2}\" y2=\"{}\"/>", base + 5.0);
        }
    }
    out.push_str("</g><g fill=\"#000\">");
    for v in yt {
        let _ = write!(out, "<text x=\"{}\" y=\"{:.2}\" text-anchor=\"end\" dominant-baseline=\"middle\" dy=\"0.35em\">{}</text>", LEFT - 9.0, ys.at(*v), tick_text(*v, ystep));
    }
    if let Some((xt, xs)) = x {
        let xstep = if xt.len() > 1 { xt[1] - xt[0] } else { 1.0 };
        for v in xt {
            let _ = write!(out, "<text x=\"{:.2}\" y=\"{}\" text-anchor=\"middle\">{}</text>", xs.at(*v), base + 21.0, tick_text(*v, xstep));
        }
    }
    let _ = write!(out, "<text x=\"{:.2}\" y=\"{}\" text-anchor=\"middle\">{}</text>", (LEFT + WIDTH - RIGHT) / 2.0, HEIGHT - 16.0, esc(xlabel));
    let _ = write!(out, "<text transform=\"translate(20 {:.2}) rotate(-90)\" text-anchor=\"middle\">{}</text>", (TOP + base) / 2.0, esc(ylabel));
    out.push_str("</g>");
}

fn close(mut out: String) -> String {
    out.push_str("</svg>");
    out
}

fn bounds(values: impl Iterator<Item = f64>) -> Option<(f64, f64)> {
    values.filter(|v| v.is_finite()).fold(None, |acc, v| Some(acc.map_or((v, v), |(lo, hi): (f64, f64)| (lo.min(v), hi.max(v)))))
}

/// A histogram: bars from bin edges and counts, with an optional normal curve (mean, SD, n) scaled to the counts.
pub fn histogram(bins: &[(f64, f64, usize)], normal: Option<(f64, f64, usize)>, xlabel: &str, alt: &str) -> String {
    let (lo, hi) = (bins.first().map_or(0.0, |b| b.0), bins.last().map_or(1.0, |b| b.1));
    let width = bins.first().map_or(1.0, |b| b.1 - b.0);
    let curve = |x: f64| normal.map_or(0.0, |(m, s, n)| n as f64 * width * (-(x - m).powi(2) / (2.0 * s * s)).exp() / (s * (2.0 * std::f64::consts::PI).sqrt()));
    let top = bins.iter().map(|b| b.2 as f64).fold(0.0, f64::max).max(if normal.is_some() { (0..=60).map(|i| curve(lo + (hi - lo) * i as f64 / 60.0)).fold(0.0, f64::max) } else { 0.0 });
    let xt = ticks(lo, hi, 7);
    let yt = ticks(0.0, top.max(1.0), 5);
    let mut fr = frame(alt, &xt, &yt, xlabel, "Frequency");
    fr.out.push_str(&format!("<g fill=\"{FILL}\" stroke=\"#000\" stroke-width=\"0.75\">"));
    for (a, b, c) in bins {
        let (x0, x1, y0, y1) = (fr.x.at(*a), fr.x.at(*b), fr.y.at(*c as f64), fr.y.at(0.0));
        let _ = write!(fr.out, "<rect x=\"{x0:.2}\" y=\"{y0:.2}\" width=\"{:.2}\" height=\"{:.2}\"/>", x1 - x0, y1 - y0);
    }
    fr.out.push_str("</g>");
    if normal.is_some() {
        let pts: Vec<String> = (0..=120).map(|i| { let x = xt[0] + (xt[xt.len() - 1] - xt[0]) * i as f64 / 120.0; format!("{:.2},{:.2}", fr.x.at(x), fr.y.at(curve(x))) }).collect();
        let _ = write!(fr.out, "<polyline fill=\"none\" stroke=\"#000\" stroke-width=\"1.5\" points=\"{}\"/>", pts.join(" "));
    }
    close(fr.out)
}

/// One box of a box plot.
pub struct BoxDraw {
    pub label: String,
    pub lower_whisker: f64,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub upper_whisker: f64,
    pub outliers: Vec<f64>,
}

/// Box plots side by side, one per group: box from Q1 to Q3, median line, whiskers to the most extreme values within
/// 1.5 IQR, circles for values beyond.
pub fn boxplot(boxes: &[BoxDraw], xlabel: &str, ylabel: &str, alt: &str) -> String {
    let (lo, hi) = bounds(boxes.iter().flat_map(|b| [b.lower_whisker, b.upper_whisker].into_iter().chain(b.outliers.iter().copied()))).unwrap_or((0.0, 1.0));
    let yt = ticks(lo, hi, 6);
    let ys = Scale { d0: yt[0], d1: yt[yt.len() - 1], p0: HEIGHT - BOTTOM, p1: TOP };
    let mut out = open(alt);
    axes(&mut out, None, (&yt, ys), xlabel, ylabel);
    let slot = (WIDTH - RIGHT - LEFT) / boxes.len().max(1) as f64;
    let half = (slot * 0.3).min(40.0);
    out.push_str("<g stroke=\"#000\" stroke-width=\"1\">");
    for (i, b) in boxes.iter().enumerate() {
        let c = LEFT + slot * (i as f64 + 0.5);
        let (yq1, yq3, ymd, ylo, yhi) = (ys.at(b.q1), ys.at(b.q3), ys.at(b.median), ys.at(b.lower_whisker), ys.at(b.upper_whisker));
        let _ = write!(out, "<line x1=\"{c:.2}\" y1=\"{ylo:.2}\" x2=\"{c:.2}\" y2=\"{yq1:.2}\"/><line x1=\"{c:.2}\" y1=\"{yq3:.2}\" x2=\"{c:.2}\" y2=\"{yhi:.2}\"/>");
        let _ = write!(out, "<line x1=\"{:.2}\" y1=\"{ylo:.2}\" x2=\"{:.2}\" y2=\"{ylo:.2}\"/><line x1=\"{:.2}\" y1=\"{yhi:.2}\" x2=\"{:.2}\" y2=\"{yhi:.2}\"/>", c - half / 2.0, c + half / 2.0, c - half / 2.0, c + half / 2.0);
        let _ = write!(out, "<rect x=\"{:.2}\" y=\"{yq3:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{FILL}\"/>", c - half, 2.0 * half, (yq1 - yq3).max(0.5));
        let _ = write!(out, "<line x1=\"{:.2}\" y1=\"{ymd:.2}\" x2=\"{:.2}\" y2=\"{ymd:.2}\" stroke-width=\"2.5\"/>", c - half, c + half);
        for o in &b.outliers {
            let _ = write!(out, "<circle cx=\"{c:.2}\" cy=\"{:.2}\" r=\"3.5\" fill=\"#FFFFFF\"/>", ys.at(*o));
        }
        let _ = write!(out, "<line x1=\"{c:.2}\" y1=\"{}\" x2=\"{c:.2}\" y2=\"{}\"/>", HEIGHT - BOTTOM, HEIGHT - BOTTOM + 5.0);
    }
    out.push_str("</g><g fill=\"#000\">");
    for (i, b) in boxes.iter().enumerate() {
        let _ = write!(out, "<text x=\"{:.2}\" y=\"{}\" text-anchor=\"middle\">{}</text>", LEFT + slot * (i as f64 + 0.5), HEIGHT - BOTTOM + 21.0, esc(&b.label));
    }
    out.push_str("</g>");
    close(out)
}

/// Vertical bars for categories, with optional error bars (lower, upper) such as a 95% confidence interval.
pub fn bars(cats: &[(String, f64)], errors: Option<&[(f64, f64)]>, xlabel: &str, ylabel: &str, alt: &str) -> String {
    let top = cats.iter().map(|c| c.1).chain(errors.into_iter().flatten().map(|e| e.1)).fold(0.0, f64::max);
    let bottom = cats.iter().map(|c| c.1).chain(errors.into_iter().flatten().map(|e| e.0)).fold(0.0, f64::min);
    let yt = ticks(bottom, top.max(bottom + 1e-9), 5);
    let ys = Scale { d0: yt[0], d1: yt[yt.len() - 1], p0: HEIGHT - BOTTOM, p1: TOP };
    let mut out = open(alt);
    axes(&mut out, None, (&yt, ys), xlabel, ylabel);
    let slot = (WIDTH - RIGHT - LEFT) / cats.len().max(1) as f64;
    let half = (slot * 0.32).min(46.0);
    out.push_str(&format!("<g fill=\"{FILL}\" stroke=\"#000\" stroke-width=\"0.75\">"));
    for (i, (_, v)) in cats.iter().enumerate() {
        let c = LEFT + slot * (i as f64 + 0.5);
        let (y0, y1) = (ys.at(v.max(0.0)), ys.at(v.min(0.0)));
        let _ = write!(out, "<rect x=\"{:.2}\" y=\"{y0:.2}\" width=\"{:.2}\" height=\"{:.2}\"/>", c - half, 2.0 * half, (y1 - y0).max(0.0));
    }
    out.push_str("</g><g stroke=\"#000\" stroke-width=\"1.25\">");
    if let Some(errs) = errors {
        for (i, (lo, hi)) in errs.iter().enumerate() {
            if !(lo.is_finite() && hi.is_finite()) {
                continue;
            }
            let c = LEFT + slot * (i as f64 + 0.5);
            let (a, b) = (ys.at(*lo), ys.at(*hi));
            let _ = write!(out, "<line x1=\"{c:.2}\" y1=\"{a:.2}\" x2=\"{c:.2}\" y2=\"{b:.2}\"/><line x1=\"{:.2}\" y1=\"{a:.2}\" x2=\"{:.2}\" y2=\"{a:.2}\"/><line x1=\"{:.2}\" y1=\"{b:.2}\" x2=\"{:.2}\" y2=\"{b:.2}\"/>", c - 7.0, c + 7.0, c - 7.0, c + 7.0);
        }
    }
    for i in 0..cats.len() {
        let c = LEFT + slot * (i as f64 + 0.5);
        let _ = write!(out, "<line x1=\"{c:.2}\" y1=\"{}\" x2=\"{c:.2}\" y2=\"{}\" stroke-width=\"1\"/>", HEIGHT - BOTTOM, HEIGHT - BOTTOM + 5.0);
    }
    out.push_str("</g><g fill=\"#000\">");
    for (i, (label, _)) in cats.iter().enumerate() {
        let _ = write!(out, "<text x=\"{:.2}\" y=\"{}\" text-anchor=\"middle\">{}</text>", LEFT + slot * (i as f64 + 0.5), HEIGHT - BOTTOM + 21.0, esc(label));
    }
    out.push_str("</g>");
    close(out)
}

/// A scatterplot with an optional straight line (intercept, slope) drawn across the x range.
pub fn scatter(points: &[(f64, f64)], line: Option<(f64, f64)>, xlabel: &str, ylabel: &str, alt: &str) -> String {
    let (x0, x1) = bounds(points.iter().map(|p| p.0)).unwrap_or((0.0, 1.0));
    let (y0, y1) = bounds(points.iter().map(|p| p.1)).unwrap_or((0.0, 1.0));
    let xt = ticks(x0, x1, 7);
    let yt = ticks(y0, y1, 6);
    let mut fr = frame(alt, &xt, &yt, xlabel, ylabel);
    fr.out.push_str("<g fill=\"none\" stroke=\"#000\" stroke-width=\"1\">");
    for (x, y) in points {
        let _ = write!(fr.out, "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"3.5\"/>", fr.x.at(*x), fr.y.at(*y));
    }
    fr.out.push_str("</g>");
    if let Some((a, b)) = line {
        let (lx0, lx1) = (xt[0], xt[xt.len() - 1]);
        // Clip the line to the plotting area.
        let (ya, yb) = (a + b * lx0, a + b * lx1);
        let (ylo, yhi) = (yt[0], yt[yt.len() - 1]);
        let clip = |x: f64, y: f64, other: f64| -> (f64, f64) {
            if y < ylo && b != 0.0 { ((ylo - a) / b, ylo) } else if y > yhi && b != 0.0 { ((yhi - a) / b, yhi) } else { let _ = other; (x, y) }
        };
        let (p0, p1) = (clip(lx0, ya, lx1), clip(lx1, yb, lx0));
        let _ = write!(fr.out, "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{DARK}\" stroke-width=\"1.75\"/>", fr.x.at(p0.0), fr.y.at(p0.1), fr.x.at(p1.0), fr.y.at(p1.1));
    }
    close(fr.out)
}

/// A scatterplot with fitted curves over it: each curve is a list of (x, y) points along the x range, drawn with its own
/// dash pattern and named in a legend at the top left. A curve leaving the plotting area is broken there, not stretched.
pub fn scatter_curves(points: &[(f64, f64)], curves: &[(String, Vec<(f64, f64)>)], xlabel: &str, ylabel: &str, alt: &str) -> String {
    const DASH: [&str; 8] = ["", "6 4", "2 3", "8 3 2 3", "12 4", "1 5", "8 2 2 2 2 2", "4 2"];
    // With no points, the curves set the axes.
    let all: Vec<(f64, f64)> = if points.is_empty() { curves.iter().flat_map(|c| c.1.iter().copied()).filter(|p| p.1.is_finite()).collect() } else { points.to_vec() };
    let (x0, x1) = bounds(all.iter().map(|p| p.0)).unwrap_or((0.0, 1.0));
    let (y0, y1) = bounds(all.iter().map(|p| p.1)).unwrap_or((0.0, 1.0));
    let xt = ticks(x0, x1, 7);
    let yt = ticks(y0, y1, 6);
    let (ylo, yhi) = (yt[0], yt[yt.len() - 1]);
    let mut fr = frame(alt, &xt, &yt, xlabel, ylabel);
    fr.out.push_str("<g fill=\"none\" stroke=\"#7A7A7A\" stroke-width=\"1\">");
    for (x, y) in points {
        let _ = write!(fr.out, "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"3\"/>", fr.x.at(*x), fr.y.at(*y));
    }
    fr.out.push_str("</g>");
    for (k, (_, pts)) in curves.iter().enumerate() {
        let dash = DASH[k % DASH.len()];
        let dash = if dash.is_empty() { String::new() } else { format!(" stroke-dasharray=\"{dash}\"") };
        let mut run: Vec<String> = Vec::new();
        let flush = |run: &mut Vec<String>, out: &mut String| {
            if run.len() > 1 {
                let _ = write!(out, "<polyline fill=\"none\" stroke=\"#000\" stroke-width=\"1.75\"{dash} points=\"{}\"/>", run.join(" "));
            }
            run.clear();
        };
        for (x, y) in pts {
            if y.is_finite() && *y >= ylo && *y <= yhi && *x >= xt[0] && *x <= xt[xt.len() - 1] {
                run.push(format!("{:.2},{:.2}", fr.x.at(*x), fr.y.at(*y)));
            } else {
                flush(&mut run, &mut fr.out);
            }
        }
        flush(&mut run, &mut fr.out);
    }
    if !curves.is_empty() {
        let (lx, ly) = (LEFT + 12.0, TOP + 8.0);
        let width = 44.0 + 7.5 * curves.iter().map(|c| c.0.chars().count()).max().unwrap_or(0) as f64;
        let _ = write!(fr.out, "<rect x=\"{lx}\" y=\"{ly}\" width=\"{width:.1}\" height=\"{:.1}\" fill=\"#FFFFFF\" stroke=\"#000\" stroke-width=\"0.75\"/>", 10.0 + 20.0 * curves.len() as f64);
        for (k, (name, _)) in curves.iter().enumerate() {
            let yy = ly + 15.0 + 20.0 * k as f64;
            let dash = DASH[k % DASH.len()];
            let _ = write!(fr.out, "<line x1=\"{}\" y1=\"{yy}\" x2=\"{}\" y2=\"{yy}\" stroke=\"#000\" stroke-width=\"1.75\"{}/>", lx + 8.0, lx + 34.0, if dash.is_empty() { String::new() } else { format!(" stroke-dasharray=\"{dash}\"") });
            let _ = write!(fr.out, "<text x=\"{}\" y=\"{}\" fill=\"#000\">{}</text>", lx + 40.0, yy + 4.0, esc(name));
        }
    }
    close(fr.out)
}

/// A line through points, with an optional diagonal from (0, 0) to (1, 1) (an ROC curve).
pub fn line_xy(points: &[(f64, f64)], diagonal: bool, xlabel: &str, ylabel: &str, alt: &str) -> String {
    let (x0, x1) = bounds(points.iter().map(|p| p.0)).unwrap_or((0.0, 1.0));
    let (y0, y1) = bounds(points.iter().map(|p| p.1)).unwrap_or((0.0, 1.0));
    let xt = ticks(x0.min(0.0), x1.max(if diagonal { 1.0 } else { x1 }), 5);
    let yt = ticks(y0.min(0.0), y1.max(if diagonal { 1.0 } else { y1 }), 5);
    let mut fr = frame(alt, &xt, &yt, xlabel, ylabel);
    if diagonal {
        let _ = write!(fr.out, "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{DARK}\" stroke-width=\"1\" stroke-dasharray=\"5 4\"/>", fr.x.at(0.0), fr.y.at(0.0), fr.x.at(1.0), fr.y.at(1.0));
    }
    let pts: Vec<String> = points.iter().map(|(x, y)| format!("{:.2},{:.2}", fr.x.at(*x), fr.y.at(*y))).collect();
    let _ = write!(fr.out, "<polyline fill=\"none\" stroke=\"#000\" stroke-width=\"1.75\" points=\"{}\"/>", pts.join(" "));
    close(fr.out)
}

/// A profile plot: categories across, one line (with its own dash pattern and marker) per series, and a legend.
pub fn profile(cats: &[String], series: &[(String, Vec<f64>)], xlabel: &str, ylabel: &str, legend_title: &str, alt: &str) -> String {
    let (lo, hi) = bounds(series.iter().flat_map(|s| s.1.iter().copied())).unwrap_or((0.0, 1.0));
    let yt = ticks(lo, hi, 6);
    let ys = Scale { d0: yt[0], d1: yt[yt.len() - 1], p0: HEIGHT - BOTTOM, p1: TOP };
    let mut out = open(alt);
    axes(&mut out, None, (&yt, ys), xlabel, ylabel);
    let right = WIDTH - RIGHT - 130.0;
    let slot = (right - LEFT) / cats.len().max(1) as f64;
    let xc = |i: usize| LEFT + slot * (i as f64 + 0.5);
    const DASH: [&str; 4] = ["", "6 4", "2 3", "8 3 2 3"];
    for (k, (_, vals)) in series.iter().enumerate() {
        let pts: Vec<String> = vals.iter().enumerate().filter(|(_, v)| v.is_finite()).map(|(i, v)| format!("{:.2},{:.2}", xc(i), ys.at(*v))).collect();
        let dash = DASH[k % DASH.len()];
        let _ = write!(out, "<polyline fill=\"none\" stroke=\"#000\" stroke-width=\"1.75\"{} points=\"{}\"/>", if dash.is_empty() { String::new() } else { format!(" stroke-dasharray=\"{dash}\"") }, pts.join(" "));
        for (i, v) in vals.iter().enumerate().filter(|(_, v)| v.is_finite()) {
            let (cx, cy) = (xc(i), ys.at(*v));
            let _ = match k % 3 {
                0 => write!(out, "<circle cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"4\" fill=\"#000\"/>"),
                1 => write!(out, "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"8\" height=\"8\" fill=\"#FFFFFF\" stroke=\"#000\"/>", cx - 4.0, cy - 4.0),
                _ => write!(out, "<path d=\"M{cx:.2} {:.2} L{:.2} {:.2} L{:.2} {:.2} Z\" fill=\"{FILL}\" stroke=\"#000\"/>", cy - 5.0, cx + 5.0, cy + 4.0, cx - 5.0, cy + 4.0),
            };
        }
    }
    out.push_str("<g stroke=\"#000\" stroke-width=\"1\">");
    for i in 0..cats.len() {
        let _ = write!(out, "<line x1=\"{:.2}\" y1=\"{}\" x2=\"{:.2}\" y2=\"{}\"/>", xc(i), HEIGHT - BOTTOM, xc(i), HEIGHT - BOTTOM + 5.0);
    }
    out.push_str("</g><g fill=\"#000\">");
    for (i, c) in cats.iter().enumerate() {
        let _ = write!(out, "<text x=\"{:.2}\" y=\"{}\" text-anchor=\"middle\">{}</text>", xc(i), HEIGHT - BOTTOM + 21.0, esc(c));
    }
    let lx = right + 18.0;
    let _ = write!(out, "<text x=\"{lx}\" y=\"{}\" font-weight=\"bold\">{}</text>", TOP + 12.0, esc(legend_title));
    for (k, (name, _)) in series.iter().enumerate() {
        let yy = TOP + 34.0 + 22.0 * k as f64;
        let dash = DASH[k % DASH.len()];
        let _ = write!(out, "<line x1=\"{lx}\" y1=\"{yy}\" x2=\"{}\" y2=\"{yy}\" stroke=\"#000\" stroke-width=\"1.75\"{}/>", lx + 26.0, if dash.is_empty() { String::new() } else { format!(" stroke-dasharray=\"{dash}\"") });
        let _ = write!(out, "<text x=\"{}\" y=\"{}\">{}</text>", lx + 32.0, yy + 4.0, esc(name));
    }
    out.push_str("</g>");
    close(out)
}

/// One row of a forest plot: a study (a square sized by its weight) or a summary (a diamond).
pub struct ForestRow {
    pub label: String,
    pub est: f64,
    pub lo: f64,
    pub hi: f64,
    /// The study's share of the weight (0 to 1); None draws a summary diamond.
    pub weight: Option<f64>,
    /// The values printed at the right, such as "0.42 [0.10, 0.74]".
    pub right: String,
}

/// A forest plot: one row per study with its estimate and interval, summaries as diamonds, a vertical line at the
/// null value. With `exp_ticks` the axis is on the log scale and its ticks are labeled with the exponentiated values
/// (odds and risk ratios).
pub fn forest(rows: &[ForestRow], null: f64, xlabel: &str, right_title: &str, exp_ticks: bool, alt: &str) -> String {
    const ROW: f64 = 22.0;
    const LABELS: f64 = 190.0;
    const VALUES: f64 = 170.0;
    let width = WIDTH;
    let top = TOP + 24.0;
    let base = top + ROW * rows.len() as f64 + 8.0;
    let height = base + 56.0;
    let (lo, hi) = bounds(rows.iter().flat_map(|r| [r.lo, r.hi]).chain([null])).unwrap_or((-1.0, 1.0));
    let xt = if exp_ticks { log_ticks(lo, hi) } else { ticks(lo, hi, 5) };
    let x = Scale { d0: xt[0], d1: xt[xt.len() - 1], p0: LABELS, p1: width - VALUES - 12.0 };
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width} {height:.0}\" width=\"{width}\" height=\"{height:.0}\" role=\"img\" aria-label=\"{a}\" font-family=\"{FONT}\" font-size=\"13\"><title>{a}</title><rect width=\"{width}\" height=\"{height:.0}\" fill=\"#FFFFFF\"/>",
        a = esc(alt)
    );
    let _ = write!(out, "<g fill=\"#000\"><text x=\"8\" y=\"{TOP:.1}\" dy=\"0.8em\" font-weight=\"bold\">Study</text><text x=\"{:.1}\" y=\"{TOP:.1}\" dy=\"0.8em\" text-anchor=\"end\" font-weight=\"bold\">{}</text></g>", width - 8.0, esc(right_title));
    // The null line and the axis.
    let _ = write!(out, "<g stroke=\"#000\" stroke-width=\"1\"><line x1=\"{:.2}\" y1=\"{top:.1}\" x2=\"{:.2}\" y2=\"{base:.1}\" stroke-dasharray=\"4 3\"/><line x1=\"{:.2}\" y1=\"{base:.1}\" x2=\"{:.2}\" y2=\"{base:.1}\"/>", x.at(null), x.at(null), x.p0, x.p1);
    for v in &xt {
        let _ = write!(out, "<line x1=\"{p:.2}\" y1=\"{base:.1}\" x2=\"{p:.2}\" y2=\"{:.1}\"/>", base + 5.0, p = x.at(*v));
    }
    out.push_str("</g><g fill=\"#000\">");
    let step = if xt.len() > 1 { xt[1] - xt[0] } else { 1.0 };
    for v in &xt {
        let label = if exp_ticks { exp_text(v.exp()) } else { tick_text(*v, step) };
        let _ = write!(out, "<text x=\"{:.2}\" y=\"{:.1}\" text-anchor=\"middle\">{label}</text>", x.at(*v), base + 21.0);
    }
    let _ = write!(out, "<text x=\"{:.2}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text></g>", (x.p0 + x.p1) / 2.0, height - 12.0, esc(xlabel));
    let biggest = rows.iter().filter_map(|r| r.weight).fold(0.0f64, f64::max).max(1e-12);
    for (i, r) in rows.iter().enumerate() {
        let y = top + ROW * (i as f64 + 0.5);
        let bold = if r.weight.is_none() { " font-weight=\"bold\"" } else { "" };
        let _ = write!(out, "<text x=\"8\" y=\"{y:.1}\" dy=\"0.35em\" fill=\"#000\"{bold}>{}</text>", esc(&r.label));
        let _ = write!(out, "<text x=\"{:.1}\" y=\"{y:.1}\" dy=\"0.35em\" text-anchor=\"end\" fill=\"#000\"{bold}>{}</text>", width - 8.0, esc(&r.right));
        let clip = |v: f64| x.at(v.clamp(x.d0, x.d1));
        match r.weight {
            Some(w) => {
                let _ = write!(out, "<line x1=\"{:.2}\" y1=\"{y:.1}\" x2=\"{:.2}\" y2=\"{y:.1}\" stroke=\"#000\" stroke-width=\"1\"/>", clip(r.lo), clip(r.hi));
                let side = 4.0 + 8.0 * (w / biggest).sqrt();
                if r.est >= x.d0 && r.est <= x.d1 {
                    let _ = write!(out, "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{side:.2}\" height=\"{side:.2}\" fill=\"{DARK}\"/>", x.at(r.est) - side / 2.0, y - side / 2.0);
                }
            }
            None => {
                let _ = write!(out, "<polygon points=\"{:.2},{y:.1} {:.2},{:.1} {:.2},{y:.1} {:.2},{:.1}\" fill=\"{DARK}\" stroke=\"#000\" stroke-width=\"0.75\"/>", clip(r.lo), clip(r.est), y - 7.0, clip(r.hi), clip(r.est), y + 7.0);
            }
        }
    }
    close(out)
}

/// Round values on the ratio scale (0.1, 0.2, 0.5, 1, 2, 5, 10, …) covering [exp(lo), exp(hi)], as their logs.
fn log_ticks(lo: f64, hi: f64) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for e in -6..=6 {
        for m in [1.0, 2.0, 5.0] {
            let v = m * 10f64.powi(e);
            out.push(v.ln());
        }
    }
    let first = out.iter().rposition(|v| *v <= lo + 1e-12).unwrap_or(0);
    let last = out.iter().position(|v| *v >= hi - 1e-12).unwrap_or(out.len() - 1);
    let mut t = out[first..=last].to_vec();
    // Thin long runs so the labels don't collide.
    while t.len() > 7 {
        t = t.iter().step_by(2).copied().collect();
    }
    if t.len() < 2 {
        t = vec![lo, hi];
    }
    t
}

fn exp_text(v: f64) -> String {
    if v >= 1.0 {
        format!("{}", (v * 10.0).round() / 10.0)
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// A funnel plot: each study's effect against its standard error (0 at the top), the summary as a vertical line and
/// the pseudo 95% limits around it; filled-in studies (trim and fill) are hollow.
pub fn funnel(points: &[(f64, f64)], filled: &[(f64, f64)], center: f64, xlabel: &str, alt: &str) -> String {
    let max_se = bounds(points.iter().chain(filled).map(|p| p.1)).map_or(1.0, |b| b.1);
    let yt = ticks(0.0, max_se, 5);
    let ymax = yt[yt.len() - 1];
    let z = 1.959_963_984_540_054;
    let (x0, x1) = bounds(points.iter().chain(filled).map(|p| p.0).chain([center - z * ymax, center + z * ymax])).unwrap_or((-1.0, 1.0));
    let xt = ticks(x0, x1, 6);
    let x = Scale { d0: xt[0], d1: xt[xt.len() - 1], p0: LEFT, p1: WIDTH - RIGHT };
    let y = Scale { d0: 0.0, d1: ymax, p0: TOP, p1: HEIGHT - BOTTOM };
    let mut out = open(alt);
    axes(&mut out, Some((&xt, x)), (&yt, y), xlabel, "Standard error");
    let _ = write!(
        out,
        "<g fill=\"none\" stroke=\"#000\" stroke-width=\"1\"><polyline stroke-dasharray=\"4 3\" points=\"{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\"/><line x1=\"{c:.2}\" y1=\"{:.2}\" x2=\"{c:.2}\" y2=\"{:.2}\"/></g>",
        x.at(center - z * ymax),
        y.at(ymax),
        x.at(center),
        y.at(0.0),
        x.at(center + z * ymax),
        y.at(ymax),
        y.at(0.0),
        y.at(ymax),
        c = x.at(center)
    );
    out.push_str("<g stroke=\"#000\" stroke-width=\"1\">");
    for (v, s) in points {
        let _ = write!(out, "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"4\" fill=\"{DARK}\"/>", x.at(*v), y.at(*s));
    }
    for (v, s) in filled {
        let _ = write!(out, "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"4\" fill=\"#FFFFFF\"/>", x.at(*v), y.at(*s));
    }
    out.push_str("</g>");
    close(out)
}

/// A node of a decision-tree diagram, numbered as rpart numbers them (children of k are 2k and 2k + 1).
pub struct TreeNode {
    pub id: u64,
    /// The rule for the left branch (internal nodes).
    pub label: String,
    /// The rule for the right branch.
    pub right_label: String,
    /// The node's prediction, as text.
    pub value: String,
    pub n: usize,
    pub leaf: bool,
}

/// A decision tree: nodes as boxes (prediction and n), branches labeled with their rules, leaves along the bottom.
pub fn tree_diagram(nodes: &[TreeNode], alt: &str) -> String {
    const LEVEL: f64 = 96.0;
    const SLOT: f64 = 134.0;
    let depth = |id: u64| 63 - id.leading_zeros() as usize;
    let find = |id: u64| nodes.iter().position(|n| n.id == id);
    // Leaves in drawing order (left to right), then each internal node above the middle of its children.
    let mut x: Vec<f64> = vec![0.0; nodes.len()];
    let mut next = 0.0;
    fn place(i: usize, nodes: &[TreeNode], find: &dyn Fn(u64) -> Option<usize>, x: &mut [f64], next: &mut f64) {
        if nodes[i].leaf {
            x[i] = *next;
            *next += 1.0;
            return;
        }
        let (l, r) = (find(2 * nodes[i].id), find(2 * nodes[i].id + 1));
        if let (Some(l), Some(r)) = (l, r) {
            place(l, nodes, find, x, next);
            place(r, nodes, find, x, next);
            x[i] = (x[l] + x[r]) / 2.0;
        } else {
            x[i] = *next;
            *next += 1.0;
        }
    }
    if let Some(root) = find(1) {
        place(root, nodes, &find, &mut x, &mut next);
    }
    let leaves = next.max(1.0);
    let max_depth = nodes.iter().map(|n| depth(n.id)).max().unwrap_or(0) as f64;
    let width = (leaves * SLOT + 40.0).max(WIDTH);
    let height = TOP + 30.0 + max_depth * LEVEL + 60.0;
    let left_pad = (width - leaves * SLOT) / 2.0;
    let px = |i: usize| left_pad + (x[i] + 0.5) * SLOT;
    let py = |id: u64| TOP + 20.0 + depth(id) as f64 * LEVEL;
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width:.0} {height:.0}\" width=\"{width:.0}\" height=\"{height:.0}\" role=\"img\" aria-label=\"{a}\" font-family=\"{FONT}\" font-size=\"11\"><title>{a}</title><rect width=\"{width:.0}\" height=\"{height:.0}\" fill=\"#FFFFFF\"/>",
        a = esc(alt)
    );
    // Branches first, so boxes sit on top.
    out.push_str("<g stroke=\"#000\" stroke-width=\"1\" fill=\"none\">");
    for (i, nd) in nodes.iter().enumerate() {
        if nd.leaf {
            continue;
        }
        for (child, _) in [(2 * nd.id, &nd.label), (2 * nd.id + 1, &nd.right_label)] {
            if let Some(c) = find(child) {
                let _ = write!(out, "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"/>", px(i), py(nd.id) + 18.0, px(c), py(child) - 18.0);
            }
        }
    }
    out.push_str("</g><g fill=\"#000\">");
    for (i, nd) in nodes.iter().enumerate() {
        if nd.leaf {
            continue;
        }
        for (child, label) in [(2 * nd.id, &nd.label), (2 * nd.id + 1, &nd.right_label)] {
            if let Some(c) = find(child) {
                let (mx, my) = ((px(i) + px(c)) / 2.0, (py(nd.id) + 18.0 + py(child) - 18.0) / 2.0);
                let _ = write!(out, "<text x=\"{mx:.1}\" y=\"{my:.1}\" text-anchor=\"middle\" dy=\"0.35em\" paint-order=\"stroke\" stroke=\"#FFFFFF\" stroke-width=\"3\">{}</text>", esc(label));
            }
        }
    }
    out.push_str("</g>");
    for (i, nd) in nodes.iter().enumerate() {
        let (cx, cy) = (px(i), py(nd.id));
        let w = (SLOT - 10.0).min(6.4 * nd.value.chars().count().max(8) as f64 + 18.0);
        let fill = if nd.leaf { "#E6E6E6" } else { "#FFFFFF" };
        let _ = write!(
            out,
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{w:.1}\" height=\"36\" rx=\"4\" fill=\"{fill}\" stroke=\"#000\" stroke-width=\"1\"/><text x=\"{cx:.1}\" y=\"{:.1}\" text-anchor=\"middle\" fill=\"#000\" font-weight=\"bold\">{}</text><text x=\"{cx:.1}\" y=\"{:.1}\" text-anchor=\"middle\" fill=\"#000\">n = {}</text>",
            cx - w / 2.0,
            cy - 18.0,
            cy - 3.0,
            esc(&nd.value),
            cy + 12.0,
            nd.n
        );
    }
    close(out)
}

/// A control chart: the statistic by sample joined by a line, the center line, and the control limits (dashed, stepped
/// when they vary with the sample size); flagged points are drawn as filled squares.
#[allow(clippy::too_many_arguments)]
pub fn control_chart(points: &[(f64, f64)], center: &[f64], lcl: &[f64], ucl: &[f64], flags: &[bool], xlabel: &str, ylabel: &str, alt: &str) -> String {
    let all = points.iter().map(|p| p.1).chain(lcl.iter().copied()).chain(ucl.iter().copied());
    let (y0, y1) = bounds(all).unwrap_or((0.0, 1.0));
    let pad = (y1 - y0).max(1e-12) * 0.05;
    let yt = ticks(y0 - pad, y1 + pad, 6);
    let n = points.len().max(1) as f64;
    let xt = ticks(1.0, n, 8);
    let mut fr = frame(alt, &xt, &yt, xlabel, ylabel);
    let step = |vals: &[f64]| -> String {
        let mut s = String::new();
        for (i, v) in vals.iter().enumerate() {
            let x = points.get(i).map_or(i as f64 + 1.0, |p| p.0);
            let _ = write!(s, "{:.2},{:.2} {:.2},{:.2} ", fr.x.at(x - 0.5), fr.y.at(*v), fr.x.at(x + 0.5), fr.y.at(*v));
        }
        s
    };
    let _ = write!(fr.out, "<g fill=\"none\" stroke=\"#000\"><polyline stroke-width=\"1\" points=\"{}\"/><polyline stroke-width=\"1\" stroke-dasharray=\"6 4\" points=\"{}\"/><polyline stroke-width=\"1\" stroke-dasharray=\"6 4\" points=\"{}\"/></g>", step(center), step(lcl), step(ucl));
    let line: Vec<String> = points.iter().map(|(x, y)| format!("{:.2},{:.2}", fr.x.at(*x), fr.y.at(*y))).collect();
    let _ = write!(fr.out, "<polyline fill=\"none\" stroke=\"{DARK}\" stroke-width=\"1.25\" points=\"{}\"/>", line.join(" "));
    fr.out.push_str("<g stroke=\"#000\" stroke-width=\"1\">");
    for (i, (x, y)) in points.iter().enumerate() {
        if flags.get(i).copied().unwrap_or(false) {
            let _ = write!(fr.out, "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"8\" height=\"8\" fill=\"#000\"/>", fr.x.at(*x) - 4.0, fr.y.at(*y) - 4.0);
        } else {
            let _ = write!(fr.out, "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"3\" fill=\"#FFFFFF\"/>", fr.x.at(*x), fr.y.at(*y));
        }
    }
    fr.out.push_str("</g>");
    // Labels at the right end of the lines.
    if let (Some(c), Some(l), Some(u)) = (center.last(), lcl.last(), ucl.last()) {
        let xr = WIDTH - RIGHT - 2.0;
        let _ = write!(fr.out, "<g fill=\"#000\" font-size=\"11\" text-anchor=\"end\"><text x=\"{xr:.1}\" y=\"{:.1}\">UCL</text><text x=\"{xr:.1}\" y=\"{:.1}\">CL</text><text x=\"{xr:.1}\" y=\"{:.1}\">LCL</text></g>", fr.y.at(*u) - 4.0, fr.y.at(*c) - 4.0, fr.y.at(*l) - 4.0);
    }
    close(fr.out)
}

/// A CUSUM chart: the upper and lower cumulative sums by sample, the zero line and the decision interval ±h, with
/// signals as filled squares.
#[allow(clippy::too_many_arguments)]
pub fn cusum_chart(upper: &[f64], lower: &[f64], h: f64, up_flags: &[bool], lo_flags: &[bool], xlabel: &str, ylabel: &str, alt: &str) -> String {
    let all = upper.iter().chain(lower).copied().chain([h, -h]);
    let (y0, y1) = bounds(all).unwrap_or((-1.0, 1.0));
    let pad = (y1 - y0).max(1e-12) * 0.05;
    let yt = ticks(y0 - pad, y1 + pad, 6);
    let n = upper.len().max(1) as f64;
    let xt = ticks(1.0, n, 8);
    let mut fr = frame(alt, &xt, &yt, xlabel, ylabel);
    let (xa, xb) = (fr.x.at(0.5), fr.x.at(n + 0.5));
    let _ = write!(
        fr.out,
        "<g fill=\"none\" stroke=\"#000\" stroke-width=\"1\"><line x1=\"{xa:.2}\" x2=\"{xb:.2}\" y1=\"{0:.2}\" y2=\"{0:.2}\"/><line stroke-dasharray=\"6 4\" x1=\"{xa:.2}\" x2=\"{xb:.2}\" y1=\"{1:.2}\" y2=\"{1:.2}\"/><line stroke-dasharray=\"6 4\" x1=\"{xa:.2}\" x2=\"{xb:.2}\" y1=\"{2:.2}\" y2=\"{2:.2}\"/></g>",
        fr.y.at(0.0),
        fr.y.at(h),
        fr.y.at(-h)
    );
    for (vals, flags, dash) in [(upper, up_flags, ""), (lower, lo_flags, " stroke-dasharray=\"3 2\"")] {
        let line: Vec<String> = vals.iter().enumerate().map(|(i, y)| format!("{:.2},{:.2}", fr.x.at(i as f64 + 1.0), fr.y.at(*y))).collect();
        let _ = write!(fr.out, "<polyline fill=\"none\" stroke=\"{DARK}\" stroke-width=\"1.25\"{dash} points=\"{}\"/>", line.join(" "));
        fr.out.push_str("<g stroke=\"#000\" stroke-width=\"1\">");
        for (i, y) in vals.iter().enumerate() {
            let (x, y) = (fr.x.at(i as f64 + 1.0), fr.y.at(*y));
            if flags.get(i).copied().unwrap_or(false) {
                let _ = write!(fr.out, "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"8\" height=\"8\" fill=\"#000\"/>", x - 4.0, y - 4.0);
            } else {
                let _ = write!(fr.out, "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"3\" fill=\"#FFFFFF\"/>");
            }
        }
        fr.out.push_str("</g>");
    }
    let xr = WIDTH - RIGHT - 2.0;
    let _ = write!(fr.out, "<g fill=\"#000\" font-size=\"11\" text-anchor=\"end\"><text x=\"{xr:.1}\" y=\"{:.1}\">UDB</text><text x=\"{xr:.1}\" y=\"{:.1}\">LDB</text></g>", fr.y.at(h) - 4.0, fr.y.at(-h) + 13.0);
    close(fr.out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_are_round_and_cover_the_range() {
        assert_eq!(ticks(0.0, 10.0, 5), vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
        assert_eq!(ticks(3.2, 74.7, 7), vec![0.0, 20.0, 40.0, 60.0, 80.0]);
        let t = ticks(0.12, 0.93, 5);
        assert!(t[0] <= 0.12 && *t.last().unwrap() >= 0.93, "{t:?}");
        assert_eq!(tick_text(-2.0, 1.0), "\u{2212}2");
        assert_eq!(tick_text(0.25, 0.25), "0.25");
        assert_eq!(tick_text(0.2, 0.2), "0.2");
    }

    #[test]
    fn labels_are_escaped() {
        let s = scatter(&[(1.0, 2.0), (2.0, 3.0)], Some((1.0, 1.0)), "a<b", "\"y\" & z", "A & B");
        assert!(s.contains("a&lt;b") && s.contains("&quot;y&quot; &amp; z") && s.contains("aria-label=\"A &amp; B\""));
        assert!(s.starts_with("<svg") && s.ends_with("</svg>"));
    }
}
