//! Synthetic fingerprints with known minutiae (ridge dislocations on a curved ridge
//! field, plus blur, noise and uneven contrast), to measure the extractor's recall and
//! precision against ground truth.

use std::f32::consts::TAU;

use super::img::{Img, blur};
use super::minutiae::{self, Params};

pub struct Synth {
    pub img: Img,
    pub truth: Vec<(f32, f32)>,
}

fn rng(seed: &mut u64) -> f32 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*seed >> 33) as f32) / (1u64 << 31) as f32
}

pub fn make(w: usize, h: usize, period: f32, n: usize, noise: f32, seed: u64) -> Synth {
    let mut s = seed;
    let a0 = rng(&mut s) * TAU;
    let curv = (rng(&mut s) - 0.5) * 0.004;
    let mut truth: Vec<(f32, f32, f32)> = Vec::new();
    let mut guard = 0;
    while truth.len() < n && guard < 5000 {
        guard += 1;
        let (x, y) = (period * 3.0 + rng(&mut s) * (w as f32 - 6.0 * period), period * 3.0 + rng(&mut s) * (h as f32 - 6.0 * period));
        if truth.iter().all(|t| (t.0 - x).hypot(t.1 - y) > 3.0 * period) {
            truth.push((x, y, if rng(&mut s) < 0.5 { 1.0 } else { -1.0 }));
        }
    }
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let raw = Img::from_fn(w, h, |x, y| {
        let (x, y) = (x as f32, y as f32);
        let base = (x * a0.cos() + y * a0.sin()) + curv * ((x - cx).powi(2) - (y - cy).powi(2));
        let ph = TAU * base / period + truth.iter().map(|t| t.2 * (y - t.1).atan2(x - t.0)).sum::<f32>();
        ph.cos()
    });
    let mut img = blur(&raw, 0.8);
    let contrast = blur(&Img::from_fn(w, h, |_, _| 0.0), 1.0); // placeholder for shape
    let _ = contrast;
    let mut s2 = seed ^ 0x9e37_79b9;
    for (i, v) in img.px.iter_mut().enumerate() {
        let (x, y) = ((i % w) as f32, (i / w) as f32);
        let gain = 0.6 + 0.4 * ((x / 37.0).sin() * (y / 53.0).cos()).abs();
        *v = 128.0 + 60.0 * gain * *v + noise * 60.0 * (rng(&mut s2) - 0.5) * 2.0;
    }
    Synth { img, truth: truth.iter().map(|t| (t.0, t.1)).collect() }
}

/// Recall and precision of `extract` on synthetic prints.
pub fn score(p: &Params, noise: f32, runs: u64) -> (f32, f32, f32) {
    score_tol(p, noise, runs, 6.0)
}

pub fn score_tol(p: &Params, noise: f32, runs: u64, tol: f32) -> (f32, f32, f32) {
    let (mut tp, mut truth_n, mut found_n) = (0usize, 0usize, 0usize);
    for seed in 1..=runs {
        let sy = make(300, 300, 9.0, 12, noise, seed);
        let band = minutiae::band(&sy.img, 9.0);
        let f = minutiae::extract(&band, &vec![true; 300 * 300], p);
        // ground truth inside the extractor's usable area only
        let inside: Vec<&(f32, f32)> = sy.truth.iter().filter(|t| f.mask[t.1 as usize * 300 + t.0 as usize]).collect();
        truth_n += inside.len();
        found_n += f.minutiae.len();
        tp += inside.iter().filter(|t| f.minutiae.iter().any(|m| (m.x - t.0).hypot(m.y - t.1) < tol)).count();
        // false detections: not near any true minutia (inside or not)
        found_n -= f.minutiae.iter().filter(|m| sy.truth.iter().any(|t| (m.x - t.0).hypot(m.y - t.1) < tol) && !inside.iter().any(|t| (m.x - t.0).hypot(m.y - t.1) < tol)).count();
    }
    let recall = tp as f32 / truth_n.max(1) as f32;
    let precision = tp as f32 / found_n.max(1) as f32;
    (recall, precision, found_n as f32 / runs as f32)
}

pub fn cmd() -> anyhow::Result<()> {
    let base = Params::default();
    let variants: Vec<(&str, Params)> = vec![
        ("defaults", base.clone()),
        ("no spur filter", Params { spur_filter: false, ..base.clone() }),
        ("no coherence gate", Params { min_coherence: 0.0, ..base.clone() }),
        ("no pair removal", Params { min_pair_dist: 0.0, ..base.clone() }),
        ("none of the three", Params { spur_filter: false, min_coherence: 0.0, min_pair_dist: 0.0, ..base.clone() }),
    ];
    for tol in [4.0f32, 6.0, 9.0, 12.0] {
        let (r, pr, n) = score_tol(&Params { spur_filter: false, ..base.clone() }, 0.0, 12, tol);
        println!("no spur filter, clean, tolerance {tol:4.1} px: recall {:5.1}% precision {:5.1}% ({n:.1}/print)", r * 100.0, pr * 100.0);
    }
    for (name, p) in &variants {
        let (r0, p0, n0) = score(p, 0.0, 12);
        let (r1, p1, n1) = score(p, 0.6, 12);
        println!("{name:<18} clean: recall {:5.1}% precision {:5.1}% ({n0:4.1}/print) | noisy: recall {:5.1}% precision {:5.1}% ({n1:4.1}/print)", r0 * 100.0, p0 * 100.0, r1 * 100.0, p1 * 100.0);
    }
    Ok(())
}
