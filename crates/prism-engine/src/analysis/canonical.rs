//! Canonical correlation (`corr.canonical`): the strongest correlations between weighted sums of one set of variables
//! and weighted sums of another.
//!
//! Defaults (05 §1): listwise deletion across both sets; canonical correlations from the correlation matrix (so the
//! weights are standardized coefficients); the dimension-reduction tests with Wilks' lambda and Rao's F approximation
//! (the test of function i asks whether functions i to the last are all zero); structure coefficients (each variable's
//! correlation with its own set's canonical variate). The sign of a function is arbitrary; PrismStatistics makes the
//! structure coefficients of the first set sum to a positive number.

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

pub const METHOD_ID: &str = "corr.canonical";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub set1: Vec<String>,
    pub set2: Vec<String>,
}

type Matrix = Vec<Vec<f64>>;

/// The lower Cholesky factor of a symmetric positive definite matrix, or None when it is not positive definite.
fn cholesky(a: &Matrix) -> Option<Matrix> {
    let n = a.len();
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let s: f64 = (0..j).map(|k| l[i][k] * l[j][k]).sum();
            if i == j {
                let d = a[i][i] - s;
                if d <= 1e-10 {
                    return None;
                }
                l[i][j] = d.sqrt();
            } else {
                l[i][j] = (a[i][j] - s) / l[j][j];
            }
        }
    }
    Some(l)
}

/// Solves L x = b for lower-triangular L.
fn forward(l: &Matrix, b: &[f64]) -> Vec<f64> {
    let mut x = vec![0.0; b.len()];
    for i in 0..b.len() {
        x[i] = (b[i] - (0..i).map(|k| l[i][k] * x[k]).sum::<f64>()) / l[i][i];
    }
    x
}

/// Solves Lᵀ x = b for lower-triangular L.
fn backward(l: &Matrix, b: &[f64]) -> Vec<f64> {
    let n = b.len();
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        x[i] = (b[i] - ((i + 1)..n).map(|k| l[k][i] * x[k]).sum::<f64>()) / l[i][i];
    }
    x
}

/// Eigenvalues (largest first) and eigenvectors (as columns, in the same order) of a symmetric matrix, by Jacobi
/// rotations.
fn eigen_symmetric(a: &Matrix) -> (Vec<f64>, Matrix) {
    let n = a.len();
    let mut m = a.clone();
    let mut v: Matrix = (0..n).map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect();
    for _ in 0..100 {
        let off: f64 = (0..n).flat_map(|i| (0..n).filter(move |&j| j != i).map(move |j| (i, j))).map(|(i, j)| m[i][j] * m[i][j]).sum();
        if off < 1e-30 {
            break;
        }
        for p in 0..n {
            for q in (p + 1)..n {
                if m[p][q].abs() < 1e-300 {
                    continue;
                }
                let theta = (m[q][q] - m[p][p]) / (2.0 * m[p][q]);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let t = if theta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for row in m.iter_mut() {
                    let (mkp, mkq) = (row[p], row[q]);
                    row[p] = c * mkp - s * mkq;
                    row[q] = s * mkp + c * mkq;
                }
                let (rp, rq) = (m[p].clone(), m[q].clone());
                for (k, (mpk, mqk)) in rp.iter().zip(&rq).enumerate() {
                    m[p][k] = c * mpk - s * mqk;
                    m[q][k] = s * mpk + c * mqk;
                }
                for row in v.iter_mut() {
                    let (vkp, vkq) = (row[p], row[q]);
                    row[p] = c * vkp - s * vkq;
                    row[q] = s * vkp + c * vkq;
                }
            }
        }
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&i, &j| m[j][j].total_cmp(&m[i][i]));
    let values = order.iter().map(|&i| m[i][i]).collect();
    let vectors = (0..n).map(|r| order.iter().map(|&c| v[r][c]).collect()).collect();
    (values, vectors)
}

/// One canonical function.
pub struct Function {
    pub r: f64,
    /// Standardized coefficients of the first and second set.
    pub a: Vec<f64>,
    pub b: Vec<f64>,
    /// Structure coefficients: each variable's correlation with its own set's variate.
    pub sa: Vec<f64>,
    pub sb: Vec<f64>,
}

/// A dimension-reduction test: are functions `from` to the last all zero?
pub struct WilksTest {
    pub lambda: f64,
    pub f: f64,
    pub df1: f64,
    pub df2: f64,
    pub p: f64,
}

/// The canonical functions of two sets given their correlation matrix (set 1 first, `p` variables), or None when a
/// set's variables are collinear.
pub fn canonical(r: &Matrix, p: usize) -> Option<Vec<Function>> {
    let q = r.len() - p;
    let sub = |rows: std::ops::Range<usize>, cols: std::ops::Range<usize>| -> Matrix { rows.map(|i| cols.clone().map(|j| r[i][j]).collect()).collect() };
    let (rxx, ryy, rxy) = (sub(0..p, 0..p), sub(p..p + q, p..p + q), sub(0..p, p..p + q));
    let (lx, ly) = (cholesky(&rxx)?, cholesky(&ryy)?);
    // M = Lx⁻¹ Rxy Ly⁻ᵀ, column by column of Ly⁻ᵀ applied on the right: rows of M are Ly⁻¹ (row of Lx⁻¹ Rxy).
    let left: Matrix = (0..q).map(|j| forward(&lx, &(0..p).map(|i| rxy[i][j]).collect::<Vec<_>>())).collect(); // q × p (columns of Lx⁻¹ Rxy)
    let m: Matrix = (0..p).map(|i| forward(&ly, &(0..q).map(|j| left[j][i]).collect::<Vec<_>>())).collect(); // p × q
    let mmt: Matrix = (0..p).map(|i| (0..p).map(|k| (0..q).map(|j| m[i][j] * m[k][j]).sum()).collect()).collect();
    let (vals, vecs) = eigen_symmetric(&mmt);
    let s = p.min(q);
    let mut out = Vec::new();
    for f in 0..s {
        let rho = vals[f].max(0.0).sqrt().min(1.0);
        let u: Vec<f64> = (0..p).map(|i| vecs[i][f]).collect();
        let v: Vec<f64> = if rho > 1e-12 { (0..q).map(|j| (0..p).map(|i| m[i][j] * u[i]).sum::<f64>() / rho).collect() } else { vec![0.0; q] };
        let mut a = backward(&lx, &u);
        let mut b = backward(&ly, &v);
        let mut sa: Vec<f64> = (0..p).map(|i| (0..p).map(|k| rxx[i][k] * a[k]).sum()).collect();
        let mut sb: Vec<f64> = (0..q).map(|i| (0..q).map(|k| ryy[i][k] * b[k]).sum()).collect();
        if sa.iter().sum::<f64>() < 0.0 {
            for x in a.iter_mut().chain(b.iter_mut()).chain(sa.iter_mut()).chain(sb.iter_mut()) {
                *x = -*x;
            }
        }
        out.push(Function { r: rho, a, b, sa, sb });
    }
    Some(out)
}

/// Wilks' lambda for functions `from` onward, with Rao's F approximation.
pub fn wilks(rs: &[f64], from: usize, p: usize, q: usize, n: usize) -> WilksTest {
    let lambda: f64 = rs[from..].iter().map(|r| 1.0 - r * r).product();
    let (pk, qk) = ((p - from) as f64, (q - from) as f64);
    let df1 = pk * qk;
    let w = n as f64 - 1.5 - (p + q) as f64 / 2.0;
    let t = if pk * pk + qk * qk - 5.0 > 0.0 { ((pk * pk * qk * qk - 4.0) / (pk * pk + qk * qk - 5.0)).sqrt() } else { 1.0 };
    let df2 = w * t - pk * qk / 2.0 + 1.0;
    let l = lambda.powf(1.0 / t);
    let f = (1.0 - l) / l * df2 / df1;
    WilksTest { lambda, f, df1, df2, p: prismqda_stats::dist::f_sf(f, df1, df2) }
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    if spec.set1.is_empty() || spec.set2.is_empty() {
        return Err(Error::Invalid("Put at least one variable in each set.".into()));
    }
    if spec.set1.len() + spec.set2.len() < 3 {
        return Err(Error::Invalid("With one variable in each set this is an ordinary correlation; use Bivariate Correlations.".into()));
    }
    let all: Vec<&String> = spec.set1.iter().chain(&spec.set2).collect();
    for (i, a) in all.iter().enumerate() {
        if all[..i].iter().any(|b| ds.find(a) == ds.find(b)) {
            return Err(Error::Invalid(format!("{a} is used more than once.")));
        }
    }
    let labels: Vec<String> = all.iter().map(|n| ds.variable(n).map(|v| v.display_name().to_string())).collect::<Result<_>>()?;
    let cells: Vec<Vec<Slot<f64>>> = all.iter().map(|n| numeric(ds, n)).collect::<Result<_>>()?;
    let total = cells[0].len();
    let mut tally = Tally::default();
    let used: Vec<usize> = (0..total).filter(|&r| !cells.iter().zip(&all).any(|(c, n)| tally.check(&c[r], n))).collect();
    let cols: Vec<Vec<f64>> = cells.iter().map(|c| used.iter().map(|&r| c[r].value().expect("checked")).collect()).collect();
    let (p, q, n) = (spec.set1.len(), spec.set2.len(), used.len());
    let (l1, l2) = (&labels[..p], &labels[p..]);
    let note = format!(
        "Canonical correlation between the first set ({}) and the second set ({}), from the correlation matrix, so the weights are standardized canonical coefficients. Each dimension-reduction test asks whether that function and all later ones are zero, with Wilks' lambda and Rao's F approximation. Structure coefficients are each variable's correlation with its own set's canonical variate; a function's sign is arbitrary, and is chosen so the first set's structure coefficients sum to a positive number. Cases were used when every variable had a valid value (listwise). Computed by PrismStatistics with prismqda-stats {}.",
        apa::series(l1),
        apa::series(l2),
        super::STATS_VERSION
    );
    let mut b = common::bundle(METHOD_ID, METHOD_VERSION, format!("Canonical Correlation: {} with {}", l1.join(", "), l2.join(", ")), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, n), note);
    set(&mut b, "n", n as f64);
    set(&mut b, "p", p as f64);
    set(&mut b, "q", q as f64);
    set_text(&mut b, "set1", l1.join("\u{1f}"));
    set_text(&mut b, "set2", l2.join("\u{1f}"));
    if n < p + q + 2 {
        fail(&mut b, format!("There are {n} cases with every value for {} variables; canonical correlation needs more cases than variables (and many more to be stable).", p + q));
        return Ok(b);
    }
    let k = p + q;
    let mut r: Matrix = vec![vec![1.0; k]; k];
    for i in 0..k {
        for j in (i + 1)..k {
            let Some(c) = prismqda_stats::correlation::pearson(&cols[i], &cols[j], 0.95) else {
                fail(&mut b, format!("{} or {} has no variation among the cases used.", labels[i], labels[j]));
                return Ok(b);
            };
            r[i][j] = c.r;
            r[j][i] = c.r;
        }
    }
    let Some(funcs) = canonical(&r, p) else {
        fail(&mut b, "A variable in one set is (almost) exactly predicted by the others in the same set, so the canonical weights can't be computed. Remove one of the overlapping variables.");
        return Ok(b);
    };
    let s = funcs.len();
    set(&mut b, "functions", s as f64);
    let rs: Vec<f64> = funcs.iter().map(|f| f.r).collect();
    let mut rows = Vec::new();
    for i in 0..s {
        let w = wilks(&rs, i, p, q, n);
        let key = format!("fn.{}", i + 1);
        for (kk, v) in [("r", rs[i]), ("r2", rs[i] * rs[i]), ("lambda", w.lambda), ("F", w.f), ("df1", w.df1), ("df2", w.df2), ("p", w.p)] {
            set(&mut b, format!("{key}.{kk}"), v);
        }
        rows.push(vec![text(format!("{} to {s}", i + 1)), num(rs[i], 3), num(rs[i] * rs[i], 3), num(w.lambda, 3), num(w.f, 3), num(w.df1, 0), num(w.df2, 2), common::p(w.p)]);
    }
    b.tables.push(Table {
        id: "functions".into(),
        title: "Canonical correlations and dimension-reduction tests".into(),
        columns: vec!["Functions tested".into(), "Canonical r".into(), "r²".into(), "Wilks' λ".into(), "F".into(), "df1".into(), "df2".into(), "p".into()],
        rows,
        footnotes: vec!["The canonical r in each row is that row's first function; the test covers it and every later function.".into()],
    });
    let head = |name: &str| { let mut c = vec![name.to_string()]; c.extend((1..=s).map(|i| format!("Function {i}"))); c };
    let coef_rows = |labels: &[String], get: &dyn Fn(&Function) -> &Vec<f64>| labels.iter().enumerate().map(|(i, l)| { let mut row = vec![text(l)]; row.extend(funcs.iter().map(|f| num(get(f)[i], 3))); row }).collect::<Vec<_>>();
    b.tables.push(Table { id: "coef1".into(), title: "Standardized canonical coefficients: first set".into(), columns: head("Variable"), rows: coef_rows(l1, &|f| &f.a), footnotes: Vec::new() });
    b.tables.push(Table { id: "coef2".into(), title: "Standardized canonical coefficients: second set".into(), columns: head("Variable"), rows: coef_rows(l2, &|f| &f.b), footnotes: Vec::new() });
    let mut srows = coef_rows(l1, &|f| &f.sa);
    srows.extend(coef_rows(l2, &|f| &f.sb));
    b.tables.push(Table { id: "structure".into(), title: "Structure coefficients".into(), columns: head("Variable"), rows: srows, footnotes: vec![format!("The first {p} rows are correlations with the first set's variate, the rest with the second set's.")] });
    for (fi, f) in funcs.iter().enumerate() {
        for (i, v) in f.a.iter().enumerate() {
            set(&mut b, format!("fn.{}.a.{}", fi + 1, i + 1), *v);
        }
        for (i, v) in f.sa.iter().enumerate() {
            set(&mut b, format!("fn.{}.sa.{}", fi + 1, i + 1), *v);
        }
        for (i, v) in f.b.iter().enumerate() {
            set(&mut b, format!("fn.{}.b.{}", fi + 1, i + 1), *v);
        }
        for (i, v) in f.sb.iter().enumerate() {
            set(&mut b, format!("fn.{}.sb.{}", fi + 1, i + 1), *v);
        }
    }
    let per = n as f64 / k as f64;
    b.diagnostics.push(if per < 10.0 {
        Diagnostic { id: "size".into(), title: "Cases per variable".into(), state: DiagnosticState::ReviewNeeded, message: format!("{n} cases for {k} variables ({} per variable). With fewer than 10 per variable, canonical weights are unstable and the correlations overstate the relationship.", apa::num(per, 1)) }
    } else {
        Diagnostic { id: "size".into(), title: "Cases per variable".into(), state: DiagnosticState::NoConcernDetected, message: format!("{n} cases for {k} variables.") }
    });
    b.diagnostics.push(not_assessed("normality", "Multivariate normality", "The F tests assume the variables are jointly normal; look at each variable's distribution and at scatterplots for outliers."));
    Ok(b)
}
