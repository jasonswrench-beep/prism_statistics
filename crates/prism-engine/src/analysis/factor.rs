//! Principal components and exploratory factor analysis (`factor.efa`, checklist B3a–c), of the correlation matrix of
//! numeric variables (listwise).
//!
//! * Extraction: principal components; maximum likelihood (the objective and gradient of R's `factanal`, uniquenesses
//!   bounded below by 0.005); principal axis factoring, iterated from squared multiple correlations; and minimum residual
//!   (unweighted least squares, which iterated principal axis converges to).
//! * Rotation: varimax (Kaiser-normalized, as R's `varimax`), oblimin (quartimin, gamma 0, as `GPArotation`), and promax
//!   (power 4, as `psych`), by Bernaards and Jennrich's gradient projection algorithm.
//! * Adequacy: the Kaiser-Meyer-Olkin measure (overall and per variable) and Bartlett's test of sphericity.
//! * How many: Horn's parallel analysis (the 95th percentile of eigenvalues of seeded random normal data of the same
//!   size) and a scree plot.
//!
//! Factors are ordered by the variance they account for after rotation and signed so each has a positive sum of
//! loadings.

use prismqda_stats::dist::chisq_sf;
use prismqda_stats::rng::SplitMix64;
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use super::linalg::{det, identity, invert, mul, symmetric_eigen, transpose, Mat};
use crate::apa;
use crate::chart::{self, Chart};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Cell, Diagnostic, DiagnosticState, ResultBundle, Table};

pub const METHOD_ID: &str = "factor.efa";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Extraction {
    Pca,
    #[default]
    Ml,
    Paf,
    Minres,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Rotation {
    None,
    Varimax,
    #[default]
    Oblimin,
    Promax,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub variables: Vec<String>,
    #[serde(default)]
    pub extraction: Extraction,
    #[serde(default)]
    pub rotation: Rotation,
    /// How many factors; parallel analysis decides when left out.
    #[serde(default)]
    pub factors: Option<usize>,
    #[serde(default = "yes")]
    pub parallel: bool,
    #[serde(default = "yes")]
    pub scree: bool,
    /// Hide loadings smaller than this in the loadings table (they are still computed and stored).
    #[serde(default = "default_hide")]
    pub hide_below: f64,
    #[serde(default = "default_seed")]
    pub seed: u64,
}

fn yes() -> bool {
    true
}

fn default_hide() -> f64 {
    0.0
}

fn default_seed() -> u64 {
    20_260_101
}

/// The correlation matrix of complete rows.
pub fn correlations(cols: &[Vec<f64>]) -> Mat {
    let p = cols.len();
    let n = cols[0].len() as f64;
    let means: Vec<f64> = cols.iter().map(|c| c.iter().sum::<f64>() / n).collect();
    let sds: Vec<f64> = cols.iter().zip(&means).map(|(c, m)| (c.iter().map(|v| (v - m).powi(2)).sum::<f64>()).sqrt()).collect();
    (0..p)
        .map(|i| (0..p).map(|j| if i == j { 1.0 } else { cols[i].iter().zip(&cols[j]).map(|(a, b)| (a - means[i]) * (b - means[j])).sum::<f64>() / (sds[i] * sds[j]) }).collect())
        .collect()
}

/// Loadings of the first `m` principal axes of a (reduced) correlation matrix.
fn principal_axes(r: &Mat, m: usize) -> (Mat, Vec<f64>) {
    let (vals, vecs) = symmetric_eigen(r);
    let l = (0..r.len()).map(|i| (0..m).map(|k| vecs[i][k] * vals[k].max(0.0).sqrt()).collect()).collect();
    (l, vals)
}

/// Iterated principal axis factoring from squared multiple correlations, to convergence (the ULS / minimum residual
/// solution at its fixed point). Returns the loadings and whether communalities passed 1.
fn paf(r: &Mat, m: usize, rinv: &Mat) -> (Mat, bool) {
    let p = r.len();
    let mut h: Vec<f64> = (0..p).map(|i| 1.0 - 1.0 / rinv[i][i]).collect();
    let mut l = Vec::new();
    for _ in 0..5000 {
        let mut rr = r.clone();
        for (i, hi) in h.iter().enumerate() {
            rr[i][i] = *hi;
        }
        l = principal_axes(&rr, m).0;
        let new: Vec<f64> = l.iter().map(|row: &Vec<f64>| row.iter().map(|v| v * v).sum()).collect();
        let change: f64 = new.iter().zip(&h).map(|(a, b)| (a - b).abs()).sum();
        h = new;
        if change < 1e-12 {
            break;
        }
    }
    let heywood = h.iter().any(|v| *v >= 1.0);
    (l, heywood)
}

/// R's `factanal` objective (to minimize) and gradient in the uniquenesses.
fn ml_objective(r: &Mat, psi: &[f64], m: usize) -> (f64, Vec<f64>, Mat) {
    let p = r.len();
    let sc: Vec<f64> = psi.iter().map(|v| 1.0 / v.sqrt()).collect();
    let s: Mat = (0..p).map(|i| (0..p).map(|j| sc[i] * r[i][j] * sc[j]).collect()).collect();
    let (vals, vecs) = symmetric_eigen(&s);
    let f = -(vals[m..].iter().map(|e| e.ln() - e).sum::<f64>() - m as f64 + p as f64);
    let load: Mat = (0..p).map(|i| (0..m).map(|k| psi[i].sqrt() * vecs[i][k] * (vals[k] - 1.0).max(0.0).sqrt()).collect()).collect();
    let grad: Vec<f64> = (0..p)
        .map(|i| {
            let ll: f64 = load[i].iter().map(|v| v * v).sum();
            (ll + psi[i] - r[i][i]) / (psi[i] * psi[i])
        })
        .collect();
    (f, grad, load)
}

/// Maximum likelihood factor analysis: minimizes the `factanal` objective over uniquenesses in [0.005, 1] by projected
/// gradient descent with backtracking. Returns the loadings, uniquenesses and the objective.
pub(crate) fn ml(r: &Mat, m: usize, rinv: &Mat) -> (Mat, Vec<f64>, f64) {
    let p = r.len();
    let (lo, hi) = (0.005, 1.0);
    let mut psi: Vec<f64> = (0..p).map(|i| ((1.0 - 0.5 * m as f64 / p as f64) / rinv[i][i]).clamp(lo, hi)).collect();
    let (mut f, mut g, _) = ml_objective(r, &psi, m);
    let mut step = 0.1;
    for _ in 0..20000 {
        // The projected gradient: zero where a bound blocks the move.
        let pg: Vec<f64> = psi.iter().zip(&g).map(|(x, gi)| if (*x <= lo && *gi > 0.0) || (*x >= hi && *gi < 0.0) { 0.0 } else { *gi }).collect();
        let norm = pg.iter().map(|v| v * v).sum::<f64>().sqrt();
        if norm < 1e-11 {
            break;
        }
        let mut moved = false;
        let mut t = step * 2.0;
        for _ in 0..60 {
            let cand: Vec<f64> = psi.iter().zip(&pg).map(|(x, gi)| (x - t * gi).clamp(lo, hi)).collect();
            let (fc, gc, _) = ml_objective(r, &cand, m);
            let decrease: f64 = psi.iter().zip(&cand).zip(&g).map(|((x, c), gi)| gi * (x - c)).sum();
            if fc <= f - 1e-4 * decrease {
                psi = cand;
                f = fc;
                g = gc;
                step = t;
                moved = true;
                break;
            }
            t /= 2.0;
        }
        if !moved {
            break;
        }
    }
    let (f, _, load) = ml_objective(r, &psi, m);
    (load, psi, f)
}

/// The varimax criterion value and gradient (GPArotation's `vgQ.varimax`).
fn varimax_crit(l: &Mat) -> (f64, Mat) {
    let p = l.len();
    let m = l[0].len();
    let means: Vec<f64> = (0..m).map(|k| l.iter().map(|r| r[k] * r[k]).sum::<f64>() / p as f64).collect();
    let ql: Mat = l.iter().map(|r| r.iter().zip(&means).map(|(v, mu)| v * v - mu).collect()).collect();
    let f = -ql.iter().flatten().map(|v| v * v).sum::<f64>() / 4.0;
    let g = l.iter().zip(&ql).map(|(r, q)| r.iter().zip(q).map(|(v, qv)| -v * qv).collect()).collect();
    (f, g)
}

/// The quartimin (oblimin, gamma 0) criterion and gradient (GPArotation's `vgQ.oblimin`).
fn oblimin_crit(l: &Mat) -> (f64, Mat) {
    let m = l[0].len();
    let x: Mat = l.iter().map(|r| (0..m).map(|k| (0..m).filter(|&j| j != k).map(|j| r[j] * r[j]).sum()).collect()).collect();
    let f = l.iter().zip(&x).map(|(r, xr)| r.iter().zip(xr).map(|(v, xv)| v * v * xv).sum::<f64>()).sum::<f64>() / 4.0;
    let g = l.iter().zip(&x).map(|(r, xr)| r.iter().zip(xr).map(|(v, xv)| v * xv).collect()).collect();
    (f, g)
}

fn frob(a: &Mat) -> f64 {
    a.iter().flatten().map(|v| v * v).sum::<f64>().sqrt()
}

fn sub_scaled(a: &Mat, b: &Mat, s: f64) -> Mat {
    a.iter().zip(b).map(|(x, y)| x.iter().zip(y).map(|(u, v)| u - s * v).collect()).collect()
}

/// The orthogonal matrix nearest to `x` (the polar factor X (XᵀX)^(-1/2)).
fn nearest_orthogonal(x: &Mat) -> Mat {
    let (vals, vecs) = symmetric_eigen(&mul(&transpose(x), x));
    let n = vals.len();
    let inv_sqrt: Mat = (0..n).map(|i| (0..n).map(|j| (0..n).map(|k| vecs[i][k] * vecs[j][k] / vals[k].max(1e-300).sqrt()).sum()).collect()).collect();
    mul(x, &inv_sqrt)
}

/// Gradient projection rotation, orthogonal (GPArotation's `GPForth`). Returns the rotated loadings and the rotation.
fn gp_orth(a: &Mat, crit: fn(&Mat) -> (f64, Mat)) -> (Mat, Mat) {
    let m = a[0].len();
    let mut t = identity(m);
    let mut l = mul(a, &t);
    let (mut f, gq) = crit(&l);
    let mut g = mul(&transpose(a), &gq);
    let mut al = 1.0;
    for _ in 0..5000 {
        let mm = mul(&transpose(&t), &g);
        let s: Mat = (0..m).map(|i| (0..m).map(|j| (mm[i][j] + mm[j][i]) / 2.0).collect()).collect();
        let gp = sub_scaled(&g, &mul(&t, &s), 1.0);
        let sn = frob(&gp);
        if sn < 1e-12 {
            break;
        }
        al *= 2.0;
        let mut accepted = None;
        for _ in 0..30 {
            let tt = nearest_orthogonal(&sub_scaled(&t, &gp, al));
            let lt = mul(a, &tt);
            let (ft, gqt) = crit(&lt);
            if ft < f - 0.5 * sn * sn * al {
                accepted = Some((tt, lt, ft, gqt));
                break;
            }
            al /= 2.0;
        }
        let Some((tt, lt, ft, gqt)) = accepted else { break };
        t = tt;
        l = lt;
        f = ft;
        g = mul(&transpose(a), &gqt);
    }
    (l, t)
}

/// Gradient projection rotation, oblique (GPArotation's `GPFoblq`). Returns the pattern loadings and Phi.
fn gp_oblique(a: &Mat, crit: fn(&Mat) -> (f64, Mat)) -> Option<(Mat, Mat)> {
    let m = a[0].len();
    let mut t = identity(m);
    let mut l = mul(a, &invert(&transpose(&t))?);
    let (mut f, gq) = crit(&l);
    let grad = |l: &Mat, gq: &Mat, t: &Mat| -> Option<Mat> { Some(transpose(&mul(&mul(&transpose(l), gq), &invert(t)?)).iter().map(|r| r.iter().map(|v| -v).collect()).collect()) };
    let mut g = grad(&l, &gq, &t)?;
    let mut al = 1.0;
    for _ in 0..5000 {
        let cs: Vec<f64> = (0..m).map(|k| (0..m).map(|i| t[i][k] * g[i][k]).sum()).collect();
        let gp: Mat = (0..m).map(|i| (0..m).map(|k| g[i][k] - t[i][k] * cs[k]).collect()).collect();
        let sn = frob(&gp);
        if sn < 1e-12 {
            break;
        }
        al *= 2.0;
        let mut accepted = None;
        for _ in 0..30 {
            let x = sub_scaled(&t, &gp, al);
            let v: Vec<f64> = (0..m).map(|k| 1.0 / (0..m).map(|i| x[i][k] * x[i][k]).sum::<f64>().sqrt()).collect();
            let tt: Mat = x.iter().map(|r| r.iter().zip(&v).map(|(a, b)| a * b).collect()).collect();
            let Some(inv) = invert(&transpose(&tt)) else { al /= 2.0; continue };
            let lt = mul(a, &inv);
            let (ft, gqt) = crit(&lt);
            if ft < f - 0.5 * sn * sn * al {
                accepted = Some((tt, lt, ft, gqt));
                break;
            }
            al /= 2.0;
        }
        let Some((tt, lt, ft, gqt)) = accepted else { break };
        g = grad(&lt, &gqt, &tt)?;
        t = tt;
        l = lt;
        f = ft;
    }
    Some((l, mul(&transpose(&t), &t)))
}

/// Kaiser normalization: rows scaled to unit length before rotating, and back after.
fn kaiser(a: &Mat) -> (Mat, Vec<f64>) {
    let h: Vec<f64> = a.iter().map(|r| r.iter().map(|v| v * v).sum::<f64>().sqrt()).collect();
    (a.iter().zip(&h).map(|(r, s)| r.iter().map(|v| v / s).collect()).collect(), h)
}

fn varimax(a: &Mat) -> (Mat, Mat) {
    let (an, h) = kaiser(a);
    let (l, t) = gp_orth(&an, varimax_crit);
    (l.iter().zip(&h).map(|(r, s)| r.iter().map(|v| v * s).collect()).collect(), t)
}

/// psych's Promax: varimax, then a least-squares fit to the loadings raised to the fourth power (keeping signs).
fn promax(a: &Mat) -> Option<(Mat, Mat)> {
    let (x, rot) = varimax(a);
    let q: Mat = x.iter().map(|r| r.iter().map(|v| v * v.abs().powi(3)).collect()).collect();
    let xtx_inv = invert(&mul(&transpose(&x), &x))?;
    let u = mul(&xtx_inv, &mul(&transpose(&x), &q));
    let d = invert(&mul(&transpose(&u), &u))?;
    let u: Mat = u.iter().map(|r| r.iter().enumerate().map(|(k, v)| v * d[k][k].sqrt()).collect()).collect();
    let z = mul(&x, &u);
    let u_full = mul(&rot, &u);
    let ui = invert(&u_full)?;
    let phi = mul(&ui, &transpose(&ui));
    Some((z, phi))
}

/// Orders factors by the variance they account for and signs each so its loadings sum positive.
fn tidy(l: &Mat, phi: Option<&Mat>) -> (Mat, Option<Mat>) {
    let m = l[0].len();
    let ss: Vec<f64> = (0..m).map(|k| l.iter().map(|r| r[k] * r[k]).sum()).collect();
    let mut order: Vec<usize> = (0..m).collect();
    order.sort_by(|&a, &b| ss[b].total_cmp(&ss[a]));
    let sign: Vec<f64> = (0..m).map(|k| if l.iter().map(|r| r[k]).sum::<f64>() < 0.0 { -1.0 } else { 1.0 }).collect();
    let nl = l.iter().map(|r| order.iter().map(|&k| r[k] * sign[k]).collect()).collect();
    let nphi = phi.map(|p| order.iter().map(|&a| order.iter().map(|&b| p[a][b] * sign[a] * sign[b]).collect()).collect());
    (nl, nphi)
}

/// Standard normal draws (Box-Muller) from the seeded generator.
fn normal(rng: &mut SplitMix64) -> f64 {
    let u = ((rng.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
    let v = ((rng.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
    (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()
}

/// Horn's parallel analysis: the 95th percentile of each eigenvalue over 100 random normal datasets of n × p.
pub fn parallel(n: usize, p: usize, seed: u64) -> Vec<f64> {
    let mut rng = SplitMix64::new(seed);
    let reps = 100;
    let mut all: Vec<Vec<f64>> = vec![Vec::with_capacity(reps); p];
    for _ in 0..reps {
        let cols: Vec<Vec<f64>> = (0..p).map(|_| (0..n).map(|_| normal(&mut rng)).collect()).collect();
        let vals = symmetric_eigen(&correlations(&cols)).0;
        for (k, v) in vals.into_iter().enumerate() {
            all[k].push(v);
        }
    }
    all.into_iter()
        .map(|mut v| {
            v.sort_by(f64::total_cmp);
            v[((0.95 * reps as f64).ceil() as usize).min(reps) - 1]
        })
        .collect()
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    let p = spec.variables.len();
    if p < 3 {
        return Err(Error::Invalid("Choose at least three variables.".into()));
    }
    for (i, a) in spec.variables.iter().enumerate() {
        if spec.variables[..i].iter().any(|b| ds.find(a) == ds.find(b)) {
            return Err(Error::Invalid(format!("{a} is listed twice.")));
        }
    }
    let labels: Vec<String> = spec.variables.iter().map(|n| ds.variable(n).map(|v| v.display_name().to_string())).collect::<Result<_>>()?;
    let cells: Vec<Vec<Slot<f64>>> = spec.variables.iter().map(|n| numeric(ds, n)).collect::<Result<_>>()?;
    let total = cells[0].len();
    let mut tally = Tally::default();
    let used: Vec<usize> = (0..total).filter(|&r| !cells.iter().zip(&spec.variables).any(|(c, n)| tally.check(&c[r], n))).collect();
    let cols: Vec<Vec<f64>> = cells.iter().map(|c| used.iter().map(|&r| c[r].value().expect("checked")).collect()).collect();
    let n = used.len();
    let pca = spec.extraction == Extraction::Pca;
    let (ext_name, unit, units) = match spec.extraction {
        Extraction::Pca => ("principal components analysis", "component", "components"),
        Extraction::Ml => ("maximum likelihood factor analysis", "factor", "factors"),
        Extraction::Paf => ("principal axis factor analysis", "factor", "factors"),
        Extraction::Minres => ("minimum residual factor analysis", "factor", "factors"),
    };
    let rot_name = match spec.rotation {
        Rotation::None => "no rotation",
        Rotation::Varimax => "varimax rotation (orthogonal, Kaiser-normalized)",
        Rotation::Oblimin => "oblimin rotation (oblique, quartimin)",
        Rotation::Promax => "promax rotation (oblique, power 4)",
    };
    let note = format!(
        "{} of the correlation matrix with {rot_name}. Sampling adequacy: Kaiser-Meyer-Olkin; Bartlett's test of sphericity. Number of {units}: {}. Cases were used when every variable had a valid value (listwise). Computed by PrismStatistics.",
        apa::sentence_case(ext_name),
        match spec.factors {
            Some(k) => format!("{k}, as chosen"),
            None => format!("parallel analysis (eigenvalues above the 95th percentile of those from 100 random normal datasets of the same size, seed {})", spec.seed),
        }
    );
    let mut b = common::bundle(METHOD_ID, METHOD_VERSION, format!("{}: {}", if pca { "Principal Components" } else { "Factor Analysis" }, labels.join(", ")), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, n), note);
    set_text(&mut b, "extraction", ext_name);
    set_text(&mut b, "rotation", rot_name);
    set_text(&mut b, "unit", unit);
    set(&mut b, "p", p as f64);
    set(&mut b, "n", n as f64);
    for (i, l) in labels.iter().enumerate() {
        set_text(&mut b, format!("var.{}.label", i + 1), l.clone());
    }
    if n <= p {
        fail(&mut b, format!("There are {n} cases with every variable, and the analysis needs more cases than variables ({p})."));
        return Ok(b);
    }
    let r = correlations(&cols);
    let Some(rinv) = invert(&r) else {
        fail(&mut b, "The correlation matrix is singular: some variable is a combination of the others (or constant). Remove it and run again.");
        return Ok(b);
    };
    // Adequacy.
    let mut num_r = 0.0;
    let mut num_q = 0.0;
    let mut msa = Vec::new();
    for i in 0..p {
        let (mut ri, mut qi) = (0.0, 0.0);
        for j in 0..p {
            if i != j {
                let q = -rinv[i][j] / (rinv[i][i] * rinv[j][j]).sqrt();
                ri += r[i][j] * r[i][j];
                qi += q * q;
            }
        }
        num_r += ri;
        num_q += qi;
        msa.push(ri / (ri + qi));
    }
    let kmo = num_r / (num_r + num_q);
    let bart = -((n - 1) as f64 - (2.0 * p as f64 + 5.0) / 6.0) * det(&r).ln();
    let bart_df = (p * (p - 1)) as f64 / 2.0;
    let bart_p = chisq_sf(bart, bart_df);
    for (k, v) in [("kmo", kmo), ("bartlett.chi2", bart), ("bartlett.df", bart_df), ("bartlett.p", bart_p)] {
        set(&mut b, k, v);
    }
    for (i, v) in msa.iter().enumerate() {
        set(&mut b, format!("var.{}.msa", i + 1), *v);
    }
    b.tables.push(Table {
        id: "adequacy".into(),
        title: "KMO and Bartlett's test".into(),
        columns: vec!["Measure".into(), "Value".into()],
        rows: vec![
            vec![text("Kaiser-Meyer-Olkin measure of sampling adequacy"), num(kmo, 3)],
            vec![text("Bartlett's test: chi-square"), num(bart, 3)],
            vec![text("df"), num(bart_df, 0)],
            vec![text("p"), common::p(bart_p)],
        ],
        footnotes: vec![format!("Per-variable adequacy: {}.", labels.iter().zip(&msa).map(|(l, m)| format!("{l} {}", apa::no_leading_zero(*m, 2))).collect::<Vec<_>>().join(", "))],
    });
    b.diagnostics.push(Diagnostic {
        id: "adequacy".into(),
        title: "Sampling adequacy".into(),
        state: if kmo < 0.6 { DiagnosticState::ReviewNeeded } else { DiagnosticState::NoConcernDetected },
        message: format!("KMO = {}{}.", apa::no_leading_zero(kmo, 2), if kmo < 0.6 { "; below .60 the correlations may be too weak or too scattered to factor" } else { "" }),
    });

    // Eigenvalues and how many.
    let eig = symmetric_eigen(&r).0;
    let sim = if spec.parallel || spec.factors.is_none() { Some(parallel(n, p, spec.seed)) } else { None };
    let suggested = sim.as_ref().map(|s| eig.iter().zip(s).take_while(|(e, s)| e > s).count());
    if let Some(k) = suggested {
        set(&mut b, "parallel.suggested", k as f64);
    }
    let max_factors = if pca { p } else { (2 * p + 1 - ((8 * p + 1) as f64).sqrt().ceil() as usize) / 2 };
    let m = spec.factors.unwrap_or(suggested.unwrap_or(1).max(1));
    if m == 0 || m > max_factors.max(1) || (!pca && m >= p) {
        fail(&mut b, format!("{m} {units} can't be extracted from {p} variables{}.", if pca { String::new() } else { format!("; with {p} variables a factor analysis can have at most {max_factors}") }));
        return Ok(b);
    }
    set(&mut b, "m", m as f64);
    let mut eig_rows = Vec::new();
    let mut cum = 0.0;
    for (k, e) in eig.iter().enumerate() {
        cum += e / p as f64 * 100.0;
        set(&mut b, format!("eigen.{}", k + 1), *e);
        let mut row = vec![count(k + 1), num(*e, 3), num(e / p as f64 * 100.0, 2), num(cum, 2)];
        if let Some(s) = &sim {
            set(&mut b, format!("parallel.{}", k + 1), s[k]);
            row.push(num(s[k], 3));
        }
        eig_rows.push(row);
    }
    let mut ecols = vec!["Number".into(), "Eigenvalue".into(), "% of variance".into(), "Cumulative %".into()];
    if sim.is_some() {
        ecols.push("Random data (95th percentile)".into());
    }
    b.tables.push(Table {
        id: "eigenvalues".into(),
        title: "Eigenvalues of the correlation matrix".into(),
        columns: ecols,
        rows: eig_rows,
        footnotes: suggested.map(|k| vec![format!("Parallel analysis suggests {k} {}: the eigenvalues above the random-data line.", if k == 1 { unit } else { units })]).unwrap_or_default(),
    });
    if spec.scree {
        let cats: Vec<String> = (1..=p).map(|k| k.to_string()).collect();
        let mut series = vec![("Data".to_string(), eig.clone())];
        if let Some(s) = &sim {
            series.push(("Random data (95th percentile)".to_string(), s.clone()));
        }
        let alt = format!("Scree plot of the {p} eigenvalues{}.", if sim.is_some() { " with the parallel-analysis line" } else { "" });
        b.charts.push(Chart { id: "scree".into(), title: "Scree plot".into(), alt: alt.clone(), svg: chart::profile(&cats, &series, "Number", "Eigenvalue", "", &alt) });
    }

    // Extraction.
    let mut heywood = false;
    let mut ml_f = None;
    let unrotated: Mat = match spec.extraction {
        Extraction::Pca => principal_axes(&r, m).0,
        Extraction::Paf | Extraction::Minres => {
            let (l, h) = paf(&r, m, &rinv);
            heywood = h;
            l
        }
        Extraction::Ml => {
            let (l, psi, f) = ml(&r, m, &rinv);
            heywood = psi.iter().any(|v| *v <= 0.0051);
            ml_f = Some(f);
            l
        }
    };
    let comm: Vec<f64> = unrotated.iter().map(|row| row.iter().map(|v| v * v).sum()).collect();
    let initial: Vec<f64> = if pca { vec![1.0; p] } else { (0..p).map(|i| 1.0 - 1.0 / rinv[i][i]).collect() };
    for (i, (c, ini)) in comm.iter().zip(&initial).enumerate() {
        set(&mut b, format!("var.{}.communality", i + 1), *c);
        set(&mut b, format!("var.{}.initial", i + 1), *ini);
    }
    b.tables.push(Table {
        id: "communalities".into(),
        title: "Communalities".into(),
        columns: vec!["Variable".into(), "Initial".into(), "Extraction".into()],
        rows: labels.iter().zip(&initial).zip(&comm).map(|((l, a), c)| vec![text(l), num(*a, 3), num(*c, 3)]).collect(),
        footnotes: if pca { vec!["Initial communalities of components are 1.".into()] } else { vec!["Initial communalities are squared multiple correlations.".into()] },
    });
    if heywood {
        b.diagnostics.push(Diagnostic { id: "heywood".into(), title: "Heywood case".into(), state: DiagnosticState::ReviewNeeded, message: "A communality reached 1 (a uniqueness at its lower bound): the solution is improper. Try fewer factors or another extraction.".into() });
    }
    if let Some(f) = ml_f {
        let nf = n as f64;
        let stat = (nf - 1.0 - (2.0 * p as f64 + 5.0) / 6.0 - 2.0 * m as f64 / 3.0) * f;
        let df = (((p - m) * (p - m)) as f64 - (p + m) as f64) / 2.0;
        if df > 0.0 {
            set(&mut b, "fit.chi2", stat);
            set(&mut b, "fit.df", df);
            set(&mut b, "fit.p", chisq_sf(stat, df));
        }
    }

    // Rotation.
    let (rotated, phi) = if m == 1 {
        (unrotated.clone(), None)
    } else {
        match spec.rotation {
            Rotation::None => (unrotated.clone(), None),
            Rotation::Varimax => (varimax(&unrotated).0, None),
            Rotation::Oblimin => match gp_oblique(&unrotated, oblimin_crit) {
                Some((l, phi)) => (l, Some(phi)),
                None => (unrotated.clone(), None),
            },
            Rotation::Promax => match promax(&unrotated) {
                Some((l, phi)) => (l, Some(phi)),
                None => (unrotated.clone(), None),
            },
        }
    };
    let (loadings, phi) = tidy(&rotated, phi.as_ref());
    let structure = phi.as_ref().map(|ph| mul(&loadings, ph));
    let fname = |k: usize| format!("{} {}", apa::sentence_case(unit), k + 1);
    let hide = spec.hide_below;
    let mut rows_l = Vec::new();
    for (i, l) in labels.iter().enumerate() {
        let mut row = vec![text(l)];
        for (k, v) in loadings[i].iter().enumerate() {
            set(&mut b, format!("var.{}.loading.{}", i + 1, k + 1), *v);
            row.push(if v.abs() < hide { Cell::Empty } else { num(*v, 3) });
        }
        row.push(num(comm[i], 3));
        rows_l.push(row);
    }
    let ss: Vec<f64> = (0..m).map(|k| loadings.iter().map(|r| r[k] * r[k]).sum()).collect();
    for (k, s) in ss.iter().enumerate() {
        set(&mut b, format!("factor.{}.ss", k + 1), *s);
        set(&mut b, format!("factor.{}.pct", k + 1), s / p as f64 * 100.0);
    }
    let total_pct: f64 = comm.iter().sum::<f64>() / p as f64 * 100.0;
    set(&mut b, "variance.pct", total_pct);
    let mut ss_row = vec![text("Sum of squared loadings")];
    ss_row.extend(ss.iter().map(|s| num(*s, 3)));
    ss_row.push(Cell::Empty);
    rows_l.push(ss_row);
    if phi.is_none() {
        let mut pct_row = vec![text("% of variance")];
        pct_row.extend(ss.iter().map(|s| num(s / p as f64 * 100.0, 2)));
        pct_row.push(Cell::Empty);
        rows_l.push(pct_row);
    }
    let mut lcols = vec!["Variable".into()];
    lcols.extend((0..m).map(fname));
    lcols.push("Communality".into());
    let mut lfoot = Vec::new();
    if hide > 0.0 {
        lfoot.push(format!("Loadings below {} in absolute value are hidden.", apa::no_leading_zero(hide, 2)));
    }
    if phi.is_some() {
        lfoot.push("With an oblique rotation these are pattern loadings; the sums of squared loadings can't be added into a total because the factors are correlated.".into());
    }
    lfoot.push(format!("Together the {units} account for {}% of the variance of the variables.", apa::num(total_pct, 1)));
    b.tables.push(Table { id: "loadings".into(), title: if phi.is_some() { "Pattern matrix".into() } else if m > 1 && spec.rotation != Rotation::None { "Rotated loadings".into() } else { "Loadings".into() }, columns: lcols.clone(), rows: rows_l, footnotes: lfoot });
    if let (Some(st), Some(ph)) = (&structure, &phi) {
        let mut scols = vec!["Variable".into()];
        scols.extend((0..m).map(fname));
        b.tables.push(Table { id: "structure".into(), title: "Structure matrix".into(), columns: scols, rows: labels.iter().zip(st).map(|(l, r)| std::iter::once(text(l)).chain(r.iter().map(|v| num(*v, 3))).collect()).collect(), footnotes: vec!["Correlations between the variables and the factors.".into()] });
        let mut pcols = vec![String::new()];
        pcols.extend((0..m).map(fname));
        for (i, row) in ph.iter().enumerate() {
            for (j, v) in row.iter().enumerate().skip(i + 1) {
                set(&mut b, format!("phi.{}.{}", i + 1, j + 1), *v);
            }
        }
        b.tables.push(Table { id: "phi".into(), title: format!("{} correlations", apa::sentence_case(unit)), columns: pcols, rows: (0..m).map(|i| std::iter::once(text(fname(i))).chain(ph[i].iter().map(|v| num(*v, 3))).collect()).collect(), footnotes: Vec::new() });
    }
    b.diagnostics.push(size_diagnostic(n, format!("{n} cases for {p} variables")));
    Ok(b)
}
