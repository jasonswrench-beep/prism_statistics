//! Path diagrams (checklist C3): the structure of a model text as nodes and edges, an automatic layered layout, and the
//! diagram drawn as an APA figure (SVG) with estimates on the arrows and alternative text that describes it.
//!
//! The model text stays the source of truth. Statements a diagram doesn't draw (defined parameters, constraints,
//! intercepts, variances) are returned as `extras`, so an editor can rebuild the text from the diagram without losing
//! them.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use super::syntax::{self, Expr, Modifier, Op, Statement, Term};
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Observed,
    Latent,
    Composite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeKind {
    /// Latent → indicator (`=~`).
    Loading,
    /// Indicator → composite (`<~`).
    Composite,
    /// Predictor → outcome (`~`).
    Regression,
    /// Two-headed (`~~`).
    Covariance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub name: String,
    pub kind: NodeKind,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    /// The modifier as written before the variable ("a", "1", "NA", "start(0.5)", "c(a, b)"), if any.
    #[serde(default)]
    pub modifier: Option<String>,
}

impl Edge {
    /// The parameter as the results name it: "visual =~ x1", "y ~ x", "a ~~ b".
    pub fn param(&self) -> String {
        match self.kind {
            EdgeKind::Loading => format!("{} =~ {}", self.from, self.to),
            EdgeKind::Composite => format!("{} <~ {}", self.to, self.from),
            EdgeKind::Regression => format!("{} ~ {}", self.to, self.from),
            EdgeKind::Covariance => format!("{} ~~ {}", self.from, self.to),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Diagram {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// Statements the diagram doesn't draw, as text lines.
    pub extras: Vec<String>,
}

fn show_modifier(m: &Modifier) -> String {
    match m {
        Modifier::Fixed(v) => crate::model::format_code(*v),
        Modifier::Label(l) => l.clone(),
        Modifier::Free => "NA".into(),
        Modifier::Start(v) => format!("start({})", crate::model::format_code(*v)),
        Modifier::Groups(v) => format!("c({})", v.iter().map(show_modifier).collect::<Vec<_>>().join(", ")),
    }
}

fn show_term(t: &Term) -> String {
    let mut s = String::new();
    for m in &t.modifiers {
        s += &show_modifier(m);
        s.push('*');
    }
    s + &t.var
}

fn show_expr(e: &Expr) -> String {
    super::show(e)
}

/// The diagram of a model text (positions zero until laid out).
pub fn structure(text: &str) -> Result<Diagram> {
    let statements = syntax::parse(text)?;
    let mut d = Diagram::default();
    let add = |d: &mut Diagram, name: &str, kind: NodeKind| {
        if let Some(n) = d.nodes.iter_mut().find(|n| n.name == name) {
            if kind != NodeKind::Observed {
                n.kind = kind;
            }
        } else {
            d.nodes.push(Node { name: name.to_string(), kind, x: 0.0, y: 0.0 });
        }
    };
    // Latent and composite names first, so they are known before their indicators.
    for s in &statements {
        if let Statement::Relation { lhs, op: op @ (Op::Measured | Op::Composite), .. } = s {
            add(&mut d, lhs, if *op == Op::Measured { NodeKind::Latent } else { NodeKind::Composite });
        }
    }
    for s in &statements {
        match s {
            Statement::Relation { lhs, op, rhs } => {
                let modifier = (!rhs.modifiers.is_empty()).then(|| rhs.modifiers.iter().map(show_modifier).collect::<Vec<_>>().join("*"));
                match op {
                    Op::Measured => {
                        add(&mut d, &rhs.var, NodeKind::Observed);
                        d.edges.push(Edge { from: lhs.clone(), to: rhs.var.clone(), kind: EdgeKind::Loading, modifier });
                    }
                    Op::Composite => {
                        add(&mut d, &rhs.var, NodeKind::Observed);
                        d.edges.push(Edge { from: rhs.var.clone(), to: lhs.clone(), kind: EdgeKind::Composite, modifier });
                    }
                    Op::Regression if rhs.var == "1" => d.extras.push(format!("{lhs} ~ {}", show_term(rhs))),
                    Op::Regression => {
                        add(&mut d, lhs, NodeKind::Observed);
                        add(&mut d, &rhs.var, NodeKind::Observed);
                        d.edges.push(Edge { from: rhs.var.clone(), to: lhs.clone(), kind: EdgeKind::Regression, modifier });
                    }
                    Op::Covariance if *lhs == rhs.var => d.extras.push(format!("{lhs} ~~ {}", show_term(rhs))),
                    Op::Covariance => {
                        add(&mut d, lhs, NodeKind::Observed);
                        add(&mut d, &rhs.var, NodeKind::Observed);
                        d.edges.push(Edge { from: lhs.clone(), to: rhs.var.clone(), kind: EdgeKind::Covariance, modifier });
                    }
                }
            }
            Statement::Defined { name, expr } => d.extras.push(format!("{name} := {}", show_expr(expr))),
            Statement::Equal { lhs, rhs } => d.extras.push(format!("{} == {}", show_expr(lhs), show_expr(rhs))),
            Statement::Level(l) => d.extras.push(format!("level: {l}")),
        }
    }
    Ok(d)
}

pub const OBS_W: f64 = 84.0;
pub const OBS_H: f64 = 40.0;
pub const LAT_W: f64 = 104.0;
pub const LAT_H: f64 = 60.0;

fn size(k: NodeKind) -> (f64, f64) {
    match k {
        NodeKind::Observed => (OBS_W, OBS_H),
        _ => (LAT_W, LAT_H),
    }
}

/// Places the nodes that `fixed` doesn't: a confirmatory model as a row of factors with their indicators below; a
/// structural model left to right by the longest chain of paths leading to each variable, with the indicators of the
/// first column to its left, of the last to its right, and of the others below.
pub fn layout(d: &mut Diagram, fixed: &BTreeMap<String, [f64; 2]>) {
    let is_indicator = |d: &Diagram, n: &str| d.edges.iter().any(|e| (e.kind == EdgeKind::Loading && e.to == n) || (e.kind == EdgeKind::Composite && e.from == n));
    let structural: Vec<String> = d.nodes.iter().filter(|n| !is_indicator(d, &n.name)).map(|n| n.name.clone()).collect();
    let flows: Vec<(String, String)> = d.edges.iter().filter(|e| e.kind == EdgeKind::Regression).map(|e| (e.from.clone(), e.to.clone())).collect();
    let mut level: BTreeMap<String, usize> = structural.iter().map(|n| (n.clone(), 0)).collect();
    for _ in 0..structural.len() {
        for (a, b) in &flows {
            if let (Some(&la), Some(&lb)) = (level.get(a), level.get(b)) {
                if lb < la + 1 {
                    level.insert(b.clone(), la + 1);
                }
            }
        }
    }
    let levels = level.values().copied().max().map_or(0, |m| m + 1);
    let indicators_of = |d: &Diagram, n: &str| -> Vec<String> {
        d.edges.iter().filter_map(|e| match e.kind {
            EdgeKind::Loading if e.from == n => Some(e.to.clone()),
            EdgeKind::Composite if e.to == n => Some(e.from.clone()),
            _ => None,
        }).collect()
    };
    let mut pos: BTreeMap<String, [f64; 2]> = BTreeMap::new();
    if levels <= 1 {
        // A row of factors with their indicators beneath, as trees (a higher-order factor above its factors).
        fn place(d: &Diagram, n: &str, left: f64, y: f64, pos: &mut BTreeMap<String, [f64; 2]>, depth: usize) -> f64 {
            let kids: Vec<String> = d
                .edges
                .iter()
                .filter_map(|e| match e.kind {
                    EdgeKind::Loading if e.from == n => Some(e.to.clone()),
                    EdgeKind::Composite if e.to == n => Some(e.from.clone()),
                    _ => None,
                })
                .collect();
            let leaf = OBS_W + 16.0;
            if kids.is_empty() || depth > 6 {
                pos.entry(n.to_string()).or_insert([left + leaf / 2.0, y]);
                return leaf;
            }
            let mut x = left;
            for k in &kids {
                if !pos.contains_key(k) {
                    x += place(d, k, x, y + 160.0, pos, depth + 1);
                }
            }
            let width = (x - left).max(LAT_W + 20.0);
            pos.entry(n.to_string()).or_insert([left + width / 2.0, y]);
            width + 30.0
        }
        let mut x = 40.0;
        for n in &structural {
            x += place(d, n, x, 70.0, &mut pos, 0);
        }
    } else {
        // The first and last columns stack their factors with the indicators on the outer side; factors in middle
        // columns sit above that, with their indicators in a row above them, clear of paths that skip a column.
        let col_x = |l: usize| 260.0 + l as f64 * 300.0;
        let mut y_next: Vec<f64> = (0..levels).map(|l| if l == 0 || l + 1 == levels { 230.0 } else { 150.0 }).collect();
        for n in &structural {
            let l = level[n];
            let ind = indicators_of(d, n);
            let middle = l > 0 && l + 1 < levels;
            if middle {
                let cy = y_next[l];
                pos.insert(n.clone(), [col_x(l), cy]);
                let w = ind.len() as f64 * (OBS_W + 12.0);
                for (k, i) in ind.iter().enumerate() {
                    pos.entry(i.clone()).or_insert([col_x(l) - w / 2.0 + (OBS_W + 12.0) * (k as f64 + 0.5), cy - LAT_H / 2.0 - 75.0]);
                }
                y_next[l] = cy + 300.0;
            } else {
                let block = (ind.len() as f64 * (OBS_H + 12.0)).max(LAT_H + 40.0);
                let y0 = y_next[l];
                pos.insert(n.clone(), [col_x(l), y0 + block / 2.0]);
                let side = if l == 0 { -1.0 } else { 1.0 };
                for (k, i) in ind.iter().enumerate() {
                    pos.entry(i.clone()).or_insert([col_x(l) + side * 170.0, y0 + (OBS_H + 12.0) * (k as f64 + 0.5)]);
                }
                y_next[l] = y0 + block + 50.0;
            }
        }
    }
    // Anything left (an indicator of nothing in the structure) goes along the bottom.
    let bottom = pos.values().map(|p| p[1]).fold(0.0, f64::max) + 90.0;
    let mut x = 60.0;
    for n in &mut d.nodes {
        if let Some(p) = fixed.get(&n.name) {
            n.x = p[0];
            n.y = p[1];
        } else if let Some(p) = pos.get(&n.name) {
            n.x = p[0];
            n.y = p[1];
        } else {
            n.x = x;
            n.y = bottom;
            x += OBS_W + 20.0;
        }
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Where the line from a node's center toward (tx, ty) leaves its outline.
fn boundary(n: &Node, tx: f64, ty: f64) -> (f64, f64) {
    let (dx, dy) = (tx - n.x, ty - n.y);
    let (w, h) = size(n.kind);
    let t = match n.kind {
        NodeKind::Observed => {
            let sx = if dx.abs() > 1e-9 { (w / 2.0) / dx.abs() } else { f64::INFINITY };
            let sy = if dy.abs() > 1e-9 { (h / 2.0) / dy.abs() } else { f64::INFINITY };
            sx.min(sy)
        }
        _ => {
            let (a, b) = (w / 2.0, h / 2.0);
            1.0 / ((dx / a).powi(2) + (dy / b).powi(2)).sqrt()
        }
    };
    let t = t.min(1.0);
    (n.x + dx * t, n.y + dy * t)
}

const FONT: &str = "Arial, Helvetica, 'Liberation Sans', sans-serif";

/// How far a covariance curve bows: a quarter of the distance, plus enough to clear the nodes' own extent in the
/// direction it bows (so two stacked boxes get a visible curve beside them).
fn bow(a: &Node, b: &Node, px: f64, py: f64, len: f64) -> f64 {
    let ext = |n: &Node| {
        let (w, h) = size(n.kind);
        px.abs() * w / 2.0 + py.abs() * h / 2.0
    };
    0.25 * len + 25.0 + ext(a).max(ext(b))
}

/// The diagram as an APA figure. `labels` maps a parameter name (as [`Edge::param`]) to the text shown on its arrow.
pub fn svg(d: &Diagram, labels: &BTreeMap<String, String>, alt: &str) -> String {
    let pad = 30.0;
    let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for n in &d.nodes {
        let (w, h) = size(n.kind);
        x0 = x0.min(n.x - w / 2.0);
        x1 = x1.max(n.x + w / 2.0);
        y0 = y0.min(n.y - h / 2.0);
        y1 = y1.max(n.y + h / 2.0);
    }
    if !x0.is_finite() {
        (x0, y0, x1, y1) = (0.0, 0.0, 100.0, 100.0);
    }
    // Covariance curves bow outward: take in their apexes (and room for their labels).
    let (nx0, ny0, nx1, ny1) = (x0, y0, x1, y1);
    for e in d.edges.iter().filter(|e| e.kind == EdgeKind::Covariance) {
        let (Some(a), Some(b)) = (d.nodes.iter().find(|n| n.name == e.from), d.nodes.iter().find(|n| n.name == e.to)) else { continue };
        let (mx, my) = ((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len = (dx * dx + dy * dy).sqrt().max(1e-9);
        let (px, py) = (-dy / len, dx / len);
        let bow = bow(a, b, px, py, len);
        let side = if (px * (mx - (nx0 + nx1) / 2.0) + py * (my - (ny0 + ny1) / 2.0)) >= 0.0 { 1.0 } else { -1.0 };
        let (ax, ay) = (mx + side * px * bow * 0.5, my + side * py * bow * 0.5);
        x0 = x0.min(ax - 20.0);
        x1 = x1.max(ax + 20.0);
        y0 = y0.min(ay - 14.0);
        y1 = y1.max(ay + 14.0);
    }
    let (vx, vy, vw, vh) = (x0 - pad - 30.0, y0 - pad - 30.0, x1 - x0 + 2.0 * pad + 60.0, y1 - y0 + 2.0 * pad + 60.0);
    let mut s = String::new();
    let _ = write!(
        s,
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vx:.1} {vy:.1} {vw:.1} {vh:.1}" width="{vw:.0}" height="{vh:.0}" role="img" aria-label="{}" font-family="{FONT}" font-size="13"><defs><marker id="arrow" viewBox="0 0 10 10" refX="10" refY="5" markerWidth="8" markerHeight="8" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" fill="#000"/></marker></defs><rect x="{vx:.1}" y="{vy:.1}" width="{vw:.1}" height="{vh:.1}" fill="#fff"/>"##,
        esc(alt)
    );
    let node = |name: &str| d.nodes.iter().find(|n| n.name == name);
    for e in &d.edges {
        let (Some(a), Some(b)) = (node(&e.from), node(&e.to)) else { continue };
        let key = e.param();
        let label = labels.get(&key).cloned().unwrap_or_default();
        let _ = write!(s, r#"<g data-param="{}">"#, esc(&key));
        let (lx, ly);
        if e.kind == EdgeKind::Covariance {
            // A curve bowed away from the diagram's middle, arrowheads at both ends.
            let (mx, my) = ((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len = (dx * dx + dy * dy).sqrt().max(1e-9);
            let (nx, ny) = (-dy / len, dx / len);
            let bow = bow(a, b, nx, ny, len);
            let side = if (nx * (mx - (nx0 + nx1) / 2.0) + ny * (my - (ny0 + ny1) / 2.0)) >= 0.0 { 1.0 } else { -1.0 };
            let (cx, cy) = (mx + side * nx * bow, my + side * ny * bow);
            let (sx, sy) = boundary(a, cx, cy);
            let (ex, ey) = boundary(b, cx, cy);
            let _ = write!(s, r##"<path d="M{sx:.1},{sy:.1} Q{cx:.1},{cy:.1} {ex:.1},{ey:.1}" fill="none" stroke="#000" stroke-width="1.2" marker-start="url(#arrow)" marker-end="url(#arrow)"/>"##);
            (lx, ly) = (0.25 * sx + 0.5 * cx + 0.25 * ex, 0.25 * sy + 0.5 * cy + 0.25 * ey);
        } else {
            let (sx, sy) = boundary(a, b.x, b.y);
            let (ex, ey) = boundary(b, a.x, a.y);
            let _ = write!(s, r##"<line x1="{sx:.1}" y1="{sy:.1}" x2="{ex:.1}" y2="{ey:.1}" stroke="#000" stroke-width="1.2" marker-end="url(#arrow)"/>"##);
            (lx, ly) = (sx + 0.5 * (ex - sx), sy + 0.5 * (ey - sy));
        }
        if !label.is_empty() {
            let w = 8.0 * label.chars().count() as f64 + 8.0;
            let _ = write!(s, r##"<rect x="{:.1}" y="{:.1}" width="{w:.1}" height="18" fill="#fff"/><text x="{lx:.1}" y="{:.1}" text-anchor="middle">{}</text>"##, lx - w / 2.0, ly - 10.0, ly + 4.0, esc(&label));
        }
        s.push_str("</g>");
    }
    for n in &d.nodes {
        let (w, h) = size(n.kind);
        match n.kind {
            NodeKind::Observed => {
                let _ = write!(s, r##"<rect x="{:.1}" y="{:.1}" width="{w}" height="{h}" fill="#fff" stroke="#000" stroke-width="1.2"/>"##, n.x - w / 2.0, n.y - h / 2.0);
            }
            NodeKind::Latent => {
                let _ = write!(s, r##"<ellipse cx="{:.1}" cy="{:.1}" rx="{}" ry="{}" fill="#fff" stroke="#000" stroke-width="1.2"/>"##, n.x, n.y, w / 2.0, h / 2.0);
            }
            NodeKind::Composite => {
                // A hexagon marks a composite (formed by its indicators).
                let (a, b) = (w / 2.0, h / 2.0);
                let pts = [(n.x - a, n.y), (n.x - a / 2.0, n.y - b), (n.x + a / 2.0, n.y - b), (n.x + a, n.y), (n.x + a / 2.0, n.y + b), (n.x - a / 2.0, n.y + b)];
                let p: Vec<String> = pts.iter().map(|(x, y)| format!("{x:.1},{y:.1}")).collect();
                let _ = write!(s, r##"<polygon points="{}" fill="#fff" stroke="#000" stroke-width="1.2"/>"##, p.join(" "));
            }
        }
        let _ = write!(s, r#"<text x="{:.1}" y="{:.1}" text-anchor="middle">{}</text>"#, n.x, n.y + 4.5, esc(&n.name));
    }
    s.push_str("</svg>");
    s
}

/// Alternative text: what the diagram shows, in sentences.
pub fn alt_text(d: &Diagram, labels: &BTreeMap<String, String>, what: &str) -> String {
    let lab = |e: &Edge| labels.get(&e.param()).map(|l| format!(" ({l})")).unwrap_or_default();
    let mut parts = vec![format!("Path diagram of the {what}.")];
    for n in d.nodes.iter().filter(|n| n.kind != NodeKind::Observed) {
        let ind: Vec<String> = d
            .edges
            .iter()
            .filter(|e| (e.kind == EdgeKind::Loading && e.from == n.name) || (e.kind == EdgeKind::Composite && e.to == n.name))
            .map(|e| format!("{}{}", if e.kind == EdgeKind::Loading { &e.to } else { &e.from }, lab(e)))
            .collect();
        if !ind.is_empty() {
            parts.push(format!("{} is {} {}.", n.name, if n.kind == NodeKind::Latent { "a latent variable measured by" } else { "a composite formed from" }, ind.join(", ")));
        }
    }
    let reg: Vec<String> = d.edges.iter().filter(|e| e.kind == EdgeKind::Regression).map(|e| format!("{} to {}{}", e.from, e.to, lab(e))).collect();
    if !reg.is_empty() {
        parts.push(format!("Paths: {}.", reg.join("; ")));
    }
    let cov: Vec<String> = d.edges.iter().filter(|e| e.kind == EdgeKind::Covariance).map(|e| format!("{} with {}{}", e.from, e.to, lab(e))).collect();
    if !cov.is_empty() {
        parts.push(format!("Covariances: {}.", cov.join("; ")));
    }
    if !labels.is_empty() {
        parts.push("Values are standardized estimates.".into());
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_and_layout() {
        let mut d = structure("f =~ a*x1 + x2 + x3\ng <~ x4 + x5\ny ~ f + g\nx1 ~~ x2\nab := a*2\ny ~ 1").unwrap();
        assert_eq!(d.nodes.iter().filter(|n| n.kind == NodeKind::Latent).count(), 1);
        assert_eq!(d.nodes.iter().find(|n| n.name == "g").unwrap().kind, NodeKind::Composite);
        assert_eq!(d.edges.len(), 3 + 2 + 2 + 1);
        assert_eq!(d.edges[0].modifier.as_deref(), Some("a"));
        assert_eq!(d.extras, vec!["ab := a*2".to_string(), "y ~ 1".to_string()]);
        let mut fixed = BTreeMap::new();
        fixed.insert("y".to_string(), [999.0, 50.0]);
        layout(&mut d, &fixed);
        let y = d.nodes.iter().find(|n| n.name == "y").unwrap();
        assert_eq!((y.x, y.y), (999.0, 50.0));
        let f = d.nodes.iter().find(|n| n.name == "f").unwrap();
        let x1 = d.nodes.iter().find(|n| n.name == "x1").unwrap();
        assert!(x1.x < f.x, "indicators of the first column sit to its left");
        let svg = svg(&d, &BTreeMap::from([("f =~ x1".to_string(), ".77".to_string())]), "alt");
        assert!(svg.contains(r#"data-param="f =~ x1""#) && svg.contains(".77") && svg.contains("<ellipse") && svg.contains("<polygon"));
    }
}
