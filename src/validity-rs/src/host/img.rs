//! Minimal grayscale image type and the filters the host pipeline needs.

use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

#[derive(Clone, Debug, PartialEq)]
pub struct Img {
    pub w: usize,
    pub h: usize,
    pub px: Vec<f32>,
}

impl Img {
    pub fn new(w: usize, h: usize) -> Self {
        Self { w, h, px: vec![0.0; w * h] }
    }

    pub fn from_fn(w: usize, h: usize, f: impl Fn(usize, usize) -> f32) -> Self {
        let mut px = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                px.push(f(x, y));
            }
        }
        Self { w, h, px }
    }

    pub fn from_u8(w: usize, h: usize, data: &[u8]) -> Self {
        Self { w, h, px: data.iter().map(|&v| v as f32).collect() }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> f32 {
        self.px[y * self.w + x]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, v: f32) {
        self.px[y * self.w + x] = v;
    }

    #[inline]
    pub fn get_clamped(&self, x: isize, y: isize) -> f32 {
        let x = x.clamp(0, self.w as isize - 1) as usize;
        let y = y.clamp(0, self.h as isize - 1) as usize;
        self.px[y * self.w + x]
    }

    /// Bilinear sample with clamped edges.
    pub fn bilinear(&self, x: f32, y: f32) -> f32 {
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (x0, y0) = (x0 as isize, y0 as isize);
        let a = self.get_clamped(x0, y0);
        let b = self.get_clamped(x0 + 1, y0);
        let c = self.get_clamped(x0, y0 + 1);
        let d = self.get_clamped(x0 + 1, y0 + 1);
        a * (1.0 - fx) * (1.0 - fy) + b * fx * (1.0 - fy) + c * (1.0 - fx) * fy + d * fx * fy
    }

    pub fn map(&self, f: impl Fn(f32) -> f32) -> Img {
        Img { w: self.w, h: self.h, px: self.px.iter().map(|&v| f(v)).collect() }
    }

    pub fn zip(&self, o: &Img, f: impl Fn(f32, f32) -> f32) -> Img {
        assert_eq!((self.w, self.h), (o.w, o.h));
        Img { w: self.w, h: self.h, px: self.px.iter().zip(&o.px).map(|(&a, &b)| f(a, b)).collect() }
    }

    pub fn crop(&self, x0: usize, y0: usize, w: usize, h: usize) -> Img {
        Img::from_fn(w, h, |x, y| self.get(x0 + x, y0 + y))
    }

    /// Resample by `scale` (output = input * scale). Downscaling low-passes first
    /// so ridges do not alias.
    pub fn resize(&self, scale: f32) -> Img {
        let src = if scale < 1.0 { blur(self, 0.5 / scale) } else { self.clone() };
        let (w, h) = (((self.w as f32) * scale).round() as usize, ((self.h as f32) * scale).round() as usize);
        Img::from_fn(w, h, |x, y| src.bilinear((x as f32 + 0.5) / scale - 0.5, (y as f32 + 0.5) / scale - 0.5))
    }

    pub fn mean(&self) -> f32 {
        self.px.iter().sum::<f32>() / self.px.len().max(1) as f32
    }

    /// 8-bit rendering, 2-98 percentile stretch over `mask` (or everything);
    /// pixels outside the mask are white.
    pub fn to_u8(&self, mask: Option<&[bool]>) -> Vec<u8> {
        let vals: Vec<f32> = match mask {
            Some(m) => self.px.iter().zip(m).filter(|(_, m)| **m).map(|(v, _)| *v).collect(),
            None => self.px.clone(),
        };
        if vals.is_empty() {
            return vec![255; self.px.len()];
        }
        let (lo, hi) = (percentile(&vals, 2.0), percentile(&vals, 98.0));
        self.px
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                if mask.is_some_and(|m| !m[i]) {
                    255
                } else {
                    (((v - lo) / (hi - lo + 1e-9)) * 255.0).clamp(0.0, 255.0) as u8
                }
            })
            .collect()
    }
}

fn pgm_token<'a>(b: &'a [u8], pos: &mut usize) -> Result<&'a [u8]> {
    loop {
        while *pos < b.len() && b[*pos].is_ascii_whitespace() {
            *pos += 1;
        }
        if *pos < b.len() && b[*pos] == b'#' {
            while *pos < b.len() && b[*pos] != b'\n' {
                *pos += 1;
            }
            continue;
        }
        break;
    }
    let start = *pos;
    while *pos < b.len() && !b[*pos].is_ascii_whitespace() {
        *pos += 1;
    }
    ensure!(*pos > start, "truncated PGM header");
    Ok(&b[start..*pos])
}

/// Parse one binary PGM (P5, 8-bit) starting at `b[0]`; returns the image and the bytes used.
pub fn parse_pgm(b: &[u8]) -> Result<(Img, usize)> {
    let mut pos = 0;
    ensure!(pgm_token(b, &mut pos)? == b"P5", "not a binary PGM (P5)");
    let num = |p: &mut usize| -> Result<usize> {
        Ok(std::str::from_utf8(pgm_token(b, p)?)?.parse::<usize>().context("PGM header number")?)
    };
    let (w, h, max) = (num(&mut pos)?, num(&mut pos)?, num(&mut pos)?);
    ensure!(max > 0 && max < 256, "only 8-bit PGM is supported");
    pos += 1; // the single whitespace after maxval
    let end = pos + w * h;
    ensure!(end <= b.len(), "truncated PGM data");
    Ok((Img::from_u8(w, h, &b[pos..end]), end))
}

pub fn read_pgm(path: impl AsRef<Path>) -> Result<Img> {
    let p = path.as_ref();
    let b = std::fs::read(p).with_context(|| format!("reading {}", p.display()))?;
    Ok(parse_pgm(&b).with_context(|| format!("parsing {}", p.display()))?.0)
}

/// A concatenated PGM stream (what `frame-live` writes).
pub fn read_pgm_seq(path: impl AsRef<Path>) -> Result<Vec<Img>> {
    let p = path.as_ref();
    let b = std::fs::read(p).with_context(|| format!("reading {}", p.display()))?;
    let (mut out, mut pos) = (Vec::new(), 0);
    while pos < b.len() {
        if b[pos..].iter().all(|c| c.is_ascii_whitespace()) {
            break;
        }
        let (img, used) = parse_pgm(&b[pos..])?;
        out.push(img);
        pos += used;
    }
    if out.is_empty() {
        bail!("{}: no frames", p.display());
    }
    Ok(out)
}

pub fn pgm_bytes(w: usize, h: usize, data: &[u8]) -> Vec<u8> {
    let mut out = format!("P5\n{w} {h}\n255\n").into_bytes();
    out.extend_from_slice(data);
    out
}

pub fn write_pgm(path: impl AsRef<Path>, img: &Img, mask: Option<&[bool]>) -> Result<()> {
    let p = path.as_ref();
    std::fs::write(p, pgm_bytes(img.w, img.h, &img.to_u8(mask))).with_context(|| format!("writing {}", p.display()))
}

pub fn gauss_kernel(sigma: f32) -> Vec<f32> {
    let r = (3.0 * sigma).ceil().max(1.0) as isize;
    let mut k: Vec<f32> = (-r..=r).map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp()).collect();
    let s: f32 = k.iter().sum();
    k.iter_mut().for_each(|v| *v /= s);
    k
}

fn convolve_1d(img: &Img, k: &[f32], horizontal: bool) -> Img {
    let r = (k.len() / 2) as isize;
    let mut out = Img::new(img.w, img.h);
    for y in 0..img.h {
        for x in 0..img.w {
            let mut acc = 0.0;
            for (i, &kv) in k.iter().enumerate() {
                let d = i as isize - r;
                acc += kv
                    * if horizontal {
                        img.get_clamped(x as isize + d, y as isize)
                    } else {
                        img.get_clamped(x as isize, y as isize + d)
                    };
            }
            out.px[y * img.w + x] = acc;
        }
    }
    out
}

/// Separable Gaussian blur, clamped edges.
pub fn blur(img: &Img, sigma: f32) -> Img {
    if sigma <= 0.0 {
        return img.clone();
    }
    let k = gauss_kernel(sigma);
    convolve_1d(&convolve_1d(img, &k, true), &k, false)
}

/// Gaussian blur along x only (keeps row structure such as sensor streaks).
pub fn blur_x(img: &Img, sigma: f32) -> Img {
    convolve_1d(img, &gauss_kernel(sigma), true)
}

/// Normalised masked blur: average of in-mask neighbours only.
pub fn blur_masked(img: &Img, mask: &[bool], sigma: f32) -> Img {
    let m = Img { w: img.w, h: img.h, px: mask.iter().map(|&b| b as u8 as f32).collect() };
    let num = blur(&img.zip(&m, |a, b| a * b), sigma);
    let den = blur(&m, sigma);
    num.zip(&den, |a, b| if b > 1e-6 { a / b } else { 0.0 })
}

pub fn percentile(v: &[f32], p: f32) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    let k = (((p / 100.0) * (s.len() - 1) as f32).round() as usize).min(s.len() - 1);
    let (_, val, _) = s.select_nth_unstable_by(k, |a, b| a.total_cmp(b));
    *val
}

pub fn median(v: &[f32]) -> f32 {
    percentile(v, 50.0)
}

/// Square erosion of a boolean mask by `r` pixels (separable min filter).
pub fn erode(mask: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    let pass = |src: &[bool], horizontal: bool| -> Vec<bool> {
        let mut out = vec![false; w * h];
        for y in 0..h {
            for x in 0..w {
                let ok = (0..=2 * r).all(|i| {
                    let d = i as isize - r as isize;
                    let (xx, yy) = if horizontal { (x as isize + d, y as isize) } else { (x as isize, y as isize + d) };
                    xx >= 0 && yy >= 0 && (xx as usize) < w && (yy as usize) < h && src[yy as usize * w + xx as usize]
                });
                out[y * w + x] = ok;
            }
        }
        out
    };
    pass(&pass(mask, true), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pgm_roundtrip_and_sequence() {
        let a = Img::from_fn(5, 3, |x, y| (x * 10 + y) as f32);
        let mut seq = pgm_bytes(5, 3, &a.px.iter().map(|&v| v as u8).collect::<Vec<_>>());
        seq.extend(pgm_bytes(5, 3, &[7; 15]));
        let (b, used) = parse_pgm(&seq).unwrap();
        assert_eq!(b, a);
        let (c, _) = parse_pgm(&seq[used..]).unwrap();
        assert!(c.px.iter().all(|&v| v == 7.0));
        assert!(parse_pgm(b"P5\n# comment\n2 1\n255\n\x01\x02").is_ok());
    }

    #[test]
    fn blur_preserves_constant_and_mean() {
        let a = Img::from_fn(20, 20, |_, _| 5.0);
        assert!(blur(&a, 2.0).px.iter().all(|v| (v - 5.0).abs() < 1e-4));
        let b = Img::from_fn(31, 31, |x, y| if x == 15 && y == 15 { 1.0 } else { 0.0 });
        assert!((blur(&b, 2.0).px.iter().sum::<f32>() - 1.0).abs() < 1e-3);
    }

    #[test]
    fn resize_and_percentile() {
        let a = Img::from_fn(40, 20, |x, _| x as f32);
        let b = a.resize(0.5);
        assert_eq!((b.w, b.h), (20, 10));
        assert!((percentile(&[1.0, 2.0, 3.0, 4.0, 5.0], 50.0) - 3.0).abs() < 1e-6);
        let m = erode(&[true; 25], 5, 5, 1);
        assert_eq!(m.iter().filter(|&&v| v).count(), 9);
    }
}
