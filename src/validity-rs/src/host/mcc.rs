//! Minutia Cylinder-Code (Cappelli, Ferrara, Maltoni, IEEE PAMI 2010): every minutia
//! gets a local 3-D descriptor of its neighbours; two prints are compared by local
//! similarity sort with relaxation (LSS-R), which only rewards pairs whose relative
//! geometry agrees.

use std::f32::consts::{PI, TAU};

use super::minutiae::{Features, Minutia, angle_diff};

#[derive(Clone, Debug)]
pub struct Params {
    pub r: f32,
    pub ns: usize,
    pub nd: usize,
    pub sigma_s: f32,
    pub sigma_d: f32,
    pub mu_psi: f32,
    pub tau_psi: f32,
    /// Minimum fraction of valid cells for a cylinder to be used.
    pub min_vc: f32,
    /// Minimum number of other minutiae within reach for a cylinder to be used.
    pub min_m: usize,
    /// Minimum fraction of cells valid in both cylinders for a comparison.
    pub min_me: f32,
    pub delta_theta: f32,
    pub min_np: usize,
    pub max_np: usize,
    pub mu_p: f32,
    pub tau_p: f32,
    pub n_rel: usize,
    pub w_r: f32,
    /// Treat minutia directions as orientations (mod pi): robust to the direction flips
    /// that bifurcations and short traces cause, at some loss of distinctiveness.
    pub axial: bool,
    /// Expected minutia position error (pixels): widens the relaxation's distance and
    /// radial-angle tolerances (the published values assume ~2 px extractors).
    pub pos_tol: f32,
}

impl Params {
    /// Published 500 dpi parameters scaled by `scale` (sensor pixels per 500 dpi pixel).
    pub fn scaled(scale: f32) -> Self {
        Self {
            r: 70.0 * scale,
            ns: 16,
            nd: 6,
            sigma_s: 28.0 / 3.0 * scale,
            sigma_d: 2.0 * PI / 9.0,
            mu_psi: 0.01 / scale,
            tau_psi: 400.0 * scale,
            min_vc: 0.75,
            min_m: 2,
            min_me: 0.6,
            delta_theta: PI / 4.0,
            min_np: 4,
            max_np: 12,
            mu_p: 20.0,
            tau_p: 0.4,
            n_rel: 5,
            w_r: 0.5,
            axial: true,
            pos_tol: 6.0 * scale,
        }
    }
}

pub struct Cylinder {
    pub vals: Vec<f32>,
    pub valid: Vec<bool>,
}

pub struct Template {
    pub minutiae: Vec<Minutia>,
    pub cyl: Vec<Option<Cylinder>>,
}

fn erf(x: f32) -> f32 {
    // Abramowitz-Stegun 7.1.26, |error| < 1.5e-7
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
    let y = 1.0 - (((((1.061_405_4 * t - 1.453_152_1) * t) + 1.421_413_8) * t - 0.284_496_74) * t + 0.254_829_6) * t * (-x * x).exp();
    if x >= 0.0 { y } else { -y }
}

/// Direction difference, wrapped to (-pi, pi] or, in axial mode, to (-pi/2, pi/2].
fn ddir(a: f32, b: f32, axial: bool) -> f32 {
    let d = angle_diff(a, b);
    if !axial {
        d
    } else if d > PI / 2.0 {
        d - PI
    } else if d <= -PI / 2.0 {
        d + PI
    } else {
        d
    }
}

fn sigmoid(v: f32, mu: f32, tau: f32) -> f32 {
    1.0 / (1.0 + (-tau * (v - mu)).exp())
}

pub fn build(f: &Features, p: &Params) -> Template {
    let (ns, nd) = (p.ns, p.nd);
    let ds = 2.0 * p.r / ns as f32;
    let span = if p.axial { PI } else { TAU };
    let dd = span / nd as f32;
    let gs_norm = 1.0 / (p.sigma_s * (TAU).sqrt());
    let inside = |x: f32, y: f32| {
        let (xi, yi) = (x.round() as isize, y.round() as isize);
        xi >= 0 && yi >= 0 && (xi as usize) < f.w && (yi as usize) < f.h && f.mask[yi as usize * f.w + xi as usize]
    };
    let ms = &f.minutiae;
    let cyl = ms
        .iter()
        .enumerate()
        .map(|(mi, m)| {
            let near: Vec<&Minutia> = ms
                .iter()
                .enumerate()
                .filter(|(j, t)| *j != mi && (t.x - m.x).hypot(t.y - m.y) <= p.r + 3.0 * p.sigma_s)
                .map(|(_, t)| t)
                .collect();
            if near.len() < p.min_m {
                return None;
            }
            let (c, s) = (m.dir.cos(), m.dir.sin());
            let mut vals = vec![0.0f32; ns * ns * nd];
            let mut valid = vec![false; ns * ns * nd];
            let (mut n_valid, mut n_in_r) = (0usize, 0usize);
            for i in 0..ns {
                for j in 0..ns {
                    let (u, v) = ((i as f32 + 0.5 - ns as f32 / 2.0) * ds, (j as f32 + 0.5 - ns as f32 / 2.0) * ds);
                    let (px, py) = (m.x + c * u - s * v, m.y + s * u + c * v);
                    if u.hypot(v) > p.r {
                        continue;
                    }
                    n_in_r += 1;
                    if !inside(px, py) {
                        continue;
                    }
                    n_valid += 1;
                    for k in 0..nd {
                        let phi_k = -span / 2.0 + (k as f32 + 0.5) * dd;
                        let mut sum = 0.0;
                        for t in &near {
                            let d = (t.x - px).hypot(t.y - py);
                            if d > 3.0 * p.sigma_s {
                                continue;
                            }
                            let cs = gs_norm * (-(d * d) / (2.0 * p.sigma_s * p.sigma_s)).exp();
                            let alpha = ddir(phi_k, ddir(m.dir, t.dir, p.axial), p.axial);
                            let q = p.sigma_d * std::f32::consts::SQRT_2;
                            let cd = 0.5 * (erf((alpha + dd / 2.0) / q) - erf((alpha - dd / 2.0) / q));
                            sum += cs * cd;
                        }
                        let idx = (i * ns + j) * nd + k;
                        vals[idx] = sigmoid(sum, p.mu_psi, p.tau_psi);
                        valid[idx] = true;
                    }
                }
            }
            if (n_valid as f32) < p.min_vc * n_in_r as f32 {
                return None;
            }
            Some(Cylinder { vals, valid })
        })
        .collect();
    Template { minutiae: ms.clone(), cyl }
}

pub fn cyl_sim(a: &Cylinder, b: &Cylinder, p: &Params) -> f32 {
    let (mut d2, mut na, mut nb, mut n) = (0.0f32, 0.0f32, 0.0f32, 0usize);
    for i in 0..a.vals.len() {
        if a.valid[i] && b.valid[i] {
            let (x, y) = (a.vals[i], b.vals[i]);
            d2 += (x - y) * (x - y);
            na += x * x;
            nb += y * y;
            n += 1;
        }
    }
    if (n as f32) < p.min_me * a.vals.len() as f32 * 0.5 || na + nb == 0.0 {
        return 0.0;
    }
    let s = 1.0 - d2.sqrt() / (na.sqrt() + nb.sqrt());
    s.max(0.0)
}

/// Result of a comparison: the LSS-R score and how many minutia pairs it rests on.
#[derive(Clone, Copy, Debug, Default)]
pub struct Score {
    pub score: f32,
    pub pairs: usize,
    pub probe_cyl: usize,
}

pub fn compare(a: &Template, b: &Template, p: &Params) -> Score {
    compare_inner(a, b, p, None)
}

/// Pairs chosen by the matcher as (probe index, gallery index, local similarity, after relaxation).
pub fn explain(a: &Template, b: &Template, p: &Params) -> (Score, Vec<(usize, usize, f32, f32)>) {
    let mut out = Vec::new();
    let s = compare_inner(a, b, p, Some(&mut out));
    (s, out)
}

fn compare_inner(a: &Template, b: &Template, p: &Params, explain: Option<&mut Vec<(usize, usize, f32, f32)>>) -> Score {
    let ia: Vec<usize> = (0..a.cyl.len()).filter(|&i| a.cyl[i].is_some()).collect();
    let ib: Vec<usize> = (0..b.cyl.len()).filter(|&i| b.cyl[i].is_some()).collect();
    let probe_cyl = ia.len();
    if ia.len() < 2 || ib.len() < 2 {
        return Score { probe_cyl, ..Score::default() };
    }
    let mut sims = Vec::with_capacity(ia.len() * ib.len());
    for &i in &ia {
        for &j in &ib {
            let (ma, mb) = (&a.minutiae[i], &b.minutiae[j]);
            if ddir(ma.dir, mb.dir, p.axial).abs() > p.delta_theta {
                continue;
            }
            let s = cyl_sim(a.cyl[i].as_ref().unwrap(), b.cyl[j].as_ref().unwrap(), p);
            if s > 0.0 {
                sims.push((s, i, j));
            }
        }
    }
    sims.sort_by(|x, y| y.0.total_cmp(&x.0));
    // one-to-one pairs, best first
    let n_r = ia.len().min(ib.len());
    let (mut used_a, mut used_b) = (vec![false; a.cyl.len()], vec![false; b.cyl.len()]);
    let mut pairs = Vec::new();
    for &(s, i, j) in &sims {
        if !used_a[i] && !used_b[j] {
            used_a[i] = true;
            used_b[j] = true;
            pairs.push((s, i, j));
            if pairs.len() == n_r {
                break;
            }
        }
    }
    let n_p = (p.min_np as f32 + (sigmoid(n_r as f32, p.mu_p, p.tau_p) * (p.max_np - p.min_np) as f32).round()) as usize;
    if pairs.len() < 2 {
        return Score { probe_cyl, ..Score::default() };
    }
    // relaxation: a pair keeps its similarity only if its geometry agrees with the others
    let rho = |t: usize, k: usize| -> f32 {
        let (a1, b1) = (&a.minutiae[pairs[t].1], &b.minutiae[pairs[t].2]);
        let (a2, b2) = (&a.minutiae[pairs[k].1], &b.minutiae[pairs[k].2]);
        let d1 = ((a1.x - a2.x).hypot(a1.y - a2.y) - (b1.x - b2.x).hypot(b1.y - b2.y)).abs();
        let d2 = ddir(ddir(a1.dir, a2.dir, p.axial), ddir(b1.dir, b2.dir, p.axial), p.axial).abs();
        // radial angle in image coordinates (y down), the same frame as the minutia directions
        let rad = |m1: &Minutia, m2: &Minutia| ddir(m1.dir, (m2.y - m1.y).atan2(m2.x - m1.x), p.axial);
        let d3 = ddir(rad(a1, a2), rad(b1, b2), p.axial).abs();
        // tolerances grow with the extractor's position error and (for distortion) with distance
        let dist = (a1.x - a2.x).hypot(a1.y - a2.y).max(1.0);
        let mu1 = p.pos_tol.max(5.0) + 0.05 * dist;
        let mu3 = PI / 12.0 + p.pos_tol.atan2(dist);
        sigmoid(d1, mu1, -8.0 / mu1) * sigmoid(d2, PI / 12.0, -30.0) * sigmoid(d3, mu3, -30.0)
    };
    let n = pairs.len();
    let rho_m: Vec<f32> = (0..n * n).map(|i| if i / n == i % n { 0.0 } else { rho(i / n, i % n) }).collect();
    let lam0: Vec<f32> = pairs.iter().map(|x| x.0).collect();
    let mut lam = lam0.clone();
    for _ in 0..p.n_rel {
        let prev = lam.clone();
        for t in 0..n {
            let s: f32 = (0..n).map(|k| rho_m[t * n + k] * prev[k]).sum();
            lam[t] = p.w_r * prev[t] + (1.0 - p.w_r) * s / (n - 1) as f32;
        }
    }
    if let Some(out) = explain {
        out.extend((0..n).map(|t| (pairs[t].1, pairs[t].2, lam0[t], lam[t])));
    }
    let mut eff: Vec<(f32, f32)> = (0..n).map(|t| (lam[t] / lam0[t].max(1e-9), lam[t])).collect();
    eff.sort_by(|x, y| y.0.total_cmp(&x.0));
    let take = n_p.min(n);
    let score = eff[..take].iter().map(|e| e.1).sum::<f32>() / take as f32;
    // pairs that still carry weight after relaxation
    let strong = lam.iter().filter(|&&l| l > 0.25 * lam0.iter().cloned().fold(0.0, f32::max)).count();
    Score { score, pairs: strong, probe_cyl }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::minutiae::Kind;

    fn feats(ms: Vec<Minutia>, w: usize, h: usize) -> Features {
        Features { w, h, minutiae: ms, mask: vec![true; w * h], period: 9.0 }
    }

    fn random_set(n: usize, seed: u32, w: f32, h: f32) -> Vec<Minutia> {
        let mut s = seed;
        let mut r = || {
            s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            (s >> 8) as f32 / (1u32 << 24) as f32
        };
        (0..n).map(|_| Minutia { x: 20.0 + r() * (w - 40.0), y: 20.0 + r() * (h - 40.0), dir: r() * TAU, kind: Kind::Ending, quality: 1.0 }).collect()
    }

    #[test]
    fn same_print_rotated_scores_high_different_print_low() {
        let p = Params::scaled(1.0);
        let a = random_set(40, 7, 300.0, 300.0);
        // b = a rotated by 20 degrees and shifted, with a little jitter
        let (c, s) = ((20.0f32).to_radians().cos(), (20.0f32).to_radians().sin());
        let b: Vec<Minutia> = a
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let (x, y) = (m.x - 150.0, m.y - 150.0);
                Minutia { x: 150.0 + c * x - s * y + 10.0 + (i % 3) as f32, y: 150.0 + s * x + c * y - 5.0, dir: (m.dir + 20f32.to_radians()).rem_euclid(TAU), ..*m }
            })
            .collect();
        let other = random_set(40, 99, 300.0, 300.0);
        let (ta, tb, to) = (build(&feats(a, 340, 340), &p), build(&feats(b, 340, 340), &p), build(&feats(other, 340, 340), &p));
        let same = compare(&ta, &tb, &p);
        let diff = compare(&ta, &to, &p);
        assert!(same.score > 0.5, "same {:?}", same);
        assert!(diff.score < 0.25, "different {:?}", diff);
    }
}

