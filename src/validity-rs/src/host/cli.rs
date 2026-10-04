//! Diagnostic commands for the host pipeline (`validity-rs host-*`).

use std::path::Path;

use anyhow::{Result, bail};

use super::img::{Img, read_pgm, read_pgm_seq, write_pgm};
use super::minutiae::{self, Features, Kind};
use super::mosaic::{self, Calib, FrameProc, StitchParams};

/// Colour overlay of minutiae on an image (binary PPM).
pub fn write_overlay(path: impl AsRef<Path>, img: &Img, mask: Option<&[bool]>, f: &Features, scale: usize) -> Result<()> {
    let g = img.to_u8(mask);
    let (w, h) = (img.w * scale, img.h * scale);
    let mut rgb = vec![0u8; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            let v = g[(y / scale) * img.w + x / scale];
            rgb[(y * w + x) * 3..(y * w + x) * 3 + 3].copy_from_slice(&[v, v, v]);
        }
    }
    let mut put = |x: f32, y: f32, c: [u8; 3]| {
        let (x, y) = (x.round() as isize, y.round() as isize);
        if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
            let i = (y as usize * w + x as usize) * 3;
            rgb[i..i + 3].copy_from_slice(&c);
        }
    };
    for m in &f.minutiae {
        let c = if m.kind == Kind::Ending { [230, 30, 30] } else { [30, 90, 240] };
        let (cx, cy) = ((m.x + 0.5) * scale as f32, (m.y + 0.5) * scale as f32);
        let r = 2.2 * scale as f32;
        for k in 0..48 {
            let a = k as f32 / 48.0 * std::f32::consts::TAU;
            put(cx + r * a.cos(), cy + r * a.sin(), c);
        }
        for t in 0..(4 * scale) {
            let d = r + t as f32;
            put(cx + d * m.dir.cos(), cy + d * m.dir.sin(), c);
        }
    }
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    out.extend(rgb);
    std::fs::write(path, out)?;
    Ok(())
}

/// `host-mosaic VIDEO.pgmseq OUTPREFIX`: calibrate, stitch, stack, extract minutiae.
pub fn mosaic_cmd(input: &str, prefix: &str) -> Result<()> {
    let t0 = std::time::Instant::now();
    let frames = read_pgm_seq(input)?;
    let (w, h) = (frames[0].w, frames[0].h);
    let raw_proc = FrameProc::new(w, h, true);
    let calib = Calib::estimate(&raw_proc, &frames);
    let proc = FrameProc::new(w, h, calib.is_none());
    let st = mosaic::stitch(&proc, &frames, calib.as_ref(), &StitchParams::default());
    if st.placed.is_empty() {
        bail!("{} frames, none with enough finger", frames.len());
    }
    let m = mosaic::stack(&st.placed, Some(2.5));
    let t1 = t0.elapsed();
    let f = minutiae::extract(&m.img, &m.mask, &minutiae::Params::default());
    let t2 = t0.elapsed();
    write_pgm(format!("{prefix}-mosaic.pgm"), &m.img, Some(&m.mask))?;
    write_overlay(format!("{prefix}-minutiae.ppm"), &m.img, Some(&m.mask), &f, 4)?;
    println!(
        "{} frames, {} with finger, {} stacked ({}), mosaic {}x{} = {:.1}x one frame; ridge period {:.1} px; {} minutiae; stitch {} ms, minutiae {} ms",
        frames.len(),
        st.with_finger,
        st.placed.len(),
        if calib.is_some() { "dark+flat calibrated" } else { "no calibration" },
        m.img.w,
        m.img.h,
        m.area() as f32 / (w * h) as f32,
        f.period,
        f.minutiae.len(),
        t1.as_millis(),
        (t2 - t1).as_millis()
    );
    Ok(())
}

/// `host-minutiae IMAGE.pgm OUT.ppm`: minutiae of a plain fingerprint image.
pub fn minutiae_cmd(input: &str, out: &str) -> Result<()> {
    let img = read_pgm(input)?;
    let t0 = std::time::Instant::now();
    let p0 = minutiae::ridge_period(&img, &vec![true; img.w * img.h]).clamp(5.0, 16.0);
    let fg = minutiae::foreground(&img, p0);
    let band = minutiae::band(&img, p0);
    let f = minutiae::extract(&band, &fg, &minutiae::Params::default());
    println!("{}x{}, period {:.1} px, {} minutiae in {} ms", img.w, img.h, f.period, f.minutiae.len(), t0.elapsed().as_millis());
    write_overlay(out, &band, Some(&fg), &f, 2)
}

/// `host-selfcheck IMAGE.pgm`: the image against a rotated/shifted copy of itself.
/// Reports how many minutiae survive the transform (extraction stability) and the
/// MCC score (matcher), so the two can be told apart.
pub fn selfcheck_cmd(input: &str) -> Result<()> {
    use super::mcc;
    let img = read_pgm(input)?;
    let (w, h) = (img.w, img.h);
    let (ang, tx, ty) = (8f32.to_radians(), 12.0f32, -7.0f32);
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let (c, s) = (ang.cos(), ang.sin());
    // b(x) = a(R^-1 (x - t - c) + c)
    let rot = Img::from_fn(w, h, |x, y| {
        let (dx, dy) = (x as f32 - tx - cx, y as f32 - ty - cy);
        let (sx, sy) = (c * dx + s * dy + cx, -s * dx + c * dy + cy);
        if sx < 0.0 || sy < 0.0 || sx > (w - 1) as f32 || sy > (h - 1) as f32 { 255.0 } else { img.bilinear(sx, sy) }
    });
    let prep = |im: &Img| {
        let p0 = minutiae::ridge_period(im, &vec![true; im.w * im.h]).clamp(5.0, 16.0);
        let fg = minutiae::foreground(im, p0);
        minutiae::extract(&minutiae::band(im, p0), &fg, &minutiae::Params::default())
    };
    let (fa, fb) = (prep(&img), prep(&rot));
    let fwd = |x: f32, y: f32| (c * (x - cx) - s * (y - cy) + cx + tx, s * (x - cx) + c * (y - cy) + cy + ty);
    let tol = fa.period;
    let mut pos_ok = 0;
    let mut dir_ok = 0;
    for m in &fa.minutiae {
        let (ex, ey) = fwd(m.x, m.y);
        if let Some(n) = fb.minutiae.iter().min_by(|a, b| (a.x - ex).hypot(a.y - ey).total_cmp(&(b.x - ex).hypot(b.y - ey))) {
            if (n.x - ex).hypot(n.y - ey) < tol {
                pos_ok += 1;
                if minutiae::angle_diff(n.dir, m.dir + ang).abs() < 0.5 {
                    dir_ok += 1;
                }
            }
        }
    }
    println!("minutiae: {} in original, {} in transformed copy; {} reappear within one period, {} of those with the right direction", fa.minutiae.len(), fb.minutiae.len(), pos_ok, dir_ok);
    let mut errs: Vec<f32> = fa
        .minutiae
        .iter()
        .filter_map(|m| {
            let (ex, ey) = fwd(m.x, m.y);
            fb.minutiae.iter().map(|n| (n.x - ex).hypot(n.y - ey)).filter(|&d| d < tol).min_by(|a, b| a.total_cmp(b))
        })
        .collect();
    errs.sort_by(|a, b| a.total_cmp(b));
    if !errs.is_empty() {
        println!("position error of reappearing minutiae: median {:.1} px, 90th pct {:.1} px (period {:.1})", errs[errs.len() / 2], errs[errs.len() * 9 / 10], fa.period);
    }
    {
        let cp = mcc::Params::scaled(1.0);
        let (ta, tb) = (mcc::build(&fa, &cp), mcc::build(&fb, &cp));
        let mut lines = Vec::new();
        for (i, m) in fa.minutiae.iter().enumerate() {
            let (ex, ey) = fwd(m.x, m.y);
            let Some((j, _)) = fb.minutiae.iter().enumerate().find(|(_, n)| (n.x - ex).hypot(n.y - ey) < tol) else { continue };
            let (Some(ca), Some(cb)) = (&ta.cyl[i], &tb.cyl[j]) else {
                lines.push(format!("  #{i:2} true partner #{j:2}: cylinder invalid ({} / {})", ta.cyl[i].is_some(), tb.cyl[j].is_some()));
                continue;
            };
            let truth = mcc::cyl_sim(ca, cb, &cp);
            let best_other = tb.cyl.iter().enumerate().filter(|(k, c)| *k != j && c.is_some()).map(|(_, c)| mcc::cyl_sim(ca, c.as_ref().unwrap(), &cp)).fold(0.0f32, f32::max);
            let common = ca.valid.iter().zip(&cb.valid).filter(|(a, b)| **a && **b).count();
            let ones_a = ca.vals.iter().zip(&ca.valid).filter(|(v, ok)| **ok && **v > 0.5).count();
            lines.push(format!("  #{i:2} true partner #{j:2}: sim {truth:.3} vs best wrong {best_other:.3}; common cells {common}; 'on' cells {ones_a}"));
        }
        println!("{}", lines.join("\n"));
    }
    {
        let cp = mcc::Params::scaled(1.0);
        let (ta, tb) = (mcc::build(&fa, &cp), mcc::build(&fb, &cp));
        let (sc, pairs) = mcc::explain(&ta, &tb, &cp);
        let truth = |i: usize, j: usize| {
            let (ex, ey) = fwd(fa.minutiae[i].x, fa.minutiae[i].y);
            (fb.minutiae[j].x - ex).hypot(fb.minutiae[j].y - ey) < tol
        };
        let ok = pairs.iter().filter(|q| truth(q.0, q.1)).count();
        println!("explain: score {:.3}; {} pairs selected, {} of them true", sc.score, pairs.len(), ok);
        let tp: Vec<&(usize, usize, f32, f32)> = pairs.iter().filter(|q| truth(q.0, q.1)).take(5).collect();
        for x in 0..tp.len() {
            for y in (x + 1)..tp.len() {
                let (a1, b1, a2, b2) = (&fa.minutiae[tp[x].0], &fb.minutiae[tp[x].1], &fa.minutiae[tp[y].0], &fb.minutiae[tp[y].1]);
                let d1 = ((a1.x - a2.x).hypot(a1.y - a2.y) - (b1.x - b2.x).hypot(b1.y - b2.y)).abs();
                let d2 = minutiae::angle_diff(minutiae::angle_diff(a1.dir, a2.dir), minutiae::angle_diff(b1.dir, b2.dir)).to_degrees();
                let rad = |m1: &minutiae::Minutia, m2: &minutiae::Minutia| minutiae::angle_diff(m1.dir, (m2.y - m1.y).atan2(m2.x - m1.x));
                let d3 = minutiae::angle_diff(rad(a1, a2), rad(b1, b2)).to_degrees();
                println!("   true pair {x}-{y}: d1 {d1:5.1} px  d2 {d2:7.1} deg  d3 {d3:7.1} deg  (dist {:.0})", (a1.x - a2.x).hypot(a1.y - a2.y));
            }
        }
        for q in pairs.iter().take(12) {
            println!("   a#{:2} b#{:2} {} sim {:.3} -> {:.3}", q.0, q.1, if truth(q.0, q.1) { "TRUE " } else { "wrong" }, q.2, q.3);
        }
    }
    for (scale, axial) in [(1.0f32, false), (1.0, true)] {
        let cp = mcc::Params { axial, ..mcc::Params::scaled(scale) };
        let (ta, tb) = (mcc::build(&fa, &cp), mcc::build(&fb, &cp));
        let va = ta.cyl.iter().filter(|c| c.is_some()).count();
        let vb = tb.cyl.iter().filter(|c| c.is_some()).count();
        let sc = mcc::compare(&ta, &tb, &cp);
        let selfsc = mcc::compare(&ta, &ta, &cp);
        println!("mcc scale {scale} axial {axial}: valid cylinders {va}/{} and {vb}/{}; copy score {:.3} ({} pairs); self score {:.3}", fa.minutiae.len(), fb.minutiae.len(), sc.score, sc.pairs, selfsc.score);
    }
    Ok(())
}
