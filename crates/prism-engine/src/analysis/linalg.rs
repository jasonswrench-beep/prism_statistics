//! Small dense matrix helpers for the multivariate analyses (repeated measures, MANOVA, factor analysis). The matrices
//! are a few dozen rows and columns at most, so plain row-major vectors and textbook algorithms are enough.

pub type Mat = Vec<Vec<f64>>;

pub fn transpose(a: &Mat) -> Mat {
    if a.is_empty() {
        return Vec::new();
    }
    (0..a[0].len()).map(|j| a.iter().map(|r| r[j]).collect()).collect()
}

pub fn mul(a: &Mat, b: &Mat) -> Mat {
    a.iter().map(|r| (0..b[0].len()).map(|j| r.iter().zip(b).map(|(x, row)| x * row[j]).sum()).collect()).collect()
}

pub fn identity(n: usize) -> Mat {
    (0..n).map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect()
}

/// The inverse by Gauss-Jordan elimination with partial pivoting; None when singular.
pub fn invert(a: &Mat) -> Option<Mat> {
    let n = a.len();
    let scale = a.iter().flatten().fold(0.0f64, |m, v| m.max(v.abs())).max(1e-300);
    let mut m: Mat = a.iter().enumerate().map(|(i, r)| r.iter().copied().chain((0..n).map(|j| if i == j { 1.0 } else { 0.0 })).collect()).collect();
    for c in 0..n {
        let piv = (c..n).max_by(|&x, &y| m[x][c].abs().total_cmp(&m[y][c].abs()))?;
        if m[piv][c].abs() < 1e-12 * scale {
            return None;
        }
        m.swap(c, piv);
        let d = m[c][c];
        for v in m[c].iter_mut() {
            *v /= d;
        }
        let pivot_row = m[c].clone();
        for (r, row) in m.iter_mut().enumerate() {
            if r != c {
                let f = row[c];
                if f != 0.0 {
                    for (v, pv) in row.iter_mut().zip(&pivot_row) {
                        *v -= f * pv;
                    }
                }
            }
        }
    }
    Some(m.into_iter().map(|r| r[n..].to_vec()).collect())
}

/// The determinant by elimination.
pub fn det(a: &Mat) -> f64 {
    let n = a.len();
    let mut m = a.clone();
    let mut d = 1.0;
    for c in 0..n {
        let Some(piv) = (c..n).max_by(|&x, &y| m[x][c].abs().total_cmp(&m[y][c].abs())) else { return 0.0 };
        if m[piv][c] == 0.0 {
            return 0.0;
        }
        if piv != c {
            m.swap(c, piv);
            d = -d;
        }
        d *= m[c][c];
        let pivot = m[c].clone();
        for row in m.iter_mut().skip(c + 1) {
            let f = row[c] / pivot[c];
            for (v, pv) in row[c..].iter_mut().zip(&pivot[c..]) {
                *v -= f * pv;
            }
        }
    }
    d
}

pub fn trace(a: &Mat) -> f64 {
    (0..a.len()).map(|i| a[i][i]).sum()
}

/// The lower-triangular Cholesky factor of a symmetric positive-definite matrix; None otherwise.
pub fn cholesky(a: &Mat) -> Option<Mat> {
    let n = a.len();
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let s: f64 = (0..j).map(|k| l[i][k] * l[j][k]).sum();
            if i == j {
                let d = a[i][i] - s;
                if d <= 0.0 {
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

/// Eigenvalues and eigenvectors of a symmetric matrix by cyclic Jacobi rotations, largest value first. The vectors are
/// the columns of the second matrix.
pub fn symmetric_eigen(a: &Mat) -> (Vec<f64>, Mat) {
    let n = a.len();
    let mut m = a.clone();
    let mut v = identity(n);
    let mut prev = f64::INFINITY;
    for _ in 0..100 {
        let off: f64 = (0..n).flat_map(|i| (0..n).filter(move |&j| j != i).map(move |j| (i, j))).map(|(i, j)| m[i][j] * m[i][j]).sum();
        if off < 1e-30 {
            break;
        }
        // For large entries the absolute test is never met: stop once the off-diagonal mass is at rounding level
        // relative to the diagonal and a sweep no longer halves it.
        let diag: f64 = (0..n).map(|i| m[i][i] * m[i][i]).sum();
        if off < 1e-24 * diag && off > 0.5 * prev {
            break;
        }
        prev = off;
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
                for (k, (mpk, mqk)) in rp.into_iter().zip(rq).enumerate() {
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
    let vectors = (0..n).map(|r| order.iter().map(|&i| v[r][i]).collect()).collect();
    (values, vectors)
}

/// The eigenvalues of E⁻¹H for symmetric H and positive-definite E (the roots of a multivariate test), largest first.
pub fn relative_eigenvalues(h: &Mat, e: &Mat) -> Option<Vec<f64>> {
    let l = cholesky(e)?;
    let li = invert(&l)?;
    let s = mul(&li, &mul(h, &transpose(&li)));
    let sym: Mat = (0..s.len()).map(|i| (0..s.len()).map(|j| (s[i][j] + s[j][i]) / 2.0).collect()).collect();
    Some(symmetric_eigen(&sym).0)
}

/// The Moore-Penrose inverse of a symmetric matrix, dropping eigenvalues below √ε times the largest (as R's
/// MASS::ginv).
pub fn pseudo_inverse_sym(a: &Mat) -> Mat {
    let n = a.len();
    let (vals, vecs) = symmetric_eigen(a);
    let top = vals.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let tol = f64::EPSILON.sqrt() * top;
    let mut out = vec![vec![0.0; n]; n];
    for (k, &l) in vals.iter().enumerate() {
        if l.abs() > tol {
            for i in 0..n {
                for j in 0..n {
                    out[i][j] += vecs[i][k] * vecs[j][k] / l;
                }
            }
        }
    }
    out
}
