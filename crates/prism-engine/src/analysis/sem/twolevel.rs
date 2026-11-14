//! Two-level structural equation models (checklist C4d): random-intercept models with a within-cluster part
//! (`level: 1`) and a between-cluster part (`level: 2`), as lavaan fits them with `cluster =`.
//!
//! Each observed variable has a within part, a between part, or both: one named only at level 1 is within-only, one
//! named only at level 2 is between-only (constant within a cluster). Case i of cluster j is
//!
//!   y_ij = μ + u_j + e_ij,  Cov(e_ij) = Σ_W from the level-1 model, Cov(u_j) = Σ_B from the level-2 model,
//!
//! with the level-1 intercepts of variables that also have a between part fixed at 0 (their means are the level-2
//! intercepts). The log-likelihood is exact for clusters of any size: an orthogonal (Helmert) transform splits each
//! cluster into n_j − 1 within contrasts, each with covariance Σ_W, and the scaled cluster mean √n_j ȳ_j, which with
//! the between-only variables z_j has covariance C_j = [[Σ_W + n_j Σ_B, √n_j Σ_B], [√n_j Σ_B, Σ_B]].
//!
//! The gradient is analytic (through each level's parameters by the chain rule); estimation is Newton's method with
//! the Hessian taken from the gradient by central differences, which also gives the observed-information standard
//! errors lavaan reports. The saturated model (unrestricted Σ_W, Σ_B and means) and the baseline (variances only) are
//! fitted the same way, for χ², CFI, TLI and RMSEA; SRMR is the sum of its within and between parts. Observed
//! predictors that are not modeled (fixed.x) take the saturated model's estimates of their moments, as lavaan does.

use prismqda_stats::dist::{chisq_sf, norm_quantile, norm_two_sided};

use super::model::{Implied, Matrix, Model, Options, Param};
use super::syntax::{self, Statement};
use super::{estimate, Spec};
use crate::analysis::common::{self, *};
use crate::analysis::linalg::{cholesky, invert, Mat};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

const LN_2PI: f64 = 1.837_877_066_409_345_3;

/// Whether a model text has level: blocks.
pub fn is_two_level(text: &str) -> bool {
    syntax::parse(text).is_ok_and(|s| s.iter().any(|x| matches!(x, Statement::Level(_))))
}

struct Cluster {
    n: f64,
    /// Means over all variables (U).
    mean: Vec<f64>,
}

/// The data and the variable sets.
struct Ctx {
    /// Level-1 variables (V) and level-2 variables (B), as U indices, in the order of the level models' obs.
    v: Vec<usize>,
    b: Vec<usize>,
    /// Between-only variables (Z).
    z: Vec<usize>,
    /// A = V then Z: position in A of each U index.
    apos: Vec<Option<usize>>,
    clusters: Vec<Cluster>,
    /// Σ_j of the within scatter, over V.
    scatter: Mat,
    n: f64,
}

impl Ctx {
    fn pa(&self) -> usize {
        self.v.len() + self.z.len()
    }
}

/// A model of each level; θ is the within level's free parameters followed by the between level's.
#[derive(Clone)]
struct Pair {
    w: Model,
    b: Model,
}

impl Pair {
    fn q(&self) -> usize {
        self.w.nfree + self.b.nfree
    }
    fn theta(&self) -> Vec<f64> {
        let mut t = self.w.theta();
        t.extend(self.b.theta());
        t
    }
}

fn log_det(m: &Mat) -> Option<f64> {
    let l = cholesky(m)?;
    Some(2.0 * l.iter().enumerate().map(|(i, r)| r[i].ln()).sum::<f64>())
}

/// The implied moments: Σ_W over V, Σ_B over A (zero for within-only), μ over A.
fn moments(pair: &Pair, ctx: &Ctx, th: &[f64]) -> Option<(Implied, Implied, Mat, Mat, Vec<f64>)> {
    let (tw, tb) = th.split_at(pair.w.nfree);
    let iw = pair.w.implied_full(0, tw)?;
    let ib = pair.b.implied_full(0, tb)?;
    let pv = ctx.v.len();
    let pa = ctx.pa();
    let sw: Mat = (0..pv).map(|i| (0..pv).map(|j| iw.full[i][j]).collect()).collect();
    let mut sb = vec![vec![0.0; pa]; pa];
    let mut mu = vec![0.0; pa];
    for (i, &u) in ctx.v.iter().enumerate() {
        mu[ctx.apos[u].expect("in A")] += iw.mean[i];
    }
    for (i, &ui) in ctx.b.iter().enumerate() {
        let ai = ctx.apos[ui].expect("in A");
        mu[ai] += ib.mean[i];
        for (j, &uj) in ctx.b.iter().enumerate() {
            sb[ai][ctx.apos[uj].expect("in A")] = ib.full[i][j];
        }
    }
    Some((iw, ib, sw, sb, mu))
}

/// The log-likelihood and (if asked) its gradient over θ.
fn loglik(pair: &Pair, ctx: &Ctx, th: &[f64], grad: bool) -> Option<(f64, Vec<f64>)> {
    let (iw, ib, sw, sb, mu) = moments(pair, ctx, th)?;
    let pv = ctx.v.len();
    let pa = ctx.pa();
    let ldw = log_det(&sw)?;
    let wi = invert(&sw)?;
    let mut ll = 0.0;
    // Within contrasts.
    let nc = ctx.clusters.len() as f64;
    let tr: f64 = (0..pv).map(|i| (0..pv).map(|j| wi[i][j] * ctx.scatter[j][i]).sum::<f64>()).sum();
    ll -= 0.5 * ((ctx.n - nc) * (pv as f64 * LN_2PI + ldw) + tr);
    let mut gw: Mat = (0..pv).map(|i| (0..pv).map(|j| -0.5 * (ctx.n - nc) * wi[i][j]).collect()).collect();
    if grad {
        let wsw = mat_mul(&mat_mul(&wi, &ctx.scatter), &wi);
        for i in 0..pv {
            for j in 0..pv {
                gw[i][j] += 0.5 * wsw[i][j];
            }
        }
    }
    let mut gb = vec![vec![0.0; pa]; pa];
    let mut gmu = vec![0.0; pa];
    // Cluster means, grouped by cluster size.
    let mut sizes: Vec<f64> = ctx.clusters.iter().map(|c| c.n).collect();
    sizes.sort_by(f64::total_cmp);
    sizes.dedup();
    for n in sizes {
        let s: Vec<f64> = (0..pa).map(|a| if a < pv { n.sqrt() } else { 1.0 }).collect();
        let c: Mat = (0..pa).map(|a| (0..pa).map(|b| s[a] * sb[a][b] * s[b] + if a < pv && b < pv { sw[a][b] } else { 0.0 }).collect()).collect();
        let ldc = log_det(&c)?;
        let ci = invert(&c)?;
        let mut count = 0.0;
        let mut rr = vec![vec![0.0; pa]; pa];
        let mut rsum = vec![0.0; pa];
        for cl in ctx.clusters.iter().filter(|c| c.n == n) {
            let mut r = vec![0.0; pa];
            for (u, pos) in ctx.apos.iter().enumerate() {
                if let Some(a) = *pos {
                    r[a] = s[a] * (cl.mean[u] - mu[a]);
                }
            }
            let q: f64 = (0..pa).map(|a| r[a] * (0..pa).map(|b| ci[a][b] * r[b]).sum::<f64>()).sum();
            ll -= 0.5 * (pa as f64 * LN_2PI + ldc + q);
            count += 1.0;
            if grad {
                for a in 0..pa {
                    rsum[a] += r[a];
                    for b in 0..pa {
                        rr[a][b] += r[a] * r[b];
                    }
                }
            }
        }
        if grad {
            // G_C = −½ Σ (C⁻¹ − C⁻¹ r r' C⁻¹) over the clusters of this size.
            let crc = mat_mul(&mat_mul(&ci, &rr), &ci);
            for a in 0..pa {
                for b in 0..pa {
                    let g = -0.5 * (count * ci[a][b] - crc[a][b]);
                    if a < pv && b < pv {
                        gw[a][b] += g;
                    }
                    gb[a][b] += s[a] * g * s[b];
                }
                gmu[a] += s[a] * (0..pa).map(|b| ci[a][b] * rsum[b]).sum::<f64>();
            }
        }
    }
    if !grad {
        return Some((ll, Vec::new()));
    }
    // The chain rule through each level's parameters.
    let mut g = vec![0.0; pair.q()];
    for (level, (model, imp, offset)) in [(&pair.w, &iw, 0), (&pair.b, &ib, pair.w.nfree)].into_iter().enumerate() {
        let p = model.p();
        let map: Vec<usize> = (0..p).map(|i| ctx.apos[if level == 0 { ctx.v[i] } else { ctx.b[i] }].expect("in A")).collect();
        for (k, set) in estimate::places(model).iter().enumerate() {
            let (mut dc, mut dm) = (vec![vec![0.0; p]; p], vec![0.0; p]);
            for pl in set {
                imp.derivative(pl, p, &mut dc, &mut dm);
            }
            let mut d = 0.0;
            for i in 0..p {
                d += gmu[map[i]] * dm[i];
                for j in 0..p {
                    d += dc[i][j] * if level == 0 { gw[i][j] } else { gb[map[i]][map[j]] };
                }
            }
            g[offset + k] = d;
        }
    }
    Some((ll, g))
}

fn mat_mul(a: &Mat, b: &Mat) -> Mat {
    let n = b.first().map_or(0, Vec::len);
    a.iter().map(|r| (0..n).map(|j| r.iter().zip(b).map(|(x, br)| x * br[j]).sum()).collect()).collect()
}

fn set_theta(pair: &mut Pair, th: &[f64]) {
    let (tw, tb) = th.split_at(pair.w.nfree);
    pair.w.set_theta(tw);
    pair.b.set_theta(tb);
}

/// The Hessian of the log-likelihood, by central differences of the analytic gradient.
fn hessian(pair: &Pair, ctx: &Ctx, th: &[f64]) -> Option<Mat> {
    let q = th.len();
    let mut h = vec![vec![0.0; q]; q];
    for k in 0..q {
        let step = 1e-5 * th[k].abs().max(1.0);
        let (mut up, mut dn) = (th.to_vec(), th.to_vec());
        up[k] += step;
        dn[k] -= step;
        let (_, gu) = loglik(pair, ctx, &up, true)?;
        let (_, gd) = loglik(pair, ctx, &dn, true)?;
        for j in 0..q {
            h[j][k] = (gu[j] - gd[j]) / (2.0 * step);
        }
    }
    Some((0..q).map(|i| (0..q).map(|j| 0.5 * (h[i][j] + h[j][i])).collect()).collect())
}

struct Fitted {
    th: Vec<f64>,
    ll: f64,
    iterations: usize,
    converged: bool,
}

/// Newton's method with a line search (and a ridge when the Hessian isn't negative definite).
fn maximize(pair: &Pair, ctx: &Ctx) -> Option<Fitted> {
    let mut th = pair.theta();
    let (mut ll, _) = loglik(pair, ctx, &th, false)?;
    let q = th.len();
    let mut converged = false;
    let mut iterations = 0;
    for it in 0..500 {
        iterations = it + 1;
        let (_, g) = loglik(pair, ctx, &th, true)?;
        if g.iter().all(|v| v.abs() < 1e-9 * ctx.n.max(1.0)) {
            converged = true;
            break;
        }
        let h = hessian(pair, ctx, &th)?;
        let mut step = None;
        let mut ridge = 0.0;
        for _ in 0..40 {
            let m: Mat = (0..q).map(|i| (0..q).map(|j| -h[i][j] + if i == j { ridge } else { 0.0 }).collect()).collect();
            if cholesky(&m).is_some() {
                let mi = invert(&m)?;
                step = Some(mi.iter().map(|r| r.iter().zip(&g).map(|(a, b)| a * b).sum::<f64>()).collect::<Vec<f64>>());
                break;
            }
            ridge = if ridge == 0.0 { 1e-6 * (0..q).map(|i| h[i][i].abs()).fold(1.0, f64::max) } else { ridge * 10.0 };
        }
        let step = step?;
        let mut alpha = 1.0;
        let mut moved = false;
        while alpha > 1e-12 {
            let cand: Vec<f64> = th.iter().zip(&step).map(|(t, s)| t + alpha * s).collect();
            if let Some((lc, _)) = loglik(pair, ctx, &cand, false) {
                if lc >= ll - 1e-12 * ll.abs() {
                    let tiny = step.iter().all(|s| (alpha * s).abs() < 1e-11);
                    th = cand;
                    ll = lc;
                    moved = true;
                    converged = tiny;
                    break;
                }
            }
            alpha /= 2.0;
        }
        if !moved || converged {
            converged = converged || g.iter().all(|v| v.abs() < 1e-6 * ctx.n.max(1.0));
            break;
        }
    }
    Some(Fitted { th, ll, iterations, converged })
}

/// A level model with every variance and covariance free (or, `base`, variances only), the moments of the fixed
/// predictors `exo` fixed, and intercepts free except the level-1 intercepts of variables that also have a between
/// part.
fn open_model(obs: &[String], exo: &[usize], fixed_zero_mean: &[bool], base: bool) -> Model {
    let mut m = Model { obs: obs.to_vec(), lat: Vec::new(), groups: 1, params: Vec::new(), nfree: 0, defined: Vec::new(), meanstructure: true, xvars: exo.to_vec(), ordinal: Vec::new() };
    let p = obs.len();
    for i in 0..p {
        for j in i..p {
            let x = exo.contains(&i) && exo.contains(&j);
            if i != j && base && !x {
                continue;
            }
            let free = if x {
                None
            } else {
                m.nfree += 1;
                Some(m.nfree - 1)
            };
            m.params.push(Param { group: 0, lhs: obs[i].clone(), op: "~~", rhs: obs[j].clone(), label: None, free, value: 0.0, start_given: false, user: false, exo: x, mat: Matrix::S, i, j });
        }
    }
    for i in 0..p {
        let x = exo.contains(&i);
        let free = if x || fixed_zero_mean[i] {
            None
        } else {
            m.nfree += 1;
            Some(m.nfree - 1)
        };
        m.params.push(Param { group: 0, lhs: obs[i].clone(), op: "~1", rhs: String::new(), label: None, free, value: 0.0, start_given: false, user: false, exo: x, mat: Matrix::M, i, j: 0 });
    }
    m
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    check_level(spec.level)?;
    let Some(cluster) = spec.cluster.as_ref() else {
        return Err(Error::Invalid("A two-level model (level: 1 and level: 2) needs a cluster variable.".into()));
    };
    if spec.group.is_some() || spec.estimator != super::Estimator::Ml || spec.missing != super::Missing::Listwise || spec.bootstrap > 0 || spec.growth {
        return Err(Error::Invalid("Two-level models are fitted by maximum likelihood to complete cases, in one group, without the bootstrap.".into()));
    }
    let statements = syntax::parse(&spec.model)?;
    let mut levels: [Vec<Statement>; 2] = [Vec::new(), Vec::new()];
    let mut current = 0;
    for s in statements {
        match s {
            Statement::Level(l) => current = l,
            Statement::Defined { .. } | Statement::Equal { .. } => return Err(Error::Invalid("Defined parameters and constraints are not supported in two-level models yet.".into())),
            other => {
                if current == 0 {
                    return Err(Error::Invalid("Start a two-level model with level: 1 (or level: 2) before its statements.".into()));
                }
                levels[current - 1].push(other);
            }
        }
    }
    if levels.iter().any(Vec::is_empty) {
        return Err(Error::Invalid("A two-level model needs statements at both level: 1 and level: 2.".into()));
    }
    let observed = |n: &str| ds.find(n).is_some() && n != cluster;
    let opts = Options { std_lv: spec.std_lv, meanstructure: true, fixed_x: spec.fixed_x, growth: false };
    let mut w = Model::build(&levels[0], &observed, &opts, 1)?;
    let mut b = Model::build(&levels[1], &observed, &opts, 1)?;
    // Variables with a between part have their level-1 intercepts fixed at 0.
    for q in w.params.iter_mut().filter(|q| q.op == "~1" && q.i < w.obs.len()) {
        if b.obs.contains(&q.lhs) && !q.user {
            q.free = None;
            q.value = 0.0;
        }
    }
    w.renumber();
    let mut vars: Vec<String> = w.obs.clone();
    for o in &b.obs {
        if !vars.contains(o) {
            vars.push(o.clone());
        }
    }
    let v: Vec<usize> = w.obs.iter().map(|o| vars.iter().position(|x| x == o).expect("a var")).collect();
    let bi: Vec<usize> = b.obs.iter().map(|o| vars.iter().position(|x| x == o).expect("a var")).collect();
    let z: Vec<usize> = bi.iter().copied().filter(|u| !v.contains(u)).collect();
    let mut apos = vec![None; vars.len()];
    for (a, &u) in v.iter().chain(&z).enumerate() {
        apos[u] = Some(a);
    }

    // Data: complete cases, by cluster in order of first appearance.
    let cols: Vec<Vec<Slot<f64>>> = vars.iter().map(|x| numeric(ds, x)).collect::<Result<_>>()?;
    // Cluster identifiers: any number of codes or texts.
    let ci = ds.find(cluster).ok_or_else(|| Error::UnknownVariable(cluster.clone()))?;
    let cslots: Vec<Slot<usize>> = match &ds.columns[ci] {
        crate::model::Column::Numeric(_) => {
            let cells = numeric(ds, cluster)?;
            let mut codes: Vec<f64> = cells.iter().filter_map(Slot::value).collect();
            codes.sort_by(f64::total_cmp);
            codes.dedup();
            cells.iter().map(|c| match c {
                Slot::Valid(v) => Slot::Valid(codes.binary_search_by(|k| k.total_cmp(v)).expect("listed")),
                Slot::Empty => Slot::Empty,
                Slot::UserMissing => Slot::UserMissing,
            }).collect()
        }
        crate::model::Column::Text(t) => {
            let mut keys: Vec<&String> = t.iter().flatten().collect();
            keys.sort();
            keys.dedup();
            t.iter().map(|c| c.as_ref().map_or(Slot::Empty, |v| Slot::Valid(keys.binary_search(&v).expect("listed")))).collect()
        }
    };
    let total = ds.n_rows();
    let mut tally = Tally::default();
    let mut order: Vec<usize> = Vec::new();
    let mut rows: Vec<Vec<Vec<f64>>> = Vec::new();
    'case: for r in 0..total {
        if tally.check(&cslots[r], "cluster") {
            continue;
        }
        let mut row = Vec::with_capacity(vars.len());
        for (c, name) in cols.iter().zip(&vars) {
            if tally.check(&c[r], name) {
                continue 'case;
            }
            row.push(c[r].value().expect("checked"));
        }
        let cl = cslots[r].value().expect("checked");
        let k = match order.iter().position(|o| *o == cl) {
            Some(k) => k,
            None => {
                order.push(cl);
                rows.push(Vec::new());
                rows.len() - 1
            }
        };
        rows[k].push(row);
    }
    let n: f64 = rows.iter().map(|r| r.len() as f64).sum();
    let pv = v.len();
    let mut scatter = vec![vec![0.0; pv]; pv];
    let mut clusters = Vec::new();
    let mut varying_z = false;
    for rs in &rows {
        let nj = rs.len() as f64;
        let mean: Vec<f64> = (0..vars.len()).map(|u| rs.iter().map(|r| r[u]).sum::<f64>() / nj).collect();
        for r in rs {
            for (i, &ui) in v.iter().enumerate() {
                for (j, &uj) in v.iter().enumerate() {
                    scatter[i][j] += (r[ui] - mean[ui]) * (r[uj] - mean[uj]);
                }
            }
            if z.iter().any(|&u| (r[u] - mean[u]).abs() > 1e-9 * mean[u].abs().max(1.0)) {
                varying_z = true;
            }
        }
        clusters.push(Cluster { n: nj, mean });
    }
    let ctx = Ctx { v: v.clone(), b: bi.clone(), z: z.clone(), apos, clusters, scatter, n };
    let g = ctx.clusters.len();
    let lv = level_text(spec.level);
    let note = format!(
        "Two-level structural equation model (random intercepts) estimated by maximum likelihood, as lavaan with cluster = {cluster}: the level-1 model for the within-cluster covariances and the level-2 model for the between-cluster covariances and means, with the exact likelihood for unequal cluster sizes. Standard errors from the observed information; z tests and {lv} Wald intervals. χ² against the saturated two-level model; SRMR is the sum of its within and between parts. Computed by PrismStatistics."
    );
    let mut bdl = common::bundle(super::SEM, super::METHOD_VERSION, "Two-Level Structural Equation Model".into(), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, n as usize), note);
    set(&mut bdl, "n", n);
    set(&mut bdl, "clusters", g as f64);
    set_text(&mut bdl, "estimator", "ML");
    set(&mut bdl, "level", spec.level);
    if g < 3 || ctx.clusters.iter().all(|c| c.n < 2.0) {
        fail(&mut bdl, "There are too few clusters (or every cluster has one case).");
        return Ok(bdl);
    }

    // Starting moments: pooled within covariance, and the between covariance of the cluster means less the share of
    // within variance in them.
    let nbar = n / g as f64;
    let sw0: Mat = (0..pv).map(|i| (0..pv).map(|j| ctx.scatter[i][j] / (n - g as f64)).collect()).collect();
    let pb = bi.len();
    let mb0: Vec<f64> = bi.iter().map(|&u| ctx.clusters.iter().map(|c| c.mean[u]).sum::<f64>() / g as f64).collect();
    let mut sb0: Mat = (0..pb)
        .map(|i| {
            (0..pb)
                .map(|j| {
                    let c = ctx.clusters.iter().map(|c| (c.mean[bi[i]] - mb0[i]) * (c.mean[bi[j]] - mb0[j])).sum::<f64>() / g as f64;
                    let wpart = match (v.iter().position(|u| *u == bi[i]), v.iter().position(|u| *u == bi[j])) {
                        (Some(a), Some(c2)) => sw0[a][c2] / nbar,
                        _ => 0.0,
                    };
                    c - wpart
                })
                .collect()
        })
        .collect();
    for (i, row) in sb0.iter_mut().enumerate() {
        let floor = 0.05 * row[i].abs().max(1e-3);
        if row[i] < floor {
            row[i] = floor;
        }
    }
    if cholesky(&sb0).is_none() {
        sb0 = (0..pb).map(|i| (0..pb).map(|j| if i == j { sb0[i][i] } else { 0.0 }).collect()).collect();
    }
    let mv0: Vec<f64> = v.iter().map(|&u| ctx.clusters.iter().map(|c| c.mean[u] * c.n).sum::<f64>() / n).collect();

    // The saturated model.
    let both: Vec<bool> = w.obs.iter().map(|o| b.obs.contains(o)).collect();
    let mut h1 = Pair { w: open_model(&w.obs, &[], &both, false), b: open_model(&b.obs, &[], &vec![false; pb], false) };
    estimate::start(&mut h1.w, std::slice::from_ref(&sw0), std::slice::from_ref(&mv0));
    estimate::start(&mut h1.b, std::slice::from_ref(&sb0), std::slice::from_ref(&mb0));
    let Some(f1) = maximize(&h1, &ctx) else {
        fail(&mut bdl, "The saturated two-level model could not be estimated (a variable may have no variance within or between clusters).");
        return Ok(bdl);
    };
    set_theta(&mut h1, &f1.th);
    let (h1iw, h1ib, _, _, _) = moments(&h1, &ctx, &f1.th).expect("fitted");
    let h1w: Mat = (0..pv).map(|i| (0..pv).map(|j| h1iw.full[i][j]).collect()).collect();
    let h1b: Mat = (0..pb).map(|i| (0..pb).map(|j| h1ib.full[i][j]).collect()).collect();
    // Overall means of each level's variables (the within means of variables with a between part are 0).
    let mut mu_all = vec![0.0; vars.len()];
    for (i, &u) in v.iter().enumerate() {
        mu_all[u] += h1iw.mean[i];
    }
    for (i, &u) in bi.iter().enumerate() {
        mu_all[u] += h1ib.mean[i];
    }
    let h1mv: Vec<f64> = v.iter().map(|&u| mu_all[u]).collect();
    let h1mb: Vec<f64> = bi.iter().map(|&u| mu_all[u]).collect();

    // The model: fixed predictors at the saturated estimates; started from them.
    estimate::start(&mut w, std::slice::from_ref(&h1w), std::slice::from_ref(&h1mv));
    estimate::start(&mut b, std::slice::from_ref(&h1b), std::slice::from_ref(&h1mb));
    let mut pair = Pair { w, b };
    let Some(f) = maximize(&pair, &ctx) else {
        fail(&mut bdl, "The model could not be estimated: check that each level's model is identified.");
        return Ok(bdl);
    };
    set_theta(&mut pair, &f.th);
    let Some(h) = hessian(&pair, &ctx, &f.th) else {
        fail(&mut bdl, "The information matrix could not be computed.");
        return Ok(bdl);
    };
    let info: Mat = h.iter().map(|r| r.iter().map(|x| -x).collect()).collect();
    let Some(vc) = invert(&info).filter(|m| (0..m.len()).all(|i| m[i][i] > 0.0)) else {
        fail(&mut bdl, "The model is not identified: the information matrix is singular.");
        return Ok(bdl);
    };

    // Baseline: variances only (and the means), fixed predictors still fixed.
    let mut base = Pair { w: open_model(&pair.w.obs, &pair.w.xvars, &both, true), b: open_model(&pair.b.obs, &pair.b.xvars, &vec![false; pb], true) };
    estimate::start(&mut base.w, std::slice::from_ref(&h1w), std::slice::from_ref(&h1mv));
    estimate::start(&mut base.b, std::slice::from_ref(&h1b), std::slice::from_ref(&h1mb));
    let fb = maximize(&base, &ctx);

    // Fit.
    let q = pair.q() as f64;
    let (nxw, nxb) = (pair.w.xvars.len() as f64, pair.b.xvars.len() as f64);
    let (pvf, pbf) = (pv as f64, pb as f64);
    let moments_n = pvf * (pvf + 1.0) / 2.0 + pbf * (pbf + 1.0) / 2.0 + vars.len() as f64 - (nxw * (nxw + 1.0) / 2.0 + nxw) - (nxb * (nxb + 1.0) / 2.0 + nxb);
    let df = moments_n - q;
    let chisq = (2.0 * (f1.ll - f.ll)).max(0.0);
    let (bchisq, bdf) = match &fb {
        Some(fb) => ((2.0 * (f1.ll - fb.ll)).max(0.0), moments_n - base.q() as f64),
        None => (f64::NAN, f64::NAN),
    };
    // With fixed.x, lavaan reports log-likelihoods conditional on the fixed predictors: their own saturated
    // log-likelihood (within-only predictors over the cases, between-only ones over the clusters) is taken out.
    let mut ll_x = 0.0;
    for (model, h, count) in [(&pair.w, &h1w, n), (&pair.b, &h1b, g as f64)] {
        let x = &model.xvars;
        if !x.is_empty() {
            let sxx: Mat = x.iter().map(|&i| x.iter().map(|&j| h[i][j]).collect()).collect();
            let px = x.len() as f64;
            ll_x += -count / 2.0 * (px * LN_2PI + log_det(&sxx).unwrap_or(f64::NAN) + px);
        }
    }
    let (ll_model, ll_sat) = (f.ll - ll_x, f1.ll - ll_x);
    let rm = |l: f64| if df > 0.0 { (l / (n * df)).max(0.0).sqrt() } else { 0.0 };
    let srmr_part = |a: &Mat, m: &Mat| {
        let p = a.len();
        let (mut ss, mut c) = (0.0, 0.0);
        for i in 0..p {
            for j in 0..=i {
                ss += ((a[i][j] - m[i][j]) / (a[i][i] * a[j][j]).sqrt()).powi(2);
                c += 1.0;
            }
        }
        (ss / c).sqrt()
    };
    let (iw, ib, _, _, _) = moments(&pair, &ctx, &f.th).expect("fitted");
    let mw: Mat = (0..pv).map(|i| (0..pv).map(|j| iw.full[i][j]).collect()).collect();
    let mbm: Mat = (0..pb).map(|i| (0..pb).map(|j| ib.full[i][j]).collect()).collect();
    let (srmr_w, srmr_b) = (srmr_part(&h1w, &mw), srmr_part(&h1b, &mbm));
    for (k, val) in [
        ("fit.chisq", chisq),
        ("fit.df", df),
        ("fit.pvalue", if df > 0.0 { chisq_sf(chisq, df) } else { f64::NAN }),
        ("fit.baseline.chisq", bchisq),
        ("fit.baseline.df", bdf),
        ("fit.cfi", super::cfi_of(chisq, df, bchisq, bdf)),
        ("fit.tli", super::tli_of(chisq, df, bchisq, bdf)),
        ("fit.rmsea", rm((chisq - df).max(0.0))),
        ("fit.rmsea.lower", if df > 0.0 { rm(estimate::ncp_for(chisq, df, 0.95)) } else { 0.0 }),
        ("fit.rmsea.upper", if df > 0.0 { rm(estimate::ncp_for(chisq, df, 0.05)) } else { 0.0 }),
        ("fit.rmsea.pvalue", if df > 0.0 { 1.0 - estimate::pnchisq(chisq, df, 0.0025 * n * df) } else { f64::NAN }),
        ("fit.srmr", srmr_w + srmr_b),
        ("fit.srmr_within", srmr_w),
        ("fit.srmr_between", srmr_b),
        ("fit.logl", ll_model),
        ("fit.unrestricted.logl", ll_sat),
        ("fit.aic", -2.0 * ll_model + 2.0 * q),
        ("fit.bic", -2.0 * ll_model + q * n.ln()),
        ("npar", q),
    ] {
        set(&mut bdl, k, val);
    }
    let small = |x: f64| crate::apa::no_leading_zero(x, 3);
    let m = |k: &str| bdl.number(k).unwrap_or(f64::NAN);
    let fit_rows = vec![
        vec![text("χ²"), num(chisq, 2)],
        vec![text("df"), num(df, 0)],
        vec![text("p"), if df > 0.0 { common::p(m("fit.pvalue")) } else { text("") }],
        vec![text("CFI"), text(small(m("fit.cfi")))],
        vec![text("TLI"), text(small(m("fit.tli")))],
        vec![text("RMSEA [90% CI]"), text(format!("{} [{}, {}]", small(m("fit.rmsea")), small(m("fit.rmsea.lower")), small(m("fit.rmsea.upper"))))],
        vec![text("SRMR (within + between)"), text(format!("{} ({} + {})", small(srmr_w + srmr_b), small(srmr_w), small(srmr_b)))],
        vec![text("AIC"), num(m("fit.aic"), 2)],
        vec![text("BIC"), num(m("fit.bic"), 2)],
        vec![text("Log-likelihood"), num(ll_model, 2)],
        vec![text("Free parameters"), count(pair.q())],
        vec![text("Cases"), count(n as usize)],
        vec![text("Clusters"), count(g)],
    ];
    bdl.tables.push(Table { id: "fit".into(), title: "Model fit".into(), columns: vec!["Measure".into(), "Value".into()], rows: fit_rows, footnotes: vec![format!("Baseline model: χ²({}) = {}. Average cluster size {}.", crate::apa::num(bdf, 0), crate::apa::num(bchisq, 2), crate::apa::num(nbar, 1))] });

    // Parameters: each level's, with standardized values from that level's variances.
    let est_of = |pr: &Pair, th: &[f64]| -> Vec<f64> {
        let (tw, tb) = th.split_at(pr.w.nfree);
        let mut e = super::unstandardized(&pr.w, tw);
        e.extend(super::unstandardized(&pr.b, tb));
        e
    };
    let std_of = |pr: &Pair, th: &[f64]| -> Vec<f64> {
        let (tw, tb) = th.split_at(pr.w.nfree);
        let mut e = super::standardized(&pr.w, tw);
        e.extend(super::standardized(&pr.b, tb));
        e
    };
    let est = est_of(&pair, &f.th);
    let se = super::delta_se(&|t| est_of(&pair, t), &f.th, &vc);
    let std = std_of(&pair, &f.th);
    let std_se = super::delta_se(&|t| std_of(&pair, t), &f.th, &vc);
    let zq = norm_quantile(1.0 - (1.0 - spec.level) / 2.0);
    let all: Vec<(&str, &Param)> = pair.w.params.iter().map(|p| ("within", p)).chain(pair.b.params.iter().map(|p| ("between", p))).collect();
    let mut sections: Vec<(&str, &str, Vec<Vec<crate::result::Cell>>)> = vec![("loadings", "Factor loadings", vec![]), ("regressions", "Regressions", vec![]), ("covariances", "Covariances", vec![]), ("variances", "Variances", vec![]), ("intercepts", "Intercepts and means", vec![])];
    for (k, (lvl, p)) in all.iter().enumerate() {
        let key = format!("par.{}", k + 1);
        set_text(&mut bdl, format!("{key}.label"), p.name());
        set_text(&mut bdl, format!("{key}.group"), *lvl);
        set(&mut bdl, format!("{key}.est"), est[k]);
        set(&mut bdl, format!("{key}.std"), std[k]);
        let mut row = vec![text(p.name()), text(if *lvl == "within" { "Within" } else { "Between" }), num(est[k], 3)];
        if p.free.is_some() && se[k] > 0.0 {
            let zv = est[k] / se[k];
            let pv = norm_two_sided(zv);
            for (fname, val) in [("se", se[k]), ("z", zv), ("p", pv), ("lower", est[k] - zq * se[k]), ("upper", est[k] + zq * se[k])] {
                set(&mut bdl, format!("{key}.{fname}"), val);
            }
            row.extend([num(se[k], 3), num(zv, 2), common::p(pv), num(est[k] - zq * se[k], 3), num(est[k] + zq * se[k], 3)]);
        } else {
            row.extend([text(""), text(""), text(""), text(""), text("")]);
        }
        if std_se[k] > 1e-12 {
            set(&mut bdl, format!("{key}.stdse"), std_se[k]);
        }
        row.push(if std[k].is_finite() { num(std[k], 3) } else { text("") });
        let sec = match (p.op, p.i == p.j) {
            ("=~", _) => 0,
            ("~", _) => 1,
            ("~~", false) => 2,
            ("~~", true) => 3,
            _ => 4,
        };
        sections[sec].2.push(row);
    }
    set(&mut bdl, "par.count", all.len() as f64);
    for (id, title, rows) in sections {
        if !rows.is_empty() {
            bdl.tables.push(Table { id: id.into(), title: title.into(), columns: vec!["Parameter".into(), "Level".into(), "Estimate".into(), "SE".into(), "z".into(), "p".into(), format!("{lv} CI lower"), format!("{lv} CI upper"), "Standardized".into()], rows, footnotes: Vec::new() });
        }
    }
    // R² of each level's endogenous variables.
    let mut r2_rows = Vec::new();
    for (lvl, model, imp) in [("within", &pair.w, &iw), ("between", &pair.b, &ib)] {
        for i in 0..model.t() {
            if model.params.iter().any(|p| p.mat == Matrix::A && p.i == i) {
                let r2 = 1.0 - imp.s[i][i] / imp.full[i][i];
                set(&mut bdl, format!("r2.{lvl}:{}", model.name_of(i)), r2);
                r2_rows.push(vec![text(model.name_of(i)), text(if lvl == "within" { "Within" } else { "Between" }), num(r2, 3)]);
            }
        }
    }
    if !r2_rows.is_empty() {
        bdl.tables.push(Table { id: "r2".into(), title: "Variance explained (R²)".into(), columns: vec!["Variable".into(), "Level".into(), "R²".into()], rows: r2_rows, footnotes: Vec::new() });
    }
    // Intraclass correlations of the variables with both parts, from the saturated model.
    let mut icc_rows = Vec::new();
    for (i, &u) in v.iter().enumerate() {
        if let Some(j) = bi.iter().position(|x| *x == u) {
            let icc = h1b[j][j] / (h1b[j][j] + h1w[i][i]);
            set(&mut bdl, format!("icc.{}", vars[u]), icc);
            icc_rows.push(vec![text(vars[u].clone()), num(icc, 3)]);
        }
    }
    if !icc_rows.is_empty() {
        bdl.tables.push(Table { id: "icc".into(), title: "Intraclass correlations".into(), columns: vec!["Variable".into(), "ICC".into()], rows: icc_rows, footnotes: vec!["The share of each variable's variance between clusters, from the saturated two-level model.".into()] });
    }
    // A diagram of each level.
    for (lvl, title) in [(0usize, "Path diagram: within clusters (level 1)"), (1, "Path diagram: between clusters (level 2)")] {
        let text_level: String = spec_level_text(&spec.model, lvl + 1);
        if let Ok(mut d) = super::diagram::structure(&text_level) {
            super::diagram::layout(&mut d, &spec.layout);
            let mut labels = std::collections::BTreeMap::new();
            for e in &d.edges {
                let key = e.param();
                if let Some(k) = all.iter().position(|(l, p)| *l == if lvl == 0 { "within" } else { "between" } && p.name() == key) {
                    labels.insert(key, crate::apa::no_leading_zero(std[k], 2));
                }
            }
            let what = if lvl == 0 { "within-cluster (level 1) model" } else { "between-cluster (level 2) model" };
            let alt = super::diagram::alt_text(&d, &labels, what);
            bdl.charts.push(crate::chart::Chart { id: format!("diagram{}", lvl + 1), title: title.into(), svg: super::diagram::svg(&d, &labels, &alt), alt });
        }
    }
    bdl.diagnostics.push(if f.converged {
        Diagnostic { id: "convergence".into(), title: "Convergence".into(), state: DiagnosticState::NoConcernDetected, message: format!("The estimates converged in {} iterations (the saturated model in {}).", f.iterations, f1.iterations) }
    } else {
        Diagnostic { id: "convergence".into(), title: "Convergence".into(), state: DiagnosticState::ReviewNeeded, message: "The estimation stopped before converging; the estimates may not be the maximum likelihood ones.".into() }
    });
    if varying_z {
        bdl.diagnostics.push(Diagnostic { id: "between".into(), title: "Between-cluster variables".into(), state: DiagnosticState::ReviewNeeded, message: "A variable named only at level 2 varies within some clusters; its cluster means were used.".into() });
    }
    let negative: Vec<String> = all.iter().filter(|(_, p)| p.mat == Matrix::S && p.i == p.j && p.free.is_some() && p.value < 0.0).map(|(l, p)| format!("{} ({l})", p.lhs)).collect();
    if !negative.is_empty() {
        bdl.diagnostics.push(Diagnostic { id: "heywood".into(), title: "Heywood cases".into(), state: DiagnosticState::ReviewNeeded, message: format!("Negative variance estimates: {}.", negative.join(", ")) });
    }
    bdl.diagnostics.push(common::size_diagnostic(g, format!("{g} clusters, {} cases", n as usize)));
    Ok(bdl)
}

/// The statements of one level, as text.
fn spec_level_text(text: &str, level: usize) -> String {
    let mut out = Vec::new();
    let mut current = 0;
    for line in text.lines() {
        let t = line.split(['#', '!']).next().unwrap_or("").trim();
        if let Some(rest) = t.strip_prefix("level") {
            if let Some(v) = rest.trim().strip_prefix(':') {
                current = match v.trim() {
                    "1" | "within" => 1,
                    "2" | "between" => 2,
                    _ => 0,
                };
                continue;
            }
        }
        if current == level {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The analytic gradient agrees with central differences of the log-likelihood.
    #[test]
    fn gradient_matches_finite_differences() {
        let obs_w = vec!["y".to_string(), "x".to_string()];
        let obs_b = vec!["y".to_string(), "w".to_string()];
        let mut pair = Pair { w: open_model(&obs_w, &[], &[true, false], false), b: open_model(&obs_b, &[], &[false, false], false) };
        estimate::start(&mut pair.w, &[vec![vec![1.0, 0.3], vec![0.3, 1.2]]], &[vec![0.0, 0.1]]);
        estimate::start(&mut pair.b, &[vec![vec![0.5, 0.1], vec![0.1, 0.8]]], &[vec![0.2, -0.1]]);
        let mut apos = vec![None; 3];
        apos[0] = Some(0);
        apos[1] = Some(1);
        apos[2] = Some(2);
        let clusters = vec![Cluster { n: 3.0, mean: vec![0.5, -0.2, 1.0] }, Cluster { n: 5.0, mean: vec![-0.4, 0.3, -0.7] }, Cluster { n: 3.0, mean: vec![0.1, 0.0, 0.2] }];
        let ctx = Ctx { v: vec![0, 1], b: vec![0, 2], z: vec![2], apos, clusters, scatter: vec![vec![9.0, 1.5], vec![1.5, 11.0]], n: 11.0 };
        let th = pair.theta();
        let (_, g) = loglik(&pair, &ctx, &th, true).unwrap();
        for k in 0..th.len() {
            let h = 1e-6;
            let (mut up, mut dn) = (th.clone(), th.clone());
            up[k] += h;
            dn[k] -= h;
            let num = (loglik(&pair, &ctx, &up, false).unwrap().0 - loglik(&pair, &ctx, &dn, false).unwrap().0) / (2.0 * h);
            assert!((num - g[k]).abs() < 1e-6 * num.abs().max(1.0), "parameter {k}: {num} vs {}", g[k]);
        }
    }
}
