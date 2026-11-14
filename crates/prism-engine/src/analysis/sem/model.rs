//! The parameter table and the model it describes, with lavaan's defaults filled in (checklist C1a–C1d).
//!
//! Variables are held in reticular action model (RAM) form: every variable, observed or latent, is v = A v + u with
//! Cov(u) = S and E(u) = m. Loadings (`f =~ x`) and regressions (`y ~ x`) are entries of A, variances and covariances of
//! S, and intercepts of m. The implied moments of the observed variables follow from B = (I − A)⁻¹:
//! Σ = F B S Bᵀ Fᵀ and μ = F B m.
//!
//! The defaults are lavaan's for `sem()` and `cfa()`: the first loading of each factor is fixed at 1 (or, with
//! `std_lv`, the factor variances are fixed at 1); every variable gets a free (residual) variance; exogenous factors
//! covary; the residuals of pure outcomes covary; and observed predictors that are not modeled ("fixed.x") have their
//! variances, covariances and means fixed at the sample values.

use super::syntax::{Expr, Modifier, Op, Statement};
use crate::analysis::linalg::{invert, Mat};
use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Matrix {
    A,
    S,
    M,
    /// A threshold of an ordinal variable (i: the variable, j: which threshold).
    T,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub group: usize,
    pub lhs: String,
    /// "=~", "~", "~~" or "~1".
    pub op: &'static str,
    pub rhs: String,
    pub label: Option<String>,
    /// The index in the free-parameter vector, or None when fixed.
    pub free: Option<usize>,
    /// The fixed value, or the starting value.
    pub value: f64,
    pub start_given: bool,
    pub user: bool,
    /// A fixed.x moment: fixed at the sample value.
    pub exo: bool,
    pub mat: Matrix,
    pub i: usize,
    pub j: usize,
}

impl Param {
    pub fn name(&self) -> String {
        if self.op == "~1" {
            format!("{} ~1", self.lhs)
        } else if self.op == "|" {
            format!("{} | {}", self.lhs, self.rhs)
        } else {
            format!("{} {} {}", self.lhs, self.op, self.rhs)
        }
    }
}

#[derive(Debug, Clone)]
pub struct Model {
    /// Observed variables first, then latent ones; indices into these are the RAM indices.
    pub obs: Vec<String>,
    pub lat: Vec<String>,
    pub groups: usize,
    pub params: Vec<Param>,
    pub nfree: usize,
    pub defined: Vec<(String, Expr)>,
    pub meanstructure: bool,
    /// Observed variables whose moments are fixed at the sample values.
    pub xvars: Vec<usize>,
    /// Ordinal observed variables (WLSMV): their latent responses have variance 1, so their residual variances are
    /// 1 minus the common variance (the delta parameterization).
    pub ordinal: Vec<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub std_lv: bool,
    pub meanstructure: bool,
    pub fixed_x: bool,
    /// A latent growth model (lavaan's growth()): observed intercepts fixed at 0 and latent means free.
    pub growth: bool,
}

fn pick(mods: &[Modifier], group: usize, groups: usize) -> Result<Vec<Modifier>> {
    let mut out = Vec::new();
    for m in mods {
        match m {
            Modifier::Groups(v) => {
                if v.len() != groups {
                    return Err(Error::Invalid(format!("A c(…) modifier has {} values but the model has {groups} group{}.", v.len(), if groups == 1 { "" } else { "s" })));
                }
                out.push(v[group].clone());
            }
            other => out.push(other.clone()),
        }
    }
    Ok(out)
}

struct Row {
    lhs: String,
    op: &'static str,
    rhs: String,
    mods: Vec<Modifier>,
    user: bool,
}

impl Model {
    pub fn t(&self) -> usize {
        self.obs.len() + self.lat.len()
    }

    pub fn p(&self) -> usize {
        self.obs.len()
    }

    pub fn index(&self, name: &str) -> Option<usize> {
        self.obs.iter().position(|v| v == name).or_else(|| self.lat.iter().position(|v| v == name).map(|k| self.obs.len() + k))
    }

    pub fn name_of(&self, i: usize) -> &str {
        if i < self.obs.len() {
            &self.obs[i]
        } else {
            &self.lat[i - self.obs.len()]
        }
    }

    pub fn is_latent(&self, i: usize) -> bool {
        i >= self.obs.len()
    }

    /// Builds the model from parsed statements. `is_observed` says whether a name is a variable in the data.
    pub fn build(statements: &[Statement], is_observed: &dyn Fn(&str) -> bool, opts: &Options, groups: usize) -> Result<Model> {
        if statements.iter().any(|s| matches!(s, Statement::Level(_))) {
            return Err(Error::Invalid("A two-level model (level: 1 and level: 2) needs a cluster variable.".into()));
        }
        if statements.iter().any(|s| matches!(s, Statement::Relation { op: Op::Composite, .. })) {
            return Err(Error::Invalid("Composites (<~) are for PLS-SEM; in a covariance-based model write latent variables with =~.".into()));
        }
        let mut lat: Vec<String> = Vec::new();
        for s in statements {
            if let Statement::Relation { lhs, op: Op::Measured, .. } = s {
                if !lat.contains(lhs) {
                    if is_observed(lhs) {
                        return Err(Error::Invalid(format!("\"{lhs}\" is a variable in the data, so it can't also be a latent variable measured with =~.")));
                    }
                    lat.push(lhs.clone());
                }
            }
        }
        let mut obs: Vec<String> = Vec::new();
        let mut note = |n: &str| -> Result<()> {
            if n == "1" || lat.iter().any(|l| l == n) || obs.iter().any(|o| o == n) {
                return Ok(());
            }
            if !is_observed(n) {
                return Err(Error::Invalid(format!("\"{n}\" is not a variable in the data, and no latent variable of that name is defined with =~.")));
            }
            obs.push(n.to_string());
            Ok(())
        };
        // Observed variables in order: indicators first, then the rest, as lavaan orders them.
        for s in statements {
            if let Statement::Relation { op: Op::Measured, rhs, .. } = s {
                note(&rhs.var)?;
            }
        }
        for s in statements {
            if let Statement::Relation { lhs, rhs, .. } = s {
                note(lhs)?;
                note(&rhs.var)?;
            }
        }
        let has = |v: &[String], n: &str| v.iter().any(|x| x == n);
        let mut indicators: Vec<String> = Vec::new();
        let mut y_reg: Vec<String> = Vec::new();
        let mut x_reg: Vec<String> = Vec::new();
        let mut meanstructure = opts.meanstructure || groups > 1 || opts.growth;
        for s in statements {
            if let Statement::Relation { lhs, op, rhs } = s {
                match op {
                    Op::Measured => indicators.push(rhs.var.clone()),
                    Op::Regression if rhs.var == "1" => meanstructure = true,
                    Op::Regression => {
                        y_reg.push(lhs.clone());
                        x_reg.push(rhs.var.clone());
                    }
                    Op::Covariance | Op::Composite => {}
                }
            }
        }
        // Observed predictors that are not outcomes: they covary freely either way (as in lavaan), and with fixed.x
        // their moments are held at the sample values.
        let exo_obs: Vec<String> = obs.iter().filter(|o| has(&x_reg, o) && !has(&y_reg, o) && !has(&indicators, o)).cloned().collect();
        let xvars_names: Vec<String> = if opts.fixed_x { exo_obs.clone() } else { Vec::new() };
        let exo_lat: Vec<String> = lat.iter().filter(|l| !has(&y_reg, l) && !has(&indicators, l)).cloned().collect();
        let pure_y: Vec<String> = obs.iter().chain(lat.iter()).filter(|v| has(&y_reg, v) && !has(&x_reg, v) && !has(&indicators, v)).cloned().collect();

        let mut model = Model { obs: obs.clone(), lat: lat.clone(), groups, params: Vec::new(), nfree: 0, defined: Vec::new(), meanstructure, xvars: Vec::new(), ordinal: Vec::new() };
        model.xvars = xvars_names.iter().map(|n| model.index(n).expect("observed")).collect();
        let mut labels: Vec<(String, usize)> = Vec::new();
        for g in 0..groups {
            let mut rows: Vec<Row> = Vec::new();
            for s in statements {
                if let Statement::Relation { lhs, op, rhs } = s {
                    let (op, rhs_name) = match op {
                        Op::Regression if rhs.var == "1" => ("~1", String::new()),
                        o => (o.symbol(), rhs.var.clone()),
                    };
                    rows.push(Row { lhs: lhs.clone(), op, rhs: rhs_name, mods: pick(&rhs.modifiers, g, groups)?, user: true });
                }
            }
            let fixed_or_free = |m: &[Modifier]| m.iter().any(|x| matches!(x, Modifier::Fixed(_) | Modifier::Free));
            // The first loading of each factor.
            for l in &lat {
                if let Some(r) = rows.iter_mut().find(|r| r.op == "=~" && &r.lhs == l) {
                    if !opts.std_lv && !fixed_or_free(&r.mods) {
                        r.mods.push(Modifier::Fixed(1.0));
                    }
                }
            }
            let specified = |rows: &[Row], a: &str, b: &str| rows.iter().any(|r| r.op == "~~" && ((r.lhs == a && r.rhs == b) || (r.lhs == b && r.rhs == a)));
            let mut auto: Vec<Row> = Vec::new();
            for v in obs.iter().chain(lat.iter()) {
                if !specified(&rows, v, v) {
                    let mut mods = Vec::new();
                    if opts.std_lv && has(&lat, v) {
                        mods.push(Modifier::Fixed(1.0));
                    }
                    auto.push(Row { lhs: v.clone(), op: "~~", rhs: v.clone(), mods, user: false });
                }
            }
            for set in [&exo_lat, &pure_y, &exo_obs] {
                for (k, u) in set.iter().enumerate() {
                    for w in &set[k + 1..] {
                        if !specified(&rows, u, w) && !specified(&auto, u, w) {
                            auto.push(Row { lhs: u.clone(), op: "~~", rhs: w.clone(), mods: Vec::new(), user: false });
                        }
                    }
                }
            }
            if meanstructure {
                for v in obs.iter().chain(lat.iter()) {
                    if !rows.iter().any(|r| r.op == "~1" && &r.lhs == v) {
                        let mods = match (has(&lat, v), opts.growth) {
                            (true, false) | (false, true) => vec![Modifier::Fixed(0.0)],
                            _ => Vec::new(),
                        };
                        auto.push(Row { lhs: v.clone(), op: "~1", rhs: String::new(), mods, user: false });
                    }
                }
            }
            rows.extend(auto);
            for r in rows {
                let (mat, i, j) = match r.op {
                    "=~" => (Matrix::A, model.index(&r.rhs).expect("named"), model.index(&r.lhs).expect("named")),
                    "~" => (Matrix::A, model.index(&r.lhs).expect("named"), model.index(&r.rhs).expect("named")),
                    "~~" => (Matrix::S, model.index(&r.lhs).expect("named"), model.index(&r.rhs).expect("named")),
                    _ => (Matrix::M, model.index(&r.lhs).expect("named"), 0),
                };
                let exo = match mat {
                    Matrix::S => model.xvars.contains(&i) && model.xvars.contains(&j),
                    Matrix::M => model.xvars.contains(&i),
                    Matrix::A | Matrix::T => false,
                };
                let (mut fixed, mut label, mut start) = (None, None, None);
                for m in &r.mods {
                    match m {
                        Modifier::Fixed(v) => fixed = Some(*v),
                        Modifier::Free => fixed = None,
                        Modifier::Label(l) => label = Some(l.clone()),
                        Modifier::Start(v) => start = Some(*v),
                        Modifier::Groups(_) => {}
                    }
                }
                if let Some(dup) = model.params.iter().find(|p| p.group == g && p.mat == mat && ((p.i == i && p.j == j) || (mat == Matrix::S && p.i == j && p.j == i))) {
                    return Err(Error::Invalid(format!("The parameter {} is given twice.", dup.name())));
                }
                if exo && r.user {
                    // A user line about a fixed.x moment: still fixed at the sample value, as lavaan does.
                    fixed = Some(0.0);
                }
                let free = if exo || fixed.is_some() {
                    None
                } else if let Some(l) = &label {
                    Some(match labels.iter().find(|(n, _)| n == l) {
                        Some((_, k)) => *k,
                        None => {
                            labels.push((l.clone(), model.nfree));
                            model.nfree += 1;
                            model.nfree - 1
                        }
                    })
                } else {
                    model.nfree += 1;
                    Some(model.nfree - 1)
                };
                model.params.push(Param { group: g, lhs: r.lhs, op: r.op, rhs: r.rhs, label, free, value: fixed.or(start).unwrap_or(0.0), start_given: start.is_some(), user: r.user, exo, mat, i, j });
            }
        }
        for s in statements {
            match s {
                Statement::Defined { name, expr } => {
                    let mut names = Vec::new();
                    expr.names(&mut names);
                    for n in &names {
                        if !model.params.iter().any(|p| p.label.as_deref() == Some(n)) && !model.defined.iter().any(|(d, _)| d == n) {
                            return Err(Error::Invalid(format!("The defined parameter {name} uses \"{n}\", which is not a label in the model.")));
                        }
                    }
                    model.defined.push((name.clone(), expr.clone()));
                }
                Statement::Equal { lhs, rhs } => model.constrain(lhs, rhs)?,
                Statement::Relation { .. } | Statement::Level(_) => {}
            }
        }
        Ok(model)
    }

    /// `a == b` merges two labels; `a == 2` fixes one.
    fn constrain(&mut self, lhs: &Expr, rhs: &Expr) -> Result<()> {
        let free_of = |m: &Model, l: &str| m.params.iter().find(|p| p.label.as_deref() == Some(l)).map(|p| p.free);
        match (lhs, rhs) {
            (Expr::Name(a), Expr::Name(b)) => {
                let (Some(fa), Some(fb)) = (free_of(self, a), free_of(self, b)) else {
                    return Err(Error::Invalid(format!("The constraint {a} == {b} uses a name that is not a label in the model.")));
                };
                if let (Some(ka), Some(kb)) = (fa, fb) {
                    if ka != kb {
                        self.merge(ka, kb);
                    }
                }
                Ok(())
            }
            (Expr::Name(a), e) | (e, Expr::Name(a)) if !matches!(e, Expr::Name(_)) => {
                let v = e.eval(&|_| None).map_err(|_| Error::Invalid(format!("Only constraints between two labels, or a label and a number, are supported ({a} == …).")))?;
                if free_of(self, a).is_none() {
                    return Err(Error::Invalid(format!("The constraint on {a} uses a name that is not a label in the model.")));
                }
                if let Some(Some(k)) = free_of(self, a) {
                    for p in &mut self.params {
                        if p.free == Some(k) {
                            p.free = None;
                            p.value = v;
                        }
                    }
                    self.renumber();
                }
                Ok(())
            }
            _ => Err(Error::Invalid("Only constraints between two labels, or a label and a number, are supported.".into())),
        }
    }

    fn merge(&mut self, keep: usize, drop: usize) {
        for p in &mut self.params {
            if p.free == Some(drop) {
                p.free = Some(keep);
            }
        }
        self.renumber();
    }

    /// Renumbers free parameters 0..n in order of first appearance.
    pub(crate) fn renumber(&mut self) {
        let mut map: Vec<(usize, usize)> = Vec::new();
        for p in &mut self.params {
            if let Some(k) = p.free {
                let n = match map.iter().find(|(o, _)| *o == k) {
                    Some((_, n)) => *n,
                    None => {
                        map.push((k, map.len()));
                        map.len() - 1
                    }
                };
                p.free = Some(n);
            }
        }
        self.nfree = map.len();
    }

    /// Holds parameters of the listed kinds equal across groups (lavaan's group.equal), except the ones named in
    /// `partial` (group.partial, written like "visual =~ x2" or "x3 ~1"). As lavaan does, equal intercepts free the
    /// factor means of the later groups, and with factor variances fixed at 1 equal loadings free the later groups'
    /// factor variances.
    pub fn equate(&mut self, kinds: &[String], partial: &[String], std_lv: bool) -> Result<()> {
        const KINDS: [&str; 8] = ["loadings", "intercepts", "means", "residuals", "residual.covariances", "lv.variances", "lv.covariances", "regressions"];
        for k in kinds {
            if !KINDS.contains(&k.as_str()) {
                return Err(Error::Invalid(format!("\"{k}\" can't be held equal across groups; use {}.", KINDS.join(", "))));
            }
        }
        if self.groups < 2 || kinds.is_empty() {
            return Ok(());
        }
        let p = self.p();
        let kind_of = |q: &Param| -> &'static str {
            match (q.op, q.i == q.j, q.i < p && q.j < p) {
                ("=~", ..) => "loadings",
                ("~", ..) => "regressions",
                ("~1", ..) if q.i < p => "intercepts",
                ("~1", ..) => "means",
                ("~~", true, _) if q.i < p => "residuals",
                ("~~", true, _) => "lv.variances",
                ("~~", false, true) => "residual.covariances",
                _ => "lv.covariances",
            }
        };
        let squash = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        let partial: Vec<String> = partial.iter().map(|s| squash(s)).collect();
        let has = |k: &str| kinds.iter().any(|x| x == k);
        // Freed by equal intercepts or loadings.
        for q in &mut self.params {
            if q.group == 0 || q.user {
                continue;
            }
            let latent = q.i >= p;
            if q.op == "~1" && latent && has("intercepts") && !has("means") {
                q.free = Some(usize::MAX);
            }
            if std_lv && q.op == "~~" && latent && q.i == q.j && has("loadings") && !has("lv.variances") {
                q.free = Some(usize::MAX);
            }
        }
        let mut next = self.nfree;
        for q in &mut self.params {
            if q.free == Some(usize::MAX) {
                q.free = Some(next);
                next += 1;
            }
        }
        self.nfree = next;
        let first: Vec<(String, &'static str, String, usize)> = self
            .params
            .iter()
            .filter(|q| q.group == 0 && !q.exo && has(kind_of(q)) && !partial.contains(&squash(&q.name())))
            .filter_map(|q| q.free.map(|k| (q.lhs.clone(), q.op, q.rhs.clone(), k)))
            .collect();
        for (lhs, op, rhs, k) in first {
            for q in &mut self.params {
                if q.group > 0 && q.op == op && ((q.lhs == lhs && q.rhs == rhs) || (op == "~~" && q.lhs == rhs && q.rhs == lhs)) {
                    if let Some(own) = q.free {
                        if own != k {
                            q.free = Some(k);
                        }
                    }
                }
            }
        }
        self.renumber();
        Ok(())
    }

    /// The current value of every free parameter.
    pub fn theta(&self) -> Vec<f64> {
        let mut th = vec![0.0; self.nfree];
        for p in &self.params {
            if let Some(k) = p.free {
                th[k] = p.value;
            }
        }
        th
    }

    pub fn set_theta(&mut self, th: &[f64]) {
        for p in &mut self.params {
            if let Some(k) = p.free {
                p.value = th[k];
            }
        }
    }

    /// The RAM matrices of one group at parameters `th`.
    pub fn matrices(&self, g: usize, th: &[f64]) -> (Mat, Mat, Vec<f64>) {
        let t = self.t();
        let (mut a, mut s, mut m) = (vec![vec![0.0; t]; t], vec![vec![0.0; t]; t], vec![0.0; t]);
        for p in self.params.iter().filter(|p| p.group == g) {
            let v = p.free.map_or(p.value, |k| th[k]);
            match p.mat {
                Matrix::A => a[p.i][p.j] = v,
                Matrix::S => {
                    s[p.i][p.j] = v;
                    s[p.j][p.i] = v;
                }
                Matrix::M => m[p.i] = v,
                Matrix::T => {}
            }
        }
        (a, s, m)
    }

    /// B = (I − A)⁻¹, the full covariance B S Bᵀ and the full mean B m of one group.
    pub fn implied_full(&self, g: usize, th: &[f64]) -> Option<Implied> {
        let t = self.t();
        let (a, mut s, m) = self.matrices(g, th);
        let ia: Mat = (0..t).map(|i| (0..t).map(|j| f64::from(u8::from(i == j)) - a[i][j]).collect()).collect();
        let b = invert(&ia)?;
        let bs: Mat = (0..t).map(|i| (0..t).map(|j| (0..t).map(|k| b[i][k] * s[k][j]).sum()).collect()).collect();
        let full: Mat = (0..t).map(|i| (0..t).map(|j| (0..t).map(|k| bs[i][k] * b[j][k]).sum()).collect()).collect();
        let mean: Vec<f64> = (0..t).map(|i| (0..t).map(|k| b[i][k] * m[k]).sum()).collect();
        let mut full = full;
        // Delta parameterization: an ordinal variable's residual variance is what makes its total variance 1. These
        // variables are outcomes only, so their residual variance reaches nothing else.
        for &i in &self.ordinal {
            let common = full[i][i] - s[i][i];
            s[i][i] = 1.0 - common;
            full[i][i] = 1.0;
        }
        Some(Implied { b, full, mean, s })
    }
}

/// The implied moments of every variable in one group.
pub struct Implied {
    pub b: Mat,
    pub full: Mat,
    pub mean: Vec<f64>,
    /// The S matrix (residual variances and covariances).
    pub s: Mat,
}

/// Where a parameter sits: free parameters can sit in several places (equal labels, or both triangles of S).
#[derive(Debug, Clone, Copy)]
pub struct Place {
    pub group: usize,
    pub mat: Matrix,
    pub i: usize,
    pub j: usize,
}

impl Implied {
    /// ∂Σ (observed block, p×p) and ∂μ of one place.
    pub fn derivative(&self, place: &Place, p: usize, out_cov: &mut Mat, out_mean: &mut [f64]) {
        let (b, f) = (&self.b, &self.full);
        let (i, j) = (place.i, place.j);
        match place.mat {
            Matrix::A => {
                for r in 0..p {
                    for c in 0..p {
                        out_cov[r][c] += b[r][i] * f[j][c] + b[c][i] * f[j][r];
                    }
                    out_mean[r] += b[r][i] * self.mean[j];
                }
            }
            Matrix::S => {
                for r in 0..p {
                    for c in 0..p {
                        out_cov[r][c] += b[r][i] * b[c][j] + if i != j { b[r][j] * b[c][i] } else { 0.0 };
                    }
                }
            }
            Matrix::M => {
                for r in 0..p {
                    out_mean[r] += b[r][i];
                }
            }
            Matrix::T => {}
        }
    }
}
