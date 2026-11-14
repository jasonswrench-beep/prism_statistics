//! Designed experiments (checklist D-4, part 2): generating designs as R's FrF2 and rsm build them.
//!
//! * Full factorial designs with any number of levels per factor (the first factor changing fastest, as
//!   DoE.base's fac.design).
//! * Two-level fractional factorials 2^(k−p) of 4 to 64 runs: the minimum-aberration design for the number of runs,
//!   with FrF2's catalog generators (Yates column numbers of the base factors), in standard order; the defining
//!   relation, resolution, word-length pattern and alias structure.
//! * Central composite designs (rsm's ccd: a full two-level cube with its center points, then an axial block with
//!   its own; α orthogonal for the blocks (rsm's default), rotatable, or face-centered).
//! * Box-Behnken designs for 3 to 5 factors (rsm's bbd, blocked for 4 and 5).
//! * Plackett-Burman screening designs of 12, 20, 24, 36, 44 and 48 runs (FrF2's pb: the cyclic generator rows and a
//!   final row of −1s, the first k columns).
//! * Mixture designs: the simplex lattice {q, m} (in AlgDesign's gen.mixture order) and the simplex centroid, either
//!   optionally augmented with the overall centroid and the axial check blends.
//! * D-optimal designs: Federov's exchange algorithm over the full grid of candidate settings, from many random
//!   starts, for a linear, two-factor-interaction or quadratic model; D, A and G criteria as AlgDesign reports them.
//!   I-optimal designs minimize AlgDesign's I criterion, the average prediction variance over the candidate grid
//!   (trace of (X'X/N)⁻¹ times the candidates' moment matrix), by the same exchange with rank-one updates.
//! * Constrained mixtures: with lower bounds only, lattice and centroid designs are built in pseudo-components and
//!   mapped back (xᵢ = Lᵢ + (1 − ΣL)x'ᵢ). With upper bounds, the extreme-vertices design (McLean and Anderson): every
//!   vertex of the region {Σx = 1, L ≤ x ≤ U}, optionally with the midpoints of its edges and its centroid.
//!
//! Factors run from their low to high setting (coded −1 to +1, or 1..L for full factorials); runs can be replicated,
//! randomized (seeded) and given center points.

#![allow(clippy::needless_range_loop)]

use prismqda_stats::rng::SplitMix64;
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{ResultBundle, Table};

pub const DESIGN: &str = "doe.design";
pub const METHOD_VERSION: u32 = 1;

/// FrF2's minimum-aberration designs: (factors, runs, generator columns).
/// FrF2's Plackett-Burman generators (the first row; the others are its cyclic shifts).
fn pb_generator(runs: usize) -> Option<Vec<f64>> {
    let g: &str = match runs {
        12 => "++-+++---+-",
        20 => "++--++++-+-+----++-",
        24 => "+++++-+-++--++--+-+----",
        36 => "-+-+++---+++++-+++--+----+-+-++--+-",
        44 => "++--+-+--+++-+++++---+-+++-----+---++-+-++-",
        48 => "+++++-++++--+-+-+++--+--++-++---+-+-++----+----",
        _ => return None,
    };
    Some(g.chars().map(|c| if c == '+' { 1.0 } else { -1.0 }).collect())
}

const CATALOG: [(usize, usize, &[usize]); 35] = [
    (3, 4, &[3]),
    (4, 8, &[7]),
    (5, 8, &[3, 5]),
    (6, 8, &[3, 5, 6]),
    (7, 8, &[3, 5, 6, 7]),
    (5, 16, &[15]),
    (6, 16, &[7, 11]),
    (7, 16, &[7, 11, 13]),
    (8, 16, &[7, 11, 13, 14]),
    (9, 16, &[3, 5, 9, 14, 15]),
    (10, 16, &[3, 5, 6, 9, 14, 15]),
    (11, 16, &[3, 5, 6, 9, 10, 13, 14]),
    (12, 16, &[3, 5, 6, 9, 10, 13, 14, 15]),
    (13, 16, &[3, 5, 6, 7, 9, 10, 11, 12, 13]),
    (14, 16, &[3, 5, 6, 7, 9, 10, 11, 12, 13, 14]),
    (15, 16, &[3, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15]),
    (6, 32, &[31]),
    (7, 32, &[7, 27]),
    (8, 32, &[7, 11, 29]),
    (9, 32, &[7, 11, 19, 29]),
    (10, 32, &[7, 11, 19, 29, 30]),
    (11, 32, &[7, 11, 13, 19, 21, 25]),
    (12, 32, &[7, 11, 13, 14, 19, 21, 25]),
    (13, 32, &[7, 11, 13, 14, 19, 21, 22, 25]),
    (14, 32, &[7, 11, 13, 14, 19, 21, 22, 25, 26]),
    (15, 32, &[7, 11, 13, 14, 19, 21, 22, 25, 26, 28]),
    (7, 64, &[63]),
    (8, 64, &[15, 51]),
    (9, 64, &[7, 27, 45]),
    (10, 64, &[7, 27, 43, 53]),
    (11, 64, &[7, 11, 29, 45, 51]),
    (12, 64, &[7, 11, 29, 45, 51, 62]),
    (13, 64, &[7, 11, 21, 25, 38, 58, 60]),
    (14, 64, &[7, 11, 19, 30, 37, 41, 49, 60]),
    (15, 64, &[7, 11, 19, 30, 37, 41, 49, 60, 63]),
];

/// rsm's Box-Behnken blocks: for each block, the pairs of factors varied together (1-based).
fn bbd_blocks(k: usize) -> Option<Vec<Vec<(usize, usize)>>> {
    match k {
        3 => Some(vec![vec![(1, 2), (1, 3), (2, 3)]]),
        4 => Some(vec![vec![(1, 2), (3, 4)], vec![(1, 4), (2, 3)], vec![(1, 3), (2, 4)]]),
        5 => Some(vec![vec![(1, 2), (1, 3), (3, 4), (4, 5), (2, 5)], vec![(1, 4), (1, 5), (2, 3), (2, 4), (3, 5)]]),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Full,
    #[default]
    Fractional,
    Ccd,
    Bbd,
    /// Plackett-Burman screening design.
    Pb,
    /// Simplex-lattice mixture design.
    Lattice,
    /// Simplex-centroid mixture design.
    Centroid,
    /// D-optimal design from a candidate grid.
    Optimal,
    /// Extreme vertices of a constrained mixture region.
    Vertices,
}

/// The optimality criterion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Criterion {
    #[default]
    D,
    I,
}

/// The model a D-optimal design is built for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OptModel {
    Linear,
    Interactions,
    #[default]
    Quadratic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Alpha {
    #[default]
    Orthogonal,
    Rotatable,
    Face,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Factor {
    pub name: String,
    #[serde(default)]
    pub low: Option<f64>,
    #[serde(default)]
    pub high: Option<f64>,
    /// Full factorial: the number of levels (2 when left out).
    #[serde(default)]
    pub levels: Option<usize>,
}

fn one() -> usize {
    1
}
fn seed() -> u64 {
    20_260_101
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    #[serde(default)]
    pub kind: Kind,
    pub factors: Vec<Factor>,
    /// Fractional factorial: the number of runs (4 to 64, a power of two).
    #[serde(default)]
    pub runs: Option<usize>,
    /// Center points (CCD: in the cube block; Box-Behnken: per block; factorials: added at the end).
    #[serde(default)]
    pub center: usize,
    /// CCD: center points in the axial block.
    #[serde(default)]
    pub center_axial: Option<usize>,
    #[serde(default)]
    pub alpha: Alpha,
    #[serde(default = "one")]
    pub replicates: usize,
    #[serde(default)]
    pub randomize: bool,
    #[serde(default = "seed")]
    pub seed: u64,
    /// The response column added (empty) to the design.
    #[serde(default)]
    pub response: Option<String>,
    /// Simplex lattice: the degree m (proportions in steps of 1/m; 2 when left out).
    #[serde(default)]
    pub degree: Option<usize>,
    /// Mixture designs: add the overall centroid and the axial check blends.
    #[serde(default)]
    pub augment: bool,
    /// D-optimal designs: the model.
    #[serde(default)]
    pub model: OptModel,
    /// Optimal designs: D (the default) or I.
    #[serde(default)]
    pub criterion: Criterion,
}

/// A generated design.
pub struct Design {
    pub names: Vec<String>,
    /// Coded settings per run, in standard order.
    pub coded: Vec<Vec<f64>>,
    pub blocks: Option<Vec<usize>>,
    /// The order in which to run them (1-based positions of the standard-order runs).
    pub run_order: Vec<usize>,
    pub alpha: Option<f64>,
    /// Fractional: generators ("D = AB"), defining words, resolution, word-length pattern, aliases.
    pub generators: Vec<String>,
    pub words: Vec<String>,
    pub resolution: Option<usize>,
    pub wlp: Vec<usize>,
    pub alias_main: Vec<String>,
    pub alias_2fi: Vec<String>,
    /// D-optimal designs: the D, A and G criteria and the number of model terms.
    pub criteria: Option<Criteria>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Criteria {
    /// det(X'X/N)^(1/p).
    pub d: f64,
    /// trace((X'X/N)⁻¹)/p.
    pub a: f64,
    /// G-efficiency: p / the largest scaled prediction variance over the candidates.
    pub g: f64,
    /// AlgDesign's I: the average scaled prediction variance over the candidates.
    pub i: f64,
    pub terms: usize,
    pub candidates: usize,
}

/// The model-matrix row of coded settings x: 1, linear terms, then two-factor interactions, then squares.
pub fn model_row(model: OptModel, x: &[f64]) -> Vec<f64> {
    let k = x.len();
    let mut r = vec![1.0];
    r.extend_from_slice(x);
    if model != OptModel::Linear {
        for i in 0..k {
            for j in i + 1..k {
                r.push(x[i] * x[j]);
            }
        }
    }
    if model == OptModel::Quadratic {
        r.extend(x.iter().map(|v| v * v));
    }
    r
}

fn xtx(rows: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let p = rows.first().map_or(0, Vec::len);
    let mut m = vec![vec![0.0; p]; p];
    for r in rows {
        for a in 0..p {
            for b in 0..p {
                m[a][b] += r[a] * r[b];
            }
        }
    }
    m
}

/// AlgDesign's criteria for a design (model-matrix rows) against its candidate set.
pub fn criteria(design: &[Vec<f64>], candidates: &[Vec<f64>]) -> Option<Criteria> {
    let n = design.len() as f64;
    let p = design.first()?.len();
    let mut m = xtx(design);
    m.iter_mut().for_each(|r| r.iter_mut().for_each(|v| *v /= n));
    let det = super::linalg::det(&m);
    let inv = super::linalg::invert(&m)?;
    if det.is_nan() || det <= 0.0 {
        return None;
    }
    let quad = |x: &[f64]| (0..p).map(|a| x[a] * (0..p).map(|b| inv[a][b] * x[b]).sum::<f64>()).sum::<f64>();
    let dmax = candidates.iter().map(|c| quad(c)).fold(0.0, f64::max);
    let i = candidates.iter().map(|c| quad(c)).sum::<f64>() / candidates.len() as f64;
    Some(Criteria { d: det.powf(1.0 / p as f64), a: super::linalg::trace(&inv) / p as f64, g: p as f64 / dmax, i, terms: p, candidates: candidates.len() })
}

/// I-optimal exchange: from `starts` random starts, the N candidate rows minimizing trace((X'X)⁻¹ B), B the
/// candidates' moment matrix, swapping one design row for one candidate at a time (rank-one updates).
fn i_exchange(cand: &[Vec<f64>], n: usize, starts: usize, rng: &mut SplitMix64) -> Option<Vec<usize>> {
    let p = cand[0].len();
    let c = cand.len();
    let mut bm = xtx(cand);
    bm.iter_mut().for_each(|r| r.iter_mut().for_each(|v| *v /= c as f64));
    let crit = |set: &[usize]| -> Option<f64> {
        let rows: Vec<Vec<f64>> = set.iter().map(|&i| cand[i].clone()).collect();
        let inv = super::linalg::invert(&xtx(&rows))?;
        if super::linalg::det(&xtx(&rows)) <= 0.0 {
            return None;
        }
        Some((0..p).map(|a| (0..p).map(|b| inv[a][b] * bm[b][a]).sum::<f64>()).sum())
    };
    // D-optimal starts are non-singular; then improve I.
    let mut best: Option<(f64, Vec<usize>)> = None;
    for _ in 0..starts {
        let Some(mut set) = federov(cand, n, 1, rng) else { continue };
        let Some(mut cur) = crit(&set) else { continue };
        for _ in 0..1000 {
            let rows: Vec<Vec<f64>> = set.iter().map(|&i| cand[i].clone()).collect();
            let Some(a) = super::linalg::invert(&xtx(&rows)) else { break };
            let u: Vec<Vec<f64>> = cand.iter().map(|x| (0..p).map(|r| (0..p).map(|q| a[r][q] * x[q]).sum()).collect()).collect();
            let quad_b = |v: &[f64]| -> f64 { (0..p).map(|r| v[r] * (0..p).map(|q| bm[r][q] * v[q]).sum::<f64>()).sum() };
            let mut gain = 1e-12 * cur.abs().max(1e-300);
            let mut swap = None;
            let mut tried: Vec<usize> = Vec::new();
            for (pos, &i) in set.iter().enumerate() {
                if tried.contains(&i) {
                    continue;
                }
                tried.push(i);
                let di: f64 = cand[i].iter().zip(&u[i]).map(|(x, y)| x * y).sum();
                if di >= 1.0 - 1e-12 {
                    continue;
                }
                // Removing x_i: trace rises by u_i'Bu_i / (1 − d_i).
                let t1 = cur + quad_b(&u[i]) / (1.0 - di);
                for j in 0..c {
                    if j == i {
                        continue;
                    }
                    let dij: f64 = cand[i].iter().zip(&u[j]).map(|(x, y)| x * y).sum();
                    let dj: f64 = cand[j].iter().zip(&u[j]).map(|(x, y)| x * y).sum();
                    // A₁x_j = u_j + u_i d_ij / (1 − d_i); x_j'A₁x_j = d_j + d_ij² / (1 − d_i).
                    let f = dij / (1.0 - di);
                    let v: Vec<f64> = (0..p).map(|r| u[j][r] + u[i][r] * f).collect();
                    let dj1 = dj + dij * f;
                    let t2 = t1 - quad_b(&v) / (1.0 + dj1);
                    if cur - t2 > gain {
                        gain = cur - t2;
                        swap = Some((pos, j));
                    }
                }
            }
            let Some((pos, j)) = swap else { break };
            set[pos] = j;
            match crit(&set) {
                Some(v) => cur = v,
                None => break,
            }
        }
        if best.as_ref().is_none_or(|(b, _)| cur < *b * (1.0 - 1e-12)) {
            best = Some((cur, set));
        }
    }
    best.map(|(_, mut s)| {
        s.sort_unstable();
        s
    })
}

/// The vertices of {Σx = 1, L ≤ x ≤ U} (McLean and Anderson's extreme vertices): every component but one at a bound,
/// the last from the sum, kept when within its own bounds.
pub fn extreme_vertices(lo: &[f64], hi: &[f64]) -> Vec<Vec<f64>> {
    let q = lo.len();
    let mut out: Vec<Vec<f64>> = Vec::new();
    for free in 0..q {
        for mask in 0..(1u32 << (q - 1)) {
            let mut x = vec![0.0; q];
            let mut bit = 0;
            for j in 0..q {
                if j == free {
                    continue;
                }
                x[j] = if mask & (1 << bit) != 0 { hi[j] } else { lo[j] };
                bit += 1;
            }
            let rest: f64 = x.iter().sum();
            x[free] = 1.0 - rest;
            if x[free] >= lo[free] - 1e-12 && x[free] <= hi[free] + 1e-12 {
                x[free] = x[free].clamp(lo[free], hi[free]);
                if !out.iter().any(|v| v.iter().zip(&x).all(|(a, b)| (a - b).abs() < 1e-10)) {
                    out.push(x);
                }
            }
        }
    }
    out
}

/// The midpoints of the region's edges: pairs of vertices whose shared active bounds no other vertex also has, and
/// at least q − 2 of them.
pub fn edge_centroids(v: &[Vec<f64>], lo: &[f64], hi: &[f64]) -> Vec<Vec<f64>> {
    let q = lo.len();
    let active = |x: &[f64]| -> Vec<(usize, bool)> {
        let mut a = Vec::new();
        for j in 0..q {
            if (x[j] - lo[j]).abs() < 1e-10 {
                a.push((j, false));
            }
            if (x[j] - hi[j]).abs() < 1e-10 && (hi[j] - lo[j]).abs() >= 1e-10 {
                a.push((j, true));
            }
        }
        a
    };
    let acts: Vec<Vec<(usize, bool)>> = v.iter().map(|x| active(x)).collect();
    let mut out = Vec::new();
    for i in 0..v.len() {
        for j in i + 1..v.len() {
            let common: Vec<(usize, bool)> = acts[i].iter().filter(|a| acts[j].contains(a)).copied().collect();
            if common.len() + 2 < q {
                continue;
            }
            let other = (0..v.len()).any(|k| k != i && k != j && common.iter().all(|c| acts[k].contains(c)));
            if !other {
                out.push((0..q).map(|t| (v[i][t] + v[j][t]) / 2.0).collect());
            }
        }
    }
    out
}

/// Federov's exchange: from `starts` random starts, the N candidate rows (repeats allowed) with the largest
/// det(X'X). Returns the chosen candidate indices, sorted.
fn federov(cand: &[Vec<f64>], n: usize, starts: usize, rng: &mut SplitMix64) -> Option<Vec<usize>> {
    let p = cand[0].len();
    let c = cand.len();
    let mut best: Option<(f64, Vec<usize>)> = None;
    for _ in 0..starts {
        let mut set: Vec<usize> = (0..n).map(|_| rng.below(c)).collect();
        for _ in 0..1000 {
            let rows: Vec<Vec<f64>> = set.iter().map(|&i| cand[i].clone()).collect();
            let mut m = xtx(&rows);
            // A small ridge lets a singular start improve until it is estimable.
            let scale = (0..p).map(|a| m[a][a]).fold(0.0, f64::max).max(1.0);
            for a in 0..p {
                m[a][a] += 1e-7 * scale;
            }
            let Some(inv) = super::linalg::invert(&m) else { break };
            let mx: Vec<Vec<f64>> = cand.iter().map(|x| (0..p).map(|a| (0..p).map(|b| inv[a][b] * x[b]).sum()).collect()).collect();
            let dj: Vec<f64> = (0..c).map(|j| cand[j].iter().zip(&mx[j]).map(|(a, b)| a * b).sum()).collect();
            let (mut gain, mut swap) = (1e-10, None);
            let mut tried: Vec<usize> = Vec::new();
            for (pos, &i) in set.iter().enumerate() {
                if tried.contains(&i) {
                    continue;
                }
                tried.push(i);
                for j in 0..c {
                    if j == i {
                        continue;
                    }
                    let dij: f64 = cand[j].iter().zip(&mx[i]).map(|(a, b)| a * b).sum();
                    let delta = dj[j] - dj[i] - dj[i] * dj[j] + dij * dij;
                    if delta > gain {
                        gain = delta;
                        swap = Some((pos, j));
                    }
                }
            }
            match swap {
                Some((pos, j)) => set[pos] = j,
                None => break,
            }
        }
        let rows: Vec<Vec<f64>> = set.iter().map(|&i| cand[i].clone()).collect();
        let det = super::linalg::det(&xtx(&rows));
        if det > 0.0 && best.as_ref().is_none_or(|(b, _)| det > *b * (1.0 + 1e-12)) {
            best = Some((det, set));
        }
    }
    best.map(|(_, mut s)| {
        s.sort_unstable();
        s
    })
}

/// FrF2's default factor names: A to Z without I.
fn letter(i: usize) -> String {
    const L: &str = "ABCDEFGHJKLMNOPQRSTUVWXYZ";
    L.chars().nth(i).map_or(format!("X{}", i + 1), |c| c.to_string())
}

/// The word of a set of factors (bit mask) in letters.
fn word(mask: u32, names: &[String]) -> String {
    (0..names.len()).filter(|&i| mask & (1 << i) != 0).map(|i| names[i].clone()).collect::<Vec<_>>().join("")
}

pub fn build(spec: &Spec) -> Result<Design> {
    let k = spec.factors.len();
    if k < 1 {
        return Err(Error::Invalid("Name at least one factor.".into()));
    }
    let names: Vec<String> = spec.factors.iter().enumerate().map(|(i, f)| if f.name.trim().is_empty() { letter(i) } else { f.name.trim().to_string() }).collect();
    let mut coded: Vec<Vec<f64>> = Vec::new();
    let mut blocks: Option<Vec<usize>> = None;
    let mut d = Design { names: names.clone(), coded: Vec::new(), blocks: None, run_order: Vec::new(), alpha: None, generators: Vec::new(), words: Vec::new(), resolution: None, wlp: Vec::new(), alias_main: Vec::new(), alias_2fi: Vec::new(), criteria: None };
    match spec.kind {
        Kind::Full => {
            let levels: Vec<usize> = spec.factors.iter().map(|f| f.levels.unwrap_or(2).max(2)).collect();
            let total: usize = levels.iter().product();
            if total > 100_000 {
                return Err(Error::Invalid("That full factorial has too many runs.".into()));
            }
            for r in 0..total {
                let mut rest = r;
                let mut row = Vec::with_capacity(k);
                for &l in &levels {
                    let lv = rest % l;
                    rest /= l;
                    row.push(if l == 2 { [-1.0, 1.0][lv] } else { (lv + 1) as f64 });
                }
                coded.push(row);
            }
        }
        Kind::Fractional => {
            let runs = spec.runs.unwrap_or(0);
            let q = runs.trailing_zeros() as usize;
            if !runs.is_power_of_two() || !(4..=64).contains(&runs) {
                return Err(Error::Invalid("A fractional factorial needs 4, 8, 16, 32 or 64 runs.".into()));
            }
            let gens: Vec<usize> = if k == q {
                Vec::new()
            } else if k < q {
                return Err(Error::Invalid(format!("{k} factors need no more than {} runs; use a full factorial.", 1usize << k)));
            } else {
                CATALOG.iter().find(|(f, r, _)| *f == k && *r == runs).map(|(_, _, g)| g.to_vec()).ok_or_else(|| Error::Invalid(format!("No catalog design has {k} factors in {runs} runs (up to {} factors fit in {runs} runs, and at most 15 factors).", runs - 1)))?
            };
            for r in 0..runs {
                let base: Vec<f64> = (0..q).map(|j| if r & (1 << j) != 0 { 1.0 } else { -1.0 }).collect();
                let mut row = base.clone();
                for &g in &gens {
                    row.push((0..q).filter(|&j| g & (1 << j) != 0).map(|j| base[j]).product());
                }
                coded.push(row);
            }
            // Defining relation: each generator's word, and every product of them.
            let gen_words: Vec<u32> = gens.iter().enumerate().map(|(i, &g)| (g as u32) | (1 << (q + i))).collect();
            for (i, &g) in gens.iter().enumerate() {
                d.generators.push(format!("{} = {}", names[q + i], word(g as u32, &names[..q])));
            }
            let mut group: Vec<u32> = Vec::new();
            for subset in 1u32..(1 << gen_words.len()) {
                let w = (0..gen_words.len()).filter(|&i| subset & (1 << i) != 0).fold(0u32, |a, i| a ^ gen_words[i]);
                group.push(w);
            }
            group.sort_by_key(|w| (w.count_ones(), *w));
            d.words = group.iter().map(|w| word(*w, &names)).collect();
            // Word lengths 3 to 7 at least (as FrF2's catalog reports them).
            let mut wlp = vec![0usize; k.max(7) + 1];
            for w in &group {
                wlp[w.count_ones() as usize] += 1;
            }
            d.resolution = group.iter().map(|w| w.count_ones() as usize).min();
            d.wlp = wlp[3..].to_vec();
            // Aliases among main effects and two-factor interactions.
            let low = |m: u32| m.count_ones() <= 2;
            for i in 0..k {
                let m = 1u32 << i;
                let al: Vec<u32> = group.iter().map(|w| w ^ m).filter(|a| low(*a)).collect();
                if !al.is_empty() {
                    let mut parts: Vec<String> = al.iter().map(|a| word(*a, &names)).collect();
                    parts.sort();
                    d.alias_main.push(format!("{}={}", names[i], parts.join("=")));
                }
            }
            let mut seen: Vec<u32> = Vec::new();
            for i in 0..k {
                for j in i + 1..k {
                    let m = (1u32 << i) | (1 << j);
                    if seen.contains(&m) {
                        continue;
                    }
                    let mut chain: Vec<u32> = vec![m];
                    chain.extend(group.iter().map(|w| w ^ m).filter(|a| a.count_ones() == 2));
                    // Chains with a main effect are reported with the main effects.
                    if group.iter().any(|w| (w ^ m).count_ones() == 1) {
                        seen.extend(chain);
                        continue;
                    }
                    if chain.len() > 1 {
                        let mut parts: Vec<String> = chain.iter().map(|a| word(*a, &names)).collect();
                        parts.sort();
                        d.alias_2fi.push(parts.join("="));
                    }
                    seen.extend(chain);
                }
            }
            d.alias_2fi.sort();
        }
        Kind::Ccd => {
            if !(2..=6).contains(&k) {
                return Err(Error::Invalid("Central composite designs here have 2 to 6 factors.".into()));
            }
            let f = 1usize << k;
            let n0c = spec.center;
            let n0s = spec.center_axial.unwrap_or(spec.center);
            let alpha = match spec.alpha {
                Alpha::Face => 1.0,
                Alpha::Rotatable => (f as f64).powf(0.25),
                Alpha::Orthogonal => ((k as f64) * (1.0 + n0s as f64 / (2.0 * k as f64)) / (1.0 + n0c as f64 / f as f64)).sqrt(),
            };
            d.alpha = Some(alpha);
            let mut bl = Vec::new();
            for r in 0..f {
                coded.push((0..k).map(|j| if r & (1 << j) != 0 { 1.0 } else { -1.0 }).collect());
                bl.push(1);
            }
            for _ in 0..n0c {
                coded.push(vec![0.0; k]);
                bl.push(1);
            }
            for j in 0..k {
                for s in [-1.0, 1.0] {
                    let mut row = vec![0.0; k];
                    row[j] = s * alpha;
                    coded.push(row);
                    bl.push(2);
                }
            }
            for _ in 0..n0s {
                coded.push(vec![0.0; k]);
                bl.push(2);
            }
            blocks = Some(bl);
        }
        Kind::Bbd => {
            let Some(bs) = bbd_blocks(k) else {
                return Err(Error::Invalid("Box-Behnken designs here have 3, 4 or 5 factors.".into()));
            };
            let mut bl = Vec::new();
            for (b, pairs) in bs.iter().enumerate() {
                for &(i, j) in pairs {
                    for r in 0..4 {
                        let mut row = vec![0.0; k];
                        row[i - 1] = if r & 1 != 0 { 1.0 } else { -1.0 };
                        row[j - 1] = if r & 2 != 0 { 1.0 } else { -1.0 };
                        coded.push(row);
                        bl.push(b + 1);
                    }
                }
                for _ in 0..spec.center {
                    coded.push(vec![0.0; k]);
                    bl.push(b + 1);
                }
            }
            if bs.len() > 1 {
                blocks = Some(bl);
            }
        }
        Kind::Pb => {
            let runs = spec.runs.unwrap_or(12);
            let Some(gen) = pb_generator(runs) else {
                return Err(Error::Invalid("A Plackett-Burman design here has 12, 20, 24, 36, 44 or 48 runs (for 8, 16, 32 or 64 runs, use a fractional factorial).".into()));
            };
            let len = runs - 1;
            if k > len {
                return Err(Error::Invalid(format!("A {runs}-run Plackett-Burman design has room for at most {len} factors.")));
            }
            for i in 0..len {
                coded.push((0..k).map(|j| gen[(j + len - i) % len]).collect());
            }
            coded.push(vec![-1.0; k]);
        }
        Kind::Lattice | Kind::Centroid => {
            if !(2..=10).contains(&k) {
                return Err(Error::Invalid("A mixture design here has 2 to 10 components.".into()));
            }
            if spec.kind == Kind::Lattice {
                let m = spec.degree.unwrap_or(2);
                if !(1..=6).contains(&m) {
                    return Err(Error::Invalid("The lattice degree is 1 to 6.".into()));
                }
                // Every composition of m into k parts, ordered as AlgDesign's gen.mixture (the last component slowest).
                let mut comps: Vec<Vec<usize>> = Vec::new();
                let mut cur = vec![0usize; k];
                fn rec(j: usize, left: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
                    if j == 0 {
                        cur[0] = left;
                        out.push(cur.clone());
                        return;
                    }
                    for a in 0..=left {
                        cur[j] = a;
                        rec(j - 1, left - a, cur, out);
                    }
                }
                rec(k - 1, m, &mut cur, &mut comps);
                comps.sort_by(|a, b| a[1..].iter().rev().cmp(b[1..].iter().rev()));
                coded = comps.iter().map(|c| c.iter().map(|&a| a as f64 / m as f64).collect()).collect();
                if coded.len() > 5000 {
                    return Err(Error::Invalid("That lattice has too many blends.".into()));
                }
            } else {
                // Pure components, then the binary blends, then the ternary blends, ..., then the overall centroid.
                for size in 1..=k {
                    let mut idx: Vec<usize> = (0..size).collect();
                    loop {
                        let mut row = vec![0.0; k];
                        for &i in &idx {
                            row[i] = 1.0 / size as f64;
                        }
                        coded.push(row);
                        // Next combination in lexicographic order.
                        let mut i = size;
                        while i > 0 && idx[i - 1] == k - size + i - 1 {
                            i -= 1;
                        }
                        if i == 0 {
                            break;
                        }
                        idx[i - 1] += 1;
                        for j in i..size {
                            idx[j] = idx[j - 1] + 1;
                        }
                    }
                }
            }
            if spec.augment {
                let centroid = vec![1.0 / k as f64; k];
                if !coded.iter().any(|r| r.iter().zip(&centroid).all(|(a, b)| (a - b).abs() < 1e-12)) {
                    coded.push(centroid);
                }
                for i in 0..k {
                    let mut row = vec![1.0 / (2.0 * k as f64); k];
                    row[i] = (k as f64 + 1.0) / (2.0 * k as f64);
                    coded.push(row);
                }
            }
            for _ in 0..spec.center {
                coded.push(vec![1.0 / k as f64; k]);
            }
            // Lower bounds: the design is in pseudo-components; map it back to proportions.
            let lo: Vec<f64> = spec.factors.iter().map(|f| f.low.unwrap_or(0.0)).collect();
            let sl: f64 = lo.iter().sum();
            if lo.iter().any(|v| *v < 0.0) || sl >= 1.0 {
                return Err(Error::Invalid("Lower bounds must be zero or more and add to less than 1.".into()));
            }
            if sl > 0.0 {
                for row in coded.iter_mut() {
                    for j in 0..k {
                        row[j] = lo[j] + (1.0 - sl) * row[j];
                    }
                }
            }
        }
        Kind::Vertices => {
            if !(2..=10).contains(&k) {
                return Err(Error::Invalid("A mixture design here has 2 to 10 components.".into()));
            }
            let lo: Vec<f64> = spec.factors.iter().map(|f| f.low.unwrap_or(0.0)).collect();
            let hi: Vec<f64> = spec.factors.iter().map(|f| f.high.unwrap_or(1.0)).collect();
            if lo.iter().zip(&hi).any(|(l, h)| *l < 0.0 || *h > 1.0 || l > h) {
                return Err(Error::Invalid("Each component's bounds must satisfy 0 ≤ lower ≤ upper ≤ 1.".into()));
            }
            if lo.iter().sum::<f64>() > 1.0 + 1e-12 || hi.iter().sum::<f64>() < 1.0 - 1e-12 {
                return Err(Error::Invalid("No blend meets these bounds: the lower bounds add to more than 1, or the upper bounds to less than 1.".into()));
            }
            let v = extreme_vertices(&lo, &hi);
            if v.is_empty() {
                return Err(Error::Invalid("No blend meets these bounds.".into()));
            }
            let centroid: Vec<f64> = (0..k).map(|j| v.iter().map(|r| r[j]).sum::<f64>() / v.len() as f64).collect();
            coded = v.clone();
            if spec.augment {
                coded.extend(edge_centroids(&v, &lo, &hi));
                coded.push(centroid.clone());
            }
            for _ in 0..spec.center {
                coded.push(centroid.clone());
            }
        }
        Kind::Optimal => {
            let levels: Vec<usize> = spec.factors.iter().map(|f| f.levels.unwrap_or(if spec.model == OptModel::Quadratic { 3 } else { 2 })).collect();
            if spec.model == OptModel::Quadratic && levels.iter().any(|l| *l < 3) {
                return Err(Error::Invalid("A quadratic model needs at least three levels of each factor.".into()));
            }
            let total: usize = levels.iter().map(|l| (*l).max(2)).product();
            if total > 20_000 {
                return Err(Error::Invalid("The grid of candidate settings is too large (at most 20,000 combinations).".into()));
            }
            let grid: Vec<Vec<f64>> = (0..total)
                .map(|r| {
                    let mut rest = r;
                    levels
                        .iter()
                        .map(|&l| {
                            let l = l.max(2);
                            let lv = rest % l;
                            rest /= l;
                            -1.0 + 2.0 * lv as f64 / (l - 1) as f64
                        })
                        .collect()
                })
                .collect();
            let cand: Vec<Vec<f64>> = grid.iter().map(|x| model_row(spec.model, x)).collect();
            let p = cand[0].len();
            let n = spec.runs.unwrap_or(p + 4);
            if n < p {
                return Err(Error::Invalid(format!("The model has {p} terms, so the design needs at least {p} runs.")));
            }
            if n > 500 {
                return Err(Error::Invalid("A D-optimal design here has at most 500 runs.".into()));
            }
            let mut rng = SplitMix64::new(spec.seed ^ 0x0D0E_0F00);
            let chosen = if spec.criterion == Criterion::I { i_exchange(&cand, n, 30, &mut rng) } else { federov(&cand, n, 30, &mut rng) };
            let Some(chosen) = chosen else {
                return Err(Error::Invalid("No design of that size supports the model.".into()));
            };
            coded = chosen.iter().map(|&i| grid[i].clone()).collect();
            let rows: Vec<Vec<f64>> = chosen.iter().map(|&i| cand[i].clone()).collect();
            d.criteria = criteria(&rows, &cand);
        }
    }
    if matches!(spec.kind, Kind::Full | Kind::Fractional | Kind::Pb | Kind::Optimal) {
        for _ in 0..spec.center {
            coded.push(vec![0.0; k]);
        }
    }
    // Replicates.
    let reps = spec.replicates.max(1);
    let base = coded.len();
    let base_blocks = blocks.clone();
    for _ in 1..reps {
        for r in 0..base {
            coded.push(coded[r].clone());
        }
        if let (Some(b), Some(bb)) = (blocks.as_mut(), base_blocks.as_ref()) {
            b.extend(bb.iter().copied());
        }
    }
    let n = coded.len();
    let mut order: Vec<usize> = (1..=n).collect();
    if spec.randomize {
        // Within blocks when there are blocks.
        let mut rng = SplitMix64::new(spec.seed);
        let groups: Vec<usize> = blocks.clone().unwrap_or(vec![1; n]);
        let mut start = 0;
        while start < n {
            let mut end = start;
            while end < n && groups[end] == groups[start] {
                end += 1;
            }
            for i in (start + 1..end).rev() {
                let j = start + rng.below(i - start + 1);
                order.swap(i, j);
            }
            start = end;
        }
    }
    d.coded = coded;
    d.blocks = blocks;
    d.run_order = order;
    Ok(d)
}

/// A factor's setting in its own units: low/high when given (coded −1/+1 map to them), else the coded value.
fn actual(f: &Factor, coded: f64, levels: usize) -> f64 {
    match (f.low, f.high) {
        (Some(lo), Some(hi)) if levels > 2 => lo + (coded - 1.0) * (hi - lo) / (levels as f64 - 1.0),
        (Some(lo), Some(hi)) => (lo + hi) / 2.0 + coded * (hi - lo) / 2.0,
        _ => coded,
    }
}

/// The design as CSV text, rows in run order: run, standard order, block, factors, then an empty response column.
pub fn csv(spec: &Spec, d: &Design) -> String {
    let full = spec.kind == Kind::Full;
    let mix = matches!(spec.kind, Kind::Lattice | Kind::Centroid | Kind::Vertices);
    let mut out = String::from("run,std_order");
    if d.blocks.is_some() {
        out.push_str(",block");
    }
    for n in &d.names {
        out.push(',');
        out.push_str(&n.replace([',', '"'], "_"));
    }
    out.push(',');
    out.push_str(spec.response.as_deref().unwrap_or("response"));
    out.push('\n');
    for (pos, &std) in d.run_order.iter().enumerate() {
        let r = std - 1;
        out.push_str(&format!("{},{}", pos + 1, std));
        if let Some(b) = &d.blocks {
            out.push_str(&format!(",{}", b[r]));
        }
        for (j, f) in spec.factors.iter().enumerate() {
            let lv = if full { f.levels.unwrap_or(2).max(2) } else { 2 };
            let v = if mix { d.coded[r][j] } else { actual(f, d.coded[r][j], lv) };
            out.push_str(&format!(",{}", fmt(v)));
        }
        out.push_str(",\n");
    }
    out
}

fn fmt(v: f64) -> String {
    let s = format!("{v:.10}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" { "0".into() } else { s }
}

/// The design summary (needs no data): the run sheet, and for fractional factorials the alias structure.
pub fn run(_ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    let d = build(spec)?;
    let k = spec.factors.len();
    let n = d.coded.len();
    let kind = match spec.kind {
        Kind::Full => "Full factorial design".to_string(),
        Kind::Fractional => format!("2^({}−{}) fractional factorial design", k, k - (spec.runs.unwrap_or(1).trailing_zeros() as usize).min(k)),
        Kind::Ccd => "Central composite design".to_string(),
        Kind::Bbd => "Box-Behnken design".to_string(),
        Kind::Pb => "Plackett-Burman design".to_string(),
        Kind::Lattice => format!("{{{}, {}}} simplex-lattice mixture design", k, spec.degree.unwrap_or(2)),
        Kind::Centroid => "Simplex-centroid mixture design".to_string(),
        Kind::Vertices => "Extreme-vertices mixture design".to_string(),
        Kind::Optimal => format!("{}-optimal design ({} model)", if spec.criterion == Criterion::I { "I" } else { "D" }, match spec.model {
            OptModel::Linear => "linear",
            OptModel::Interactions => "two-factor interaction",
            OptModel::Quadratic => "quadratic",
        }),
    };
    let note = format!(
        "{kind} {}: {} runs{}{}. Computed by PrismStatistics.",
        match spec.kind {
            Kind::Full => "as R's DoE.base fac.design",
            Kind::Fractional => "as R's FrF2 (minimum aberration, catalog generators)",
            Kind::Ccd => "as R's rsm ccd",
            Kind::Bbd => "as R's rsm bbd",
            Kind::Pb => "as R's FrF2 pb",
            Kind::Lattice => "as R's AlgDesign gen.mixture",
            Kind::Centroid => "(every equal blend of 1 to q components, pure components first)",
            Kind::Vertices => "(McLean and Anderson's extreme vertices of the constrained region)",
            Kind::Optimal => "as R's AlgDesign optFederov (exchange over the grid of candidate settings, 30 random starts)",
        },
        n,
        if spec.replicates > 1 { format!(" ({} replicates)", spec.replicates) } else { String::new() },
        if spec.randomize { format!(", randomized (seed {})", spec.seed) } else { ", in standard order".into() }
    );
    let mut b = common::bundle(DESIGN, METHOD_VERSION, kind.clone(), serde_json::to_value(spec).expect("a spec always serializes"), Tally::default().accounting(0, 0), note);
    set(&mut b, "runs", n as f64);
    set(&mut b, "factors", k as f64);
    if let Some(a) = d.alpha {
        set(&mut b, "alpha", a);
    }
    if let Some(r) = d.resolution {
        set(&mut b, "resolution", r as f64);
    }
    for (i, w) in d.wlp.iter().enumerate() {
        set(&mut b, format!("wlp.{}", i + 3), *w as f64);
    }
    set_text(&mut b, "generators", d.generators.join("; "));
    if let Some(c) = d.criteria {
        set(&mut b, "opt.d", c.d);
        set(&mut b, "opt.a", c.a);
        set(&mut b, "opt.g", c.g);
        set(&mut b, "opt.i", c.i);
        set(&mut b, "opt.terms", c.terms as f64);
    }
    set_text(&mut b, "alias.main", d.alias_main.join("; "));
    set_text(&mut b, "alias.fi2", d.alias_2fi.join("; "));
    // The run sheet.
    let full = spec.kind == Kind::Full;
    let mix = matches!(spec.kind, Kind::Lattice | Kind::Centroid | Kind::Vertices);
    let mut cols = vec!["Run".to_string(), "Std. order".into()];
    if d.blocks.is_some() {
        cols.push("Block".into());
    }
    cols.extend(d.names.iter().cloned());
    let mut rows = Vec::new();
    for (pos, &std) in d.run_order.iter().enumerate() {
        let r = std - 1;
        let mut row = vec![count(pos + 1), count(std)];
        if let Some(bl) = &d.blocks {
            row.push(count(bl[r]));
        }
        for (j, f) in spec.factors.iter().enumerate() {
            let lv = if full { f.levels.unwrap_or(2).max(2) } else { 2 };
            let v = if mix { d.coded[r][j] } else { actual(f, d.coded[r][j], lv) };
            set(&mut b, format!("x.{}.{}", std, j + 1), d.coded[r][j]);
            row.push(text(fmt(v)));
        }
        rows.push(row);
    }
    b.tables.push(Table { id: "design".into(), title: "Run sheet".into(), columns: cols, rows, footnotes: vec![if spec.randomize { "Runs are listed in the randomized order in which to carry them out; Std. order is the run's place in standard order.".into() } else { "Runs in standard order; randomize the order before carrying them out.".into() }] });
    if spec.kind == Kind::Fractional {
        let mut rows = vec![vec![text("Generators"), text(d.generators.join(", "))], vec![text("Defining relation"), text(format!("I = {}", d.words.join(" = ")))]];
        if let Some(r) = d.resolution {
            rows.push(vec![text("Resolution"), text(roman(r))]);
        }
        rows.push(vec![text("Main effects aliased with two-factor interactions"), text(if d.alias_main.is_empty() { "none".into() } else { d.alias_main.join("; ") })]);
        rows.push(vec![text("Aliased two-factor interactions"), text(if d.alias_2fi.is_empty() { "none".into() } else { d.alias_2fi.join("; ") })]);
        b.tables.push(Table { id: "aliases".into(), title: "Alias structure".into(), columns: vec!["".into(), "".into()], rows, footnotes: vec!["Effects joined by = cannot be told apart in this fraction (interactions of three or more factors are not shown).".into()] });
    }
    if let Some(c) = d.criteria {
        b.tables.push(Table {
            id: "optimality".into(),
            title: "Design criteria".into(),
            columns: vec!["Criterion".into(), "Value".into()],
            rows: vec![
                vec![text("Model terms (p)"), count(c.terms)],
                vec![text("Candidate settings"), count(c.candidates)],
                vec![text("D: det(X′X/N)^(1/p)"), num(c.d, 4)],
                vec![text("A: average variance factor, trace((X′X/N)⁻¹)/p"), num(c.a, 4)],
                vec![text("G-efficiency"), num(c.g, 3)],
                vec![text("I: average prediction variance over the candidates"), num(c.i, 4)],
            ],
            footnotes: vec!["Factors are in coded units (−1 to +1). G-efficiency is p divided by the largest scaled prediction variance over the candidate settings; 1 is the ideal.".into()],
        });
    }
    if spec.kind == Kind::Pb {
        b.tables.push(Table { id: "aliases".into(), title: "Alias structure".into(), columns: vec!["".into(), "".into()], rows: vec![vec![text("Main effects"), text("Clear of one another; each is partially aliased with two-factor interactions not involving it")]], footnotes: vec!["Plackett-Burman designs estimate main effects only: assume interactions are negligible, or follow up on the active factors.".into()] });
    }
    if let Some(a) = d.alpha {
        b.tables.push(Table { id: "ccd".into(), title: "Axial distance".into(), columns: vec!["".into(), "".into()], rows: vec![vec![text("α (coded units)"), num(a, 4)]], footnotes: vec![match spec.alpha {
            Alpha::Orthogonal => "Chosen so the cube and axial blocks are orthogonal (rsm's default).".into(),
            Alpha::Rotatable => "Rotatable: the prediction variance depends only on the distance from the center.".into(),
            Alpha::Face => "Face-centered: axial points on the faces of the cube.".into(),
        }] });
    }
    Ok(b)
}

fn roman(r: usize) -> String {
    ["", "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"].get(r).map_or(r.to_string(), |s| s.to_string())
}

// ───────────────────────────── Analysis of factorial experiments ─────────────────────────────

pub const EFFECTS: &str = "doe.effects";
pub const RSM: &str = "doe.rsm";

fn level() -> f64 {
    0.95
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectsSpec {
    pub response: String,
    /// Two-level factors (numeric with two settings, possibly with center points between them, or two categories).
    pub factors: Vec<String>,
    /// The highest order of interaction in the model (all of them when left out).
    #[serde(default)]
    pub order: Option<usize>,
    /// Lenth's α (and the t tests' when there are residual degrees of freedom).
    #[serde(default = "level")]
    pub level: f64,
}

/// Codes a factor −1/+1 (and 0 for a center setting halfway between): numeric settings, or the two categories in order.
fn code_factor(ds: &Dataset, name: &str) -> Result<(Vec<Slot<f64>>, String, String)> {
    let var = ds.variable(name)?;
    let i = ds.find(name).ok_or_else(|| Error::UnknownVariable(name.to_string()))?;
    let is_text = matches!(ds.columns[i], crate::model::Column::Text(_));
    if !is_text {
        let x = numeric(ds, name)?;
        let mut vals: Vec<f64> = x.iter().filter_map(Slot::value).collect();
        vals.sort_by(f64::total_cmp);
        vals.dedup();
        let (lo, hi) = (vals[0], *vals.last().unwrap_or(&vals[0]));
        let ok = vals.len() == 2 || (vals.len() == 3 && (vals[1] - (lo + hi) / 2.0).abs() < 1e-9 * (hi - lo).abs().max(1.0));
        if !ok {
            return Err(Error::Invalid(format!("{} needs two settings (and, optionally, center points halfway between them).", var.display_name())));
        }
        let coded = x.iter().map(|s| match s {
            Slot::Valid(v) => Slot::Valid((2.0 * v - lo - hi) / (hi - lo)),
            Slot::Empty => Slot::Empty,
            Slot::UserMissing => Slot::UserMissing,
        }).collect();
        return Ok((coded, apa::plain(lo), apa::plain(hi)));
    }
    let (slots, levels) = categories(ds, name)?;
    if levels.len() != 2 {
        return Err(Error::Invalid(format!("{} needs exactly two categories.", var.display_name())));
    }
    Ok((slots.iter().map(|s| match s {
        Slot::Valid(c) => Slot::Valid(if *c == 0 { -1.0 } else { 1.0 }),
        Slot::Empty => Slot::Empty,
        Slot::UserMissing => Slot::UserMissing,
    }).collect(), levels[0].label.clone(), levels[1].label.clone()))
}

/// OLS: coefficients, (X'X)⁻¹ and residual sum of squares. Columns that are (nearly) linear combinations of earlier
/// ones are left out; their indices are returned.
fn ols_keep(y: &[f64], cols: &[Vec<f64>]) -> Option<(Vec<usize>, Vec<f64>, super::linalg::Mat, f64)> {
    let n = y.len();
    // Gram-Schmidt to find the independent columns, in order.
    let mut kept: Vec<usize> = Vec::new();
    let mut basis: Vec<Vec<f64>> = Vec::new();
    for (j, c) in cols.iter().enumerate() {
        let mut v = c.clone();
        for q in &basis {
            let d: f64 = v.iter().zip(q).map(|(a, b)| a * b).sum();
            v.iter_mut().zip(q).for_each(|(a, b)| *a -= d * b);
        }
        let norm: f64 = v.iter().map(|a| a * a).sum::<f64>().sqrt();
        let scale: f64 = c.iter().map(|a| a * a).sum::<f64>().sqrt().max(1e-300);
        if norm > 1e-9 * scale {
            basis.push(v.iter().map(|a| a / norm).collect());
            kept.push(j);
        }
    }
    let p = kept.len();
    let mut xtx = vec![vec![0.0; p]; p];
    let mut xty = vec![0.0; p];
    for i in 0..n {
        for a in 0..p {
            xty[a] += cols[kept[a]][i] * y[i];
            for b in 0..p {
                xtx[a][b] += cols[kept[a]][i] * cols[kept[b]][i];
            }
        }
    }
    let inv = super::linalg::invert(&xtx)?;
    let beta: Vec<f64> = (0..p).map(|a| (0..p).map(|b| inv[a][b] * xty[b]).sum()).collect();
    let rss = (0..n).map(|i| (y[i] - (0..p).map(|a| cols[kept[a]][i] * beta[a]).sum::<f64>()).powi(2)).sum();
    Some((kept, beta, inv, rss))
}

pub fn effects(ds: &Dataset, spec: &EffectsSpec) -> Result<ResultBundle> {
    use prismqda_stats::dist::{norm_quantile, t_quantile, t_two_sided};
    common::check_level(spec.level)?;
    let k = spec.factors.len();
    if !(1..=15).contains(&k) {
        return Err(Error::Invalid("Choose 1 to 15 two-level factors.".into()));
    }
    let y = numeric(ds, &spec.response)?;
    let coded: Vec<(Vec<Slot<f64>>, String, String)> = spec.factors.iter().map(|f| code_factor(ds, f)).collect::<Result<_>>()?;
    let labels: Vec<String> = spec.factors.iter().map(|f| ds.variable(f).map(|v| v.display_name().to_string())).collect::<Result<_>>()?;
    let total = ds.n_rows();
    let mut tally = Tally::default();
    let mut rows = Vec::new();
    'r: for r in 0..total {
        if tally.check(&y[r], &spec.response) {
            continue;
        }
        for (j, (c, _, _)) in coded.iter().enumerate() {
            if tally.check(&c[r], &spec.factors[j]) {
                continue 'r;
            }
        }
        rows.push(r);
    }
    let n = rows.len();
    let yv: Vec<f64> = rows.iter().map(|&r| y[r].value().expect("checked")).collect();
    let x: Vec<Vec<f64>> = coded.iter().map(|(c, _, _)| rows.iter().map(|&r| c[r].value().expect("checked")).collect()).collect();
    // Terms: every subset of factors up to the order, by order then in standard (Yates) order.
    let order = spec.order.unwrap_or(k).clamp(1, k);
    let mut masks: Vec<u32> = (1u32..(1 << k)).filter(|m| m.count_ones() as usize <= order).collect();
    masks.sort_by_key(|m| (m.count_ones(), *m));
    let term = |m: u32| (0..k).filter(|&i| m & (1 << i) != 0).map(|i| labels[i].clone()).collect::<Vec<_>>().join(" × ");
    let mut cols = vec![vec![1.0; n]];
    cols.extend(masks.iter().map(|&m| (0..n).map(|i| (0..k).filter(|&j| m & (1 << j) != 0).map(|j| x[j][i]).product()).collect::<Vec<f64>>()));
    let note = format!("Factorial effects as twice the regression coefficients on −1/+1 coded factors (terms up to order {order}; a term aliased with an earlier one is left out); Lenth's pseudo standard error with its margin of error and simultaneous margin of error (as R's BsMD LenthPlot); t tests when the design leaves residual degrees of freedom. Computed by PrismStatistics.");
    let mut b = common::bundle(EFFECTS, METHOD_VERSION, format!("Factorial Effects: {}", ds.variable(&spec.response)?.display_name()), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, n), note);
    set_text(&mut b, "y.label", ds.variable(&spec.response)?.display_name().to_string());
    set(&mut b, "n", n as f64);
    let Some((kept, beta, inv, rss)) = ols_keep(&yv, &cols) else {
        common::fail(&mut b, "The effects could not be estimated.");
        return Ok(b);
    };
    let df = n as f64 - kept.len() as f64;
    let s2 = if df > 0.0 { rss / df } else { f64::NAN };
    let eff: Vec<(String, f64, usize)> = kept.iter().zip(&beta).enumerate().skip(1).map(|(a, (&j, c))| (term(masks[j - 1]), 2.0 * c, a)).collect();
    let dropped: Vec<String> = (1..cols.len()).filter(|j| !kept.contains(j)).map(|j| term(masks[j - 1])).collect();
    // Lenth.
    let m = eff.len();
    let alpha = 1.0 - spec.level;
    let (mut pse, mut me, mut sme) = (f64::NAN, f64::NAN, f64::NAN);
    if m >= 2 {
        let mut abs: Vec<f64> = eff.iter().map(|e| e.1.abs()).collect();
        let s0 = 1.5 * common::median(&abs).unwrap_or(f64::NAN);
        abs.retain(|v| *v < 2.5 * s0);
        pse = 1.5 * common::median(&abs).unwrap_or(f64::NAN);
        let dd = m as f64 / 3.0;
        me = t_quantile(1.0 - alpha / 2.0, dd) * pse;
        let gamma = (1.0 + (1.0 - alpha).powf(1.0 / m as f64)) / 2.0;
        sme = t_quantile(gamma, dd) * pse;
        set(&mut b, "lenth.pse", pse);
        set(&mut b, "lenth.me", me);
        set(&mut b, "lenth.sme", sme);
    }
    set(&mut b, "intercept", beta[0]);
    set(&mut b, "df", df);
    let mut out = Vec::new();
    for (idx, (name, e, a)) in eff.iter().enumerate() {
        let key = format!("effect.{}", idx + 1);
        set_text(&mut b, format!("{key}.term"), name.clone());
        set(&mut b, format!("{key}.est"), *e);
        set(&mut b, format!("{key}.coef"), e / 2.0);
        let mut row = vec![text(name.clone()), num(*e, 4), num(e / 2.0, 4)];
        if df > 0.0 {
            let se = 2.0 * (s2 * inv[*a][*a]).sqrt();
            let t = e / se;
            let p = t_two_sided(t, df);
            set(&mut b, format!("{key}.se"), se);
            set(&mut b, format!("{key}.t"), t);
            set(&mut b, format!("{key}.p"), p);
            row.extend([num(se, 4), num(t, 3), common::p(p)]);
        }
        let flag = if e.abs() > sme { "beyond SME" } else if e.abs() > me { "beyond ME" } else { "" };
        row.push(text(flag));
        out.push(row);
    }
    let mut cols_t = vec!["Term".to_string(), "Effect".into(), "Coefficient".into()];
    if df > 0.0 {
        cols_t.extend(["SE (effect)".into(), "t".into(), "p".into()]);
    }
    cols_t.push("Lenth".into());
    let mut foot = vec![format!("Intercept (grand mean) = {}. Lenth's PSE = {}, ME = {}, SME = {} (α = {}).", apa::num(beta[0], 4), apa::num(pse, 4), apa::num(me, 4), apa::num(sme, 4), apa::num(alpha, 2))];
    if !dropped.is_empty() {
        foot.push(format!("Left out as aliased with earlier terms: {}.", dropped.join(", ")));
    }
    if df > 0.0 {
        foot.push(format!("t tests on {} residual degrees of freedom.", apa::num(df, 0)));
    }
    b.tables.push(Table { id: "effects".into(), title: "Effects".into(), columns: cols_t, rows: out, footnotes: foot });
    b.tables.push(Table {
        id: "coding".into(),
        title: "Factor coding".into(),
        columns: vec!["Factor".into(), "−1".into(), "+1".into()],
        rows: coded.iter().zip(&labels).map(|((_, lo, hi), l)| vec![text(l.clone()), text(lo.clone()), text(hi.clone())]).collect(),
        footnotes: Vec::new(),
    });
    // Half-normal plot.
    let mut sorted: Vec<(f64, String)> = eff.iter().map(|e| (e.1.abs(), e.0.clone())).collect();
    sorted.sort_by(|a, c| a.0.total_cmp(&c.0));
    let pts: Vec<(f64, f64)> = sorted.iter().enumerate().map(|(i, (v, _))| (norm_quantile(0.5 + 0.5 * (i as f64 + 0.5) / m as f64), *v)).collect();
    let big: Vec<String> = sorted.iter().filter(|(v, _)| *v > me).map(|(_, n)| n.clone()).collect();
    let alt = format!("Half-normal plot of {m} absolute effects; {}.", if big.is_empty() { "none stands out beyond Lenth's margin of error".to_string() } else { format!("beyond the margin of error: {}", big.join(", ")) });
    let line = if me.is_finite() { vec![("ME".to_string(), vec![(0.0, me), (pts.last().map_or(2.0, |p| p.0), me)])] } else { Vec::new() };
    b.charts.push(crate::chart::Chart { id: "halfnormal".into(), title: "Half-normal plot of effects".into(), svg: crate::chart::scatter_curves(&pts, &line, "Half-normal quantile", "|Effect|", &alt), alt });
    for (i, nme) in big.iter().enumerate() {
        set_text(&mut b, format!("active.{}", i + 1), nme.clone());
    }
    Ok(b)
}

// ───────────────────────────── Response surfaces ─────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RsmSpec {
    pub response: String,
    /// Factors in coded units (the design's −α..+α settings).
    pub factors: Vec<String>,
    /// 1: first order; 2: first order, two-way interactions and pure quadratics (rsm's SO).
    #[serde(default = "two")]
    pub order: usize,
    /// A blocking variable, entered first.
    #[serde(default)]
    pub block: Option<String>,
    #[serde(default = "level")]
    pub level: f64,
}

fn two() -> usize {
    2
}

pub fn response_surface(ds: &Dataset, spec: &RsmSpec) -> Result<ResultBundle> {
    use prismqda_stats::dist::{f_sf, t_quantile, t_two_sided};
    common::check_level(spec.level)?;
    let k = spec.factors.len();
    if !(1..=8).contains(&k) {
        return Err(Error::Invalid("Choose 1 to 8 factors.".into()));
    }
    let y = numeric(ds, &spec.response)?;
    let xs: Vec<Vec<Slot<f64>>> = spec.factors.iter().map(|f| numeric(ds, f)).collect::<Result<_>>()?;
    let blk = spec.block.as_ref().map(|bn| categories(ds, bn)).transpose()?;
    let labels: Vec<String> = spec.factors.iter().map(|f| ds.variable(f).map(|v| v.display_name().to_string())).collect::<Result<_>>()?;
    let total = ds.n_rows();
    let mut tally = Tally::default();
    let mut rows = Vec::new();
    'r: for r in 0..total {
        if tally.check(&y[r], &spec.response) {
            continue;
        }
        for (j, c) in xs.iter().enumerate() {
            if tally.check(&c[r], &spec.factors[j]) {
                continue 'r;
            }
        }
        if let (Some((s, _)), Some(bn)) = (&blk, &spec.block) {
            if tally.check(&s[r], bn) {
                continue;
            }
        }
        rows.push(r);
    }
    let n = rows.len();
    let yv: Vec<f64> = rows.iter().map(|&r| y[r].value().expect("checked")).collect();
    let x: Vec<Vec<f64>> = xs.iter().map(|c| rows.iter().map(|&r| c[r].value().expect("checked")).collect()).collect();
    // Columns in rsm's order: intercept, blocks (treatment-coded against the first), FO, TWI, PQ.
    let mut cols: Vec<Vec<f64>> = vec![vec![1.0; n]];
    let mut names: Vec<String> = vec!["(Intercept)".into()];
    let mut groups: Vec<(&str, usize)> = Vec::new();
    let bcodes: Option<Vec<usize>> = blk.as_ref().map(|(s, _)| rows.iter().map(|&r| s[r].value().expect("checked")).collect());
    if let (Some(bc), Some((_, levels))) = (&bcodes, &blk) {
        let present: Vec<usize> = (0..levels.len()).filter(|l| bc.contains(l)).collect();
        for &l in present.iter().skip(1) {
            cols.push(bc.iter().map(|c| if *c == l { 1.0 } else { 0.0 }).collect());
            names.push(format!("Block {}", levels[l].label));
        }
        groups.push(("Block", present.len().saturating_sub(1)));
    }
    for j in 0..k {
        cols.push(x[j].clone());
        names.push(labels[j].clone());
    }
    groups.push(("FO", k));
    if spec.order >= 2 {
        let mut c = 0;
        for i in 0..k {
            for j in i + 1..k {
                cols.push((0..n).map(|r| x[i][r] * x[j][r]).collect());
                names.push(format!("{} × {}", labels[i], labels[j]));
                c += 1;
            }
        }
        groups.push(("TWI", c));
        for j in 0..k {
            cols.push(x[j].iter().map(|v| v * v).collect());
            names.push(format!("{}²", labels[j]));
        }
        groups.push(("PQ", k));
    }
    let p = cols.len();
    let lv = level_text(spec.level);
    let note = format!("Response-surface regression as R's rsm: {} model in coded units{}; sequential sums of squares for each group of terms, lack of fit against pure error (replicated settings), and the stationary point with its canonical analysis. Computed by PrismStatistics.", if spec.order >= 2 { "second-order (first order, two-way interactions, pure quadratics)" } else { "first-order" }, if spec.block.is_some() { " with blocks entered first" } else { "" });
    let mut b = common::bundle(RSM, METHOD_VERSION, format!("Response Surface: {}", ds.variable(&spec.response)?.display_name()), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, n), note);
    set_text(&mut b, "y.label", ds.variable(&spec.response)?.display_name().to_string());
    set(&mut b, "n", n as f64);
    if n <= p {
        common::fail(&mut b, format!("The model has {p} terms; it needs more than {p} runs."));
        return Ok(b);
    }
    let Some((kept, beta, inv, rss)) = ols_keep(&yv, &cols) else {
        common::fail(&mut b, "The model could not be estimated.");
        return Ok(b);
    };
    if kept.len() < p {
        common::fail(&mut b, "Some terms cannot be estimated from this design (the settings don't support the model).");
        return Ok(b);
    }
    let df = (n - p) as f64;
    let s2 = rss / df;
    let q = t_quantile(1.0 - (1.0 - spec.level) / 2.0, df);
    let mut coef_rows = Vec::new();
    for j in 0..p {
        let se = (s2 * inv[j][j]).sqrt();
        let t = beta[j] / se;
        let pv = t_two_sided(t, df);
        let key = format!("coef.{}", j + 1);
        set_text(&mut b, format!("{key}.term"), names[j].clone());
        for (kk, v) in [("est", beta[j]), ("se", se), ("t", t), ("p", pv)] {
            set(&mut b, format!("{key}.{kk}"), v);
        }
        coef_rows.push(vec![text(names[j].clone()), num(beta[j], 4), num(se, 4), num(t, 3), common::p(pv), text(format!("[{}, {}]", apa::num(beta[j] - q * se, 4), apa::num(beta[j] + q * se, 4)))]);
    }
    let ybar = yv.iter().sum::<f64>() / n as f64;
    let sst: f64 = yv.iter().map(|v| (v - ybar).powi(2)).sum();
    let r2 = 1.0 - rss / sst;
    let adj = 1.0 - (1.0 - r2) * (n as f64 - 1.0) / df;
    set(&mut b, "r2", r2);
    set(&mut b, "adj_r2", adj);
    set(&mut b, "sigma", s2.sqrt());
    b.tables.push(Table { id: "coefficients".into(), title: "Coefficients (coded units)".into(), columns: vec!["Term".into(), "Estimate".into(), "SE".into(), "t".into(), "p".into(), format!("{lv} CI")], rows: coef_rows, footnotes: vec![format!("R² = {}, adjusted R² = {}, residual SD = {} on {} df.", apa::no_leading_zero(r2, 3), apa::no_leading_zero(adj, 3), apa::num(s2.sqrt(), 4), apa::num(df, 0))] });
    // Sequential ANOVA by groups.
    let mut anova = Vec::new();
    let mut used = 1;
    let mut prev_rss = sst;
    for (gname, gdf) in &groups {
        if *gdf == 0 {
            continue;
        }
        let sub: Vec<Vec<f64>> = cols[..used + gdf].to_vec();
        let r = ols_keep(&yv, &sub).map_or(f64::NAN, |o| o.3);
        let ss = prev_rss - r;
        let f = ss / *gdf as f64 / s2;
        let pv = f_sf(f, *gdf as f64, df);
        let label = match *gname {
            "FO" => format!("First order ({})", labels.join(", ")),
            "TWI" => "Two-way interactions".into(),
            "PQ" => "Pure quadratics".into(),
            _ => "Blocks".into(),
        };
        set(&mut b, format!("anova.{}.ss", gname.to_lowercase()), ss);
        set(&mut b, format!("anova.{}.f", gname.to_lowercase()), f);
        set(&mut b, format!("anova.{}.p", gname.to_lowercase()), pv);
        anova.push(vec![text(label), count(*gdf), num(ss, 4), num(ss / *gdf as f64, 4), num(f, 3), common::p(pv)]);
        prev_rss = r;
        used += gdf;
    }
    set(&mut b, "anova.residual.ss", rss);
    anova.push(vec![text("Residual"), num(df, 0), num(rss, 4), num(s2, 4), text(""), text("")]);
    // Pure error: replicated settings (within blocks).
    let mut cells: Vec<(Vec<u64>, Vec<f64>)> = Vec::new();
    for i in 0..n {
        let mut key: Vec<u64> = x.iter().map(|c| c[i].to_bits()).collect();
        if let Some(bc) = &bcodes {
            key.push(bc[i] as u64);
        }
        match cells.iter_mut().find(|(kk, _)| *kk == key) {
            Some(c) => c.1.push(yv[i]),
            None => cells.push((key, vec![yv[i]])),
        }
    }
    let pe_ss: f64 = cells.iter().map(|(_, v)| { let m = v.iter().sum::<f64>() / v.len() as f64; v.iter().map(|a| (a - m).powi(2)).sum::<f64>() }).sum();
    let pe_df = (n - cells.len()) as f64;
    if pe_df > 0.0 && df - pe_df > 0.0 {
        let lof_ss = rss - pe_ss;
        let lof_df = df - pe_df;
        let f = (lof_ss / lof_df) / (pe_ss / pe_df);
        let pv = f_sf(f, lof_df, pe_df);
        set(&mut b, "lof.ss", lof_ss);
        set(&mut b, "lof.f", f);
        set(&mut b, "lof.p", pv);
        set(&mut b, "pe.ss", pe_ss);
        anova.push(vec![text("  Lack of fit"), num(lof_df, 0), num(lof_ss, 4), num(lof_ss / lof_df, 4), num(f, 3), common::p(pv)]);
        anova.push(vec![text("  Pure error"), num(pe_df, 0), num(pe_ss, 4), num(pe_ss / pe_df, 4), text(""), text("")]);
    }
    b.tables.push(Table { id: "anova".into(), title: "Analysis of variance".into(), columns: vec!["Source".into(), "df".into(), "SS".into(), "MS".into(), "F".into(), "p".into()], rows: anova, footnotes: vec!["Sums of squares are sequential, in the order shown.".into()] });
    // Canonical analysis (second order).
    if spec.order >= 2 {
        let off = 1 + groups.iter().find(|g| g.0 == "Block").map_or(0, |g| g.1);
        let bvec: Vec<f64> = (0..k).map(|j| beta[off + j]).collect();
        let mut bm = vec![vec![0.0; k]; k];
        let mut idx = off + k;
        for i in 0..k {
            for j in i + 1..k {
                bm[i][j] = beta[idx] / 2.0;
                bm[j][i] = beta[idx] / 2.0;
                idx += 1;
            }
        }
        for j in 0..k {
            bm[j][j] = beta[idx];
            idx += 1;
        }
        if let Some(bi) = super::linalg::invert(&bm) {
            let xs: Vec<f64> = (0..k).map(|i| -0.5 * (0..k).map(|j| bi[i][j] * bvec[j]).sum::<f64>()).collect();
            let ys = beta[0] + 0.5 * xs.iter().zip(&bvec).map(|(a, c)| a * c).sum::<f64>();
            let (vals, _) = super::linalg::symmetric_eigen(&bm);
            let mut vals = vals;
            vals.sort_by(|a, c| c.total_cmp(a));
            for j in 0..k {
                set(&mut b, format!("stationary.{}", j + 1), xs[j]);
                set(&mut b, format!("eigen.{}", j + 1), vals[j]);
            }
            set(&mut b, "stationary.y", ys);
            let kind = if vals.iter().all(|v| *v < 0.0) { "a maximum" } else if vals.iter().all(|v| *v > 0.0) { "a minimum" } else { "a saddle point" };
            set_text(&mut b, "stationary.kind", kind);
            let mut rows = Vec::new();
            for j in 0..k {
                rows.push(vec![text(labels[j].clone()), num(xs[j], 4)]);
            }
            rows.push(vec![text("Predicted response there"), num(ys, 4)]);
            b.tables.push(Table { id: "canonical".into(), title: "Stationary point (coded units)".into(), columns: vec!["".into(), "Value".into()], rows, footnotes: vec![format!("Eigenvalues of the quadratic part: {}. The stationary point is {kind}{}.", vals.iter().map(|v| apa::num(*v, 4)).collect::<Vec<_>>().join(", "), if xs.iter().any(|v| v.abs() > 1.5) { ", outside the region studied, so treat it as a direction rather than a location" } else { "" })] });
        }
    }
    Ok(b)
}

// ───────────────────────────── Mixture experiments ─────────────────────────────

pub const MIXTURE: &str = "doe.mixture";

/// The Scheffé mixture model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MixModel {
    Linear,
    #[default]
    Quadratic,
    SpecialCubic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixSpec {
    pub response: String,
    /// The mixture components (proportions, or amounts with the same total in every run).
    pub components: Vec<String>,
    #[serde(default)]
    pub model: MixModel,
    #[serde(default = "level")]
    pub level: f64,
}

/// Scheffé terms of a blend: the proportions, then (quadratic and up) the products of pairs, then (special cubic)
/// the products of triples.
fn scheffe(model: MixModel, pt: &[f64]) -> Vec<f64> {
    let q = pt.len();
    let mut v: Vec<f64> = pt.to_vec();
    if model != MixModel::Linear {
        for i in 0..q {
            for j in i + 1..q {
                v.push(pt[i] * pt[j]);
            }
        }
    }
    if model == MixModel::SpecialCubic {
        for i in 0..q {
            for j in i + 1..q {
                for l in j + 1..q {
                    v.push(pt[i] * pt[j] * pt[l]);
                }
            }
        }
    }
    v
}

/// Scheffé's mixture models fitted by least squares without an intercept (as lm(y ~ −1 + …)); R², the model F test
/// and lack of fit are on the centered scale, since the components sum to 1.
pub fn mixture(ds: &Dataset, spec: &MixSpec) -> Result<ResultBundle> {
    use prismqda_stats::dist::{f_sf, t_quantile, t_two_sided};
    common::check_level(spec.level)?;
    let q = spec.components.len();
    if !(2..=10).contains(&q) {
        return Err(Error::Invalid("Choose 2 to 10 mixture components.".into()));
    }
    if spec.model == MixModel::SpecialCubic && q < 3 {
        return Err(Error::Invalid("The special cubic model needs at least three components.".into()));
    }
    let y = numeric(ds, &spec.response)?;
    let xs: Vec<Vec<Slot<f64>>> = spec.components.iter().map(|f| numeric(ds, f)).collect::<Result<_>>()?;
    let labels: Vec<String> = spec.components.iter().map(|f| ds.variable(f).map(|v| v.display_name().to_string())).collect::<Result<_>>()?;
    let total = ds.n_rows();
    let mut tally = Tally::default();
    let mut rows = Vec::new();
    'r: for r in 0..total {
        if tally.check(&y[r], &spec.response) {
            continue;
        }
        for (j, c) in xs.iter().enumerate() {
            if tally.check(&c[r], &spec.components[j]) {
                continue 'r;
            }
        }
        rows.push(r);
    }
    let n = rows.len();
    let yv: Vec<f64> = rows.iter().map(|&r| y[r].value().expect("checked")).collect();
    let raw: Vec<Vec<f64>> = rows.iter().map(|&r| xs.iter().map(|c| c[r].value().expect("checked")).collect()).collect();
    let sums: Vec<f64> = raw.iter().map(|r| r.iter().sum()).collect();
    if raw.iter().flatten().any(|v| *v < 0.0) || sums.iter().any(|s| *s <= 0.0) {
        return Err(Error::Invalid("Mixture components must be zero or more, with a positive total.".into()));
    }
    let s0 = sums.first().copied().unwrap_or(1.0);
    if sums.iter().any(|s| (s - s0).abs() > 1e-6 * s0.abs().max(1.0)) {
        return Err(Error::Invalid("The components must add to the same total in every run (they are proportions of a mixture).".into()));
    }
    // Proportions, one row per run.
    let props: Vec<Vec<f64>> = raw.iter().zip(&sums).map(|(r, s)| r.iter().map(|v| v / s).collect()).collect();
    let design: Vec<Vec<f64>> = props.iter().map(|r| scheffe(spec.model, r)).collect();
    let p = design[0].len();
    let cols: Vec<Vec<f64>> = (0..p).map(|j| design.iter().map(|r| r[j]).collect()).collect();
    let mut names: Vec<String> = labels.clone();
    if spec.model != MixModel::Linear {
        for i in 0..q {
            for j in i + 1..q {
                names.push(format!("{} × {}", labels[i], labels[j]));
            }
        }
    }
    if spec.model == MixModel::SpecialCubic {
        for i in 0..q {
            for j in i + 1..q {
                for l in j + 1..q {
                    names.push(format!("{} × {} × {}", labels[i], labels[j], labels[l]));
                }
            }
        }
    }
    let model_name = match spec.model {
        MixModel::Linear => "linear",
        MixModel::Quadratic => "quadratic",
        MixModel::SpecialCubic => "special cubic",
    };
    let lv = level_text(spec.level);
    let ylab = ds.variable(&spec.response)?.display_name().to_string();
    let note = format!("Scheffé {model_name} mixture model fitted by least squares without an intercept (components as proportions of their total); R² and the F test are relative to the mean, as for any model with a constant, since the components sum to 1. Computed by PrismStatistics.");
    let mut b = common::bundle(MIXTURE, METHOD_VERSION, format!("Mixture Model: {ylab}"), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, n), note);
    set_text(&mut b, "y.label", ylab);
    set(&mut b, "n", n as f64);
    set_text(&mut b, "model", model_name);
    if n <= p {
        common::fail(&mut b, format!("The model has {p} terms; it needs more than {p} runs."));
        return Ok(b);
    }
    let Some((kept, beta, inv, rss)) = ols_keep(&yv, &cols) else {
        common::fail(&mut b, "The model could not be estimated.");
        return Ok(b);
    };
    if kept.len() < p {
        common::fail(&mut b, "Some terms cannot be estimated from these blends: the design does not support the model.");
        return Ok(b);
    }
    let df = (n - p) as f64;
    let s2 = rss / df;
    let tq = t_quantile(1.0 - (1.0 - spec.level) / 2.0, df);
    let mut coef_rows = Vec::new();
    for j in 0..p {
        let se = (s2 * inv[j][j]).sqrt();
        let t = beta[j] / se;
        let pv = t_two_sided(t, df);
        let key = format!("coef.{}", j + 1);
        set_text(&mut b, format!("{key}.term"), names[j].clone());
        for (kk, v) in [("est", beta[j]), ("se", se), ("t", t), ("p", pv)] {
            set(&mut b, format!("{key}.{kk}"), v);
        }
        coef_rows.push(vec![text(names[j].clone()), num(beta[j], 4), num(se, 4), num(t, 3), common::p(pv), text(format!("[{}, {}]", apa::num(beta[j] - tq * se, 4), apa::num(beta[j] + tq * se, 4)))]);
    }
    let ybar = yv.iter().sum::<f64>() / n as f64;
    let sst: f64 = yv.iter().map(|v| (v - ybar).powi(2)).sum();
    let r2 = 1.0 - rss / sst;
    let adj = 1.0 - (1.0 - r2) * (n as f64 - 1.0) / df;
    let mdf = (p - 1) as f64;
    let f = (sst - rss) / mdf / s2;
    let fp = f_sf(f, mdf, df);
    for (kk, v) in [("r2", r2), ("adj_r2", adj), ("sigma", s2.sqrt()), ("f", f), ("f.p", fp), ("df1", mdf), ("df2", df)] {
        set(&mut b, kk, v);
    }
    b.tables.push(Table {
        id: "coefficients".into(),
        title: "Coefficients".into(),
        columns: vec!["Term".into(), "Estimate".into(), "SE".into(), "t".into(), "p".into(), format!("{lv} CI")],
        rows: coef_rows,
        footnotes: vec![
            format!("R² = {}, adjusted R² = {}, residual SD = {} on {} df.", apa::no_leading_zero(r2, 3), apa::no_leading_zero(adj, 3), apa::num(s2.sqrt(), 4), apa::num(df, 0)),
            "A linear coefficient is the predicted response of the pure component; a blending coefficient is synergistic when positive and antagonistic when negative. The t tests of linear coefficients test against 0, which is rarely of interest.".into(),
        ],
    });
    let mut anova = vec![vec![text("Model"), num(mdf, 0), num(sst - rss, 4), num((sst - rss) / mdf, 4), num(f, 3), common::p(fp)], vec![text("Residual"), num(df, 0), num(rss, 4), num(s2, 4), text(""), text("")]];
    // Pure error from replicated blends.
    let mut cells: Vec<(Vec<i64>, Vec<f64>)> = Vec::new();
    for i in 0..n {
        let key: Vec<i64> = props[i].iter().map(|v| (v * 1e9).round() as i64).collect();
        match cells.iter_mut().find(|(kk, _)| *kk == key) {
            Some(c) => c.1.push(yv[i]),
            None => cells.push((key, vec![yv[i]])),
        }
    }
    let pe_ss: f64 = cells
        .iter()
        .map(|(_, v)| {
            let m = v.iter().sum::<f64>() / v.len() as f64;
            v.iter().map(|a| (a - m).powi(2)).sum::<f64>()
        })
        .sum();
    let pe_df = (n - cells.len()) as f64;
    if pe_df > 0.0 && df - pe_df > 0.0 {
        let lof_df = df - pe_df;
        let lof_ss = rss - pe_ss;
        let lf = (lof_ss / lof_df) / (pe_ss / pe_df);
        let lp = f_sf(lf, lof_df, pe_df);
        set(&mut b, "lof.ss", lof_ss);
        set(&mut b, "lof.f", lf);
        set(&mut b, "lof.p", lp);
        set(&mut b, "pe.ss", pe_ss);
        anova.push(vec![text("  Lack of fit"), num(lof_df, 0), num(lof_ss, 4), num(lof_ss / lof_df, 4), num(lf, 3), common::p(lp)]);
        anova.push(vec![text("  Pure error"), num(pe_df, 0), num(pe_ss, 4), num(pe_ss / pe_df, 4), text(""), text("")]);
    }
    anova.push(vec![text("Total (corrected)"), num(n as f64 - 1.0, 0), num(sst, 4), text(""), text(""), text("")]);
    b.tables.push(Table { id: "anova".into(), title: "Analysis of variance".into(), columns: vec!["Source".into(), "df".into(), "SS".into(), "MS".into(), "F".into(), "p".into()], rows: anova, footnotes: Vec::new() });
    // Predicted responses of the pure components and of the overall centroid.
    let pred = |pt: &[f64]| -> f64 { scheffe(spec.model, pt).iter().zip(&beta).map(|(a, c)| a * c).sum() };
    let mut prow = Vec::new();
    for j in 0..q {
        let mut pt = vec![0.0; q];
        pt[j] = 1.0;
        prow.push(vec![text(format!("Pure {}", labels[j])), num(pred(&pt), 4)]);
    }
    let cen = pred(&vec![1.0 / q as f64; q]);
    set(&mut b, "centroid.y", cen);
    prow.push(vec![text("Overall centroid (equal parts)"), num(cen, 4)]);
    b.tables.push(Table { id: "predictions".into(), title: "Predicted responses".into(), columns: vec!["Blend".into(), "Predicted".into()], rows: prow, footnotes: Vec::new() });
    Ok(b)
}
