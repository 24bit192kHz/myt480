//! 2-D FFT on top of rustfft and the cross-correlation used for frame registration.

use std::sync::Arc;

use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};

use super::img::Img;

pub struct Fft2 {
    pub w: usize,
    pub h: usize,
    fw: Arc<dyn Fft<f32>>,
    iw: Arc<dyn Fft<f32>>,
    fh: Arc<dyn Fft<f32>>,
    ih: Arc<dyn Fft<f32>>,
}

impl Fft2 {
    pub fn new(w: usize, h: usize) -> Self {
        let mut p = FftPlanner::new();
        Self { w, h, fw: p.plan_fft_forward(w), iw: p.plan_fft_inverse(w), fh: p.plan_fft_forward(h), ih: p.plan_fft_inverse(h) }
    }

    fn run(&self, d: &mut [Complex32], inverse: bool) {
        assert_eq!(d.len(), self.w * self.h);
        let (rows, cols) = if inverse { (&self.iw, &self.ih) } else { (&self.fw, &self.fh) };
        for r in d.chunks_exact_mut(self.w) {
            rows.process(r);
        }
        let mut col = vec![Complex32::default(); self.h];
        for x in 0..self.w {
            for y in 0..self.h {
                col[y] = d[y * self.w + x];
            }
            cols.process(&mut col);
            for y in 0..self.h {
                d[y * self.w + x] = col[y];
            }
        }
        if inverse {
            let s = 1.0 / (self.w * self.h) as f32;
            d.iter_mut().for_each(|v| *v *= s);
        }
    }

    pub fn forward(&self, d: &mut [Complex32]) {
        self.run(d, false)
    }

    pub fn inverse(&self, d: &mut [Complex32]) {
        self.run(d, true)
    }

    /// Forward transform of `img` zero-padded into this transform's size.
    pub fn of(&self, img: &Img) -> Vec<Complex32> {
        let mut d = vec![Complex32::default(); self.w * self.h];
        for y in 0..img.h.min(self.h) {
            for x in 0..img.w.min(self.w) {
                d[y * self.w + x] = Complex32::new(img.get(x, y), 0.0);
            }
        }
        self.forward(&mut d);
        d
    }

    pub fn real(&self, d: &[Complex32]) -> Img {
        Img { w: self.w, h: self.h, px: d.iter().map(|c| c.re).collect() }
    }
}

/// Signed frequency (cycles per pixel) of FFT bin `i` of an `n`-point transform.
pub fn freq(i: usize, n: usize) -> f32 {
    let i = i as isize;
    let n = n as isize;
    (if i <= (n - 1) / 2 { i } else { i - n }) as f32 / n as f32
}

/// Displacement `d` of b's content relative to a's (b(x) ~ a(x - d)) with a normalised
/// peak score. Plain cross-correlation, zero-padded by `fft` (must be >= 2x the images).
pub fn xcorr(fft: &Fft2, a: &Img, b: &Img) -> (isize, isize, f32) {
    let fa = fft.of(a);
    let mut fb = fft.of(b);
    for (vb, va) in fb.iter_mut().zip(&fa) {
        *vb *= va.conj();
    }
    fft.inverse(&mut fb);
    let (mut best, mut bi) = (f32::MIN, 0);
    for (i, v) in fb.iter().enumerate() {
        if v.re > best {
            best = v.re;
            bi = i;
        }
    }
    let (y, x) = ((bi / fft.w) as isize, (bi % fft.w) as isize);
    let dy = if y < fft.h as isize / 2 { y } else { y - fft.h as isize };
    let dx = if x < fft.w as isize / 2 { x } else { x - fft.w as isize };
    let ea: f32 = a.px.iter().map(|v| v * v).sum();
    let eb: f32 = b.px.iter().map(|v| v * v).sum();
    (dy, dx, best / ((ea * eb).sqrt() + 1e-9))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let f = Fft2::new(12, 10);
        let img = Img::from_fn(12, 10, |x, y| (x * 3 + y * 7) as f32 % 5.0);
        let mut d = f.of(&img);
        f.inverse(&mut d);
        let back = f.real(&d);
        assert!(img.px.iter().zip(&back.px).all(|(a, b)| (a - b).abs() < 1e-3));
    }

    #[test]
    fn xcorr_finds_shift() {
        let base = Img::from_fn(64, 64, |x, y| ((x as f32 * 0.7).sin() + (y as f32 * 0.31 + x as f32 * 0.05).cos()) * ((x * y) % 7) as f32);
        // b = a shifted by (+3, -5): b(x, y) = a(x + 5, y - 3)
        let a = base.crop(10, 10, 32, 32);
        let b = Img::from_fn(32, 32, |x, y| base.get(10 + x + 5, 10 + y - 3));
        let f = Fft2::new(64, 64);
        let (dy, dx, s) = xcorr(&f, &a, &b);
        assert_eq!((dy, dx), (3, -5));
        assert!(s > 0.3);
    }
}
