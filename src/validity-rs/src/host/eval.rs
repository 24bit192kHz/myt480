//! False-accept / false-reject measurement on public fingerprint databases (FVC
//! layout: one directory per database, files `<finger>_<impression>.pgm`), simulating
//! this sensor: images are rescaled to the sensor's ridge period, each finger is
//! enrolled from impressions 1-4, and sensor-sized crops of impressions 5-8 are the
//! unlock attempts. Impostor attempts are every probe against every other finger of
//! the same database.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};

use super::img::{Img, median, read_pgm};
use super::mcc::{self, Template};
use super::minutiae::{self, Params as MinParams};

pub struct Opts {
    pub root: PathBuf,
    pub probe: usize,
    pub crops_per_impression: usize,
    pub min_fg: f32,
    pub sensor_period: f32,
    pub mcc_scale: f32,
    pub threads: usize,
    pub only: Option<String>,
    pub report: Option<PathBuf>,
    pub r: Option<f32>,
    pub min_vc: Option<f32>,
    pub min_me: Option<f32>,
    /// "mcc" (ours) or "nbis" (NIST mindtct + bozorth3 reference, feature `nbis`).
    pub matcher: String,
}

impl Opts {
    pub fn mcc_params(&self) -> mcc::Params {
        let mut cp = mcc::Params::scaled(self.mcc_scale);
        if let Some(r) = self.r {
            cp.r = r;
        }
        if let Some(v) = self.min_vc {
            cp.min_vc = v;
        }
        if let Some(v) = self.min_me {
            cp.min_me = v;
        }
        cp
    }
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            root: PathBuf::new(),
            probe: 112,
            crops_per_impression: 3,
            min_fg: 0.75,
            sensor_period: 8.4,
            mcc_scale: 8.4 / 9.1,
            threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4),
            only: None,
            report: None,
            r: None,
            min_vc: None,
            min_me: None,
            matcher: "mcc".into(),
        }
    }
}

/// One comparison outcome: LSS-R score and supporting pairs.
#[derive(Clone, Copy)]
pub struct Cmp {
    pub score: f32,
    pub pairs: u16,
}

struct Probe {
    finger: u32,
    tpl: Template,
}

fn par_map<T: Sync, R: Send>(items: &[T], threads: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let chunk = items.len().div_ceil(threads.max(1)).max(1);
    std::thread::scope(|s| {
        let hs: Vec<_> = items.chunks(chunk).map(|c| s.spawn(|| c.iter().map(&f).collect::<Vec<R>>())).collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    })
}

fn lcg(seed: &mut u64) -> f32 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*seed >> 33) as f32) / (1u64 << 31) as f32
}

/// Template of a full image (gallery side).
pub fn full_template(img: &Img, mp: &MinParams, cp: &mcc::Params) -> Template {
    let p0 = minutiae::ridge_period(img, &vec![true; img.w * img.h]).clamp(5.0, 16.0);
    let fg = minutiae::foreground(img, p0);
    let band = minutiae::band(img, p0);
    mcc::build(&minutiae::extract(&band, &fg, mp), cp)
}

fn db_dirs(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let mut has_pgm = false;
        for e in std::fs::read_dir(&d).with_context(|| format!("listing {}", d.display()))? {
            let p = e?.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "pgm") {
                has_pgm = true;
            }
        }
        if has_pgm {
            out.push(d);
        }
    }
    out.sort();
    Ok(out)
}

pub struct DbResult {
    pub name: String,
    pub genuine: Vec<Cmp>,
    pub impostor: Vec<Cmp>,
    pub probes: usize,
    pub probe_ms: f32,
    pub compare_us: f32,
}

pub fn eval_db(dir: &Path, o: &Opts) -> Result<DbResult> {
    let name = dir.strip_prefix(&o.root).unwrap_or(dir).display().to_string();
    let mut files: BTreeMap<u32, BTreeMap<u32, PathBuf>> = BTreeMap::new();
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else { continue };
        let Some((f, i)) = stem.split_once('_') else { continue };
        if let (Ok(f), Ok(i)) = (f.parse::<u32>(), i.parse::<u32>()) {
            files.entry(f).or_default().insert(i, p);
        }
    }
    if files.len() < 2 {
        bail!("{name}: fewer than two fingers");
    }
    let list: Vec<(u32, u32, PathBuf)> = files.iter().flat_map(|(f, m)| m.iter().map(move |(i, p)| (*f, *i, p.clone()))).collect();
    let imgs: Vec<Option<Img>> = par_map(&list, o.threads, |(_, _, p)| read_pgm(p).ok());
    let periods: Vec<f32> = par_map(&imgs, o.threads, |im| {
        im.as_ref().map(|im| minutiae::ridge_period(im, &vec![true; im.w * im.h])).unwrap_or(0.0)
    });
    let valid: Vec<f32> = periods.iter().copied().filter(|&p| p > 0.0).collect();
    let scale = o.sensor_period / median(&valid);
    let mp = MinParams::default();
    let cp = o.mcc_params();
    // gallery: impressions 1-4, rescaled to the sensor's ridge period
    let gal_idx: Vec<usize> = (0..list.len()).filter(|&k| list[k].1 <= 4 && imgs[k].is_some()).collect();
    let gal: Vec<(u32, Template)> = par_map(&gal_idx, o.threads, |&k| (list[k].0, full_template(&imgs[k].as_ref().unwrap().resize(scale), &mp, &cp)));
    let mut gallery: BTreeMap<u32, Vec<Template>> = BTreeMap::new();
    for (f, t) in gal {
        gallery.entry(f).or_default().push(t);
    }
    // probes: sensor-sized crops of impressions 5-8 with enough fingerprint in them
    let probe_idx: Vec<usize> = (0..list.len()).filter(|&k| list[k].1 >= 5 && imgs[k].is_some()).collect();
    let t0 = Instant::now();
    let probes: Vec<Probe> = par_map(&probe_idx, o.threads, |&k| {
        let img = imgs[k].as_ref().unwrap().resize(scale);
        let mut out = Vec::new();
        if o.probe == 0 {
            // sanity mode: the whole impression is the probe
            out.push(Probe { finger: list[k].0, tpl: full_template(&img, &mp, &cp) });
            return out;
        }
        if img.w < o.probe || img.h < o.probe {
            return out;
        }
        let p0 = minutiae::ridge_period(&img, &vec![true; img.w * img.h]).clamp(5.0, 16.0);
        let fg_full = minutiae::foreground(&img, p0);
        let mut seed = (list[k].0 as u64) << 8 | list[k].1 as u64;
        let mut tries = 0;
        while out.len() < o.crops_per_impression && tries < 60 {
            tries += 1;
            let x0 = (lcg(&mut seed) * (img.w - o.probe) as f32) as usize;
            let y0 = (lcg(&mut seed) * (img.h - o.probe) as f32) as usize;
            let cover = (0..o.probe * o.probe).filter(|&i| fg_full[(y0 + i / o.probe) * img.w + x0 + i % o.probe]).count() as f32 / (o.probe * o.probe) as f32;
            if cover < o.min_fg {
                continue;
            }
            // process the crop on its own, like a sensor frame
            let crop = img.crop(x0, y0, o.probe, o.probe);
            let fg: Vec<bool> = (0..o.probe * o.probe).map(|i| fg_full[(y0 + i / o.probe) * img.w + x0 + i % o.probe]).collect();
            let band = minutiae::band(&crop, p0);
            let tpl = mcc::build(&minutiae::extract(&band, &fg, &mp), &cp);
            out.push(Probe { finger: list[k].0, tpl });
        }
        out
    })
    .into_iter()
    .flatten()
    .collect();
    let probe_ms = t0.elapsed().as_secs_f32() * 1000.0 * o.threads as f32 / probes.len().max(1) as f32;
    let fingers: Vec<u32> = gallery.keys().copied().collect();
    let t1 = Instant::now();
    let scored: Vec<(Cmp, Vec<Cmp>)> = par_map(&probes, o.threads, |p| {
        let best = |f: u32| -> Cmp {
            gallery[&f]
                .iter()
                .map(|g| mcc::compare(&p.tpl, g, &cp))
                .map(|s| Cmp { score: s.score, pairs: s.pairs as u16 })
                .max_by(|a, b| a.score.total_cmp(&b.score))
                .unwrap_or(Cmp { score: 0.0, pairs: 0 })
        };
        let genuine = if gallery.contains_key(&p.finger) { best(p.finger) } else { Cmp { score: 0.0, pairs: 0 } };
        let imp = fingers.iter().filter(|&&f| f != p.finger).map(|&f| best(f)).collect();
        (genuine, imp)
    });
    let n_cmp = probes.len() * fingers.len() * 4;
    let compare_us = t1.elapsed().as_secs_f32() * 1e6 * o.threads as f32 / n_cmp.max(1) as f32;
    let mut genuine = Vec::new();
    let mut impostor = Vec::new();
    for (g, imp) in scored {
        genuine.push(g);
        impostor.extend(imp);
    }
    Ok(DbResult { name, genuine, impostor, probes: probes.len(), probe_ms, compare_us })
}

/// FRR at the threshold giving the target FAR, for a minimum-pairs rule.
pub fn operating_point(genuine: &[Cmp], impostor: &[Cmp], min_pairs: u16, far: f64) -> (f32, f64, f64) {
    let eff = |c: &Cmp| if c.pairs >= min_pairs { c.score } else { 0.0 };
    let mut imp: Vec<f32> = impostor.iter().map(eff).collect();
    imp.sort_by(|a, b| b.total_cmp(a));
    let allowed = (far * imp.len() as f64).floor() as usize;
    // threshold just above the (allowed+1)-th highest impostor score
    let thr = if allowed < imp.len() { imp[allowed] + 1e-6 } else { 0.0 };
    let fa = imp.iter().filter(|&&s| s >= thr && s > 0.0).count() as f64 / imp.len().max(1) as f64;
    let fr = genuine.iter().filter(|c| !(eff(c) >= thr && eff(c) > 0.0)).count() as f64 / genuine.len().max(1) as f64;
    (thr, fa, fr)
}

pub fn run(o: &Opts) -> Result<()> {
    let dirs = db_dirs(&o.root)?;
    let dirs: Vec<PathBuf> = dirs.into_iter().filter(|d| o.only.as_ref().is_none_or(|s| d.display().to_string().contains(s.as_str()))).collect();
    if dirs.is_empty() {
        bail!("no databases with .pgm files under {}", o.root.display());
    }
    let mut report = String::new();
    let mut all_g = Vec::new();
    let mut all_i = Vec::new();
    let t0 = Instant::now();
    for d in &dirs {
        let r = if o.matcher == "nbis" { eval_db_nbis(d, o)? } else { eval_db(d, o)? };
        let (thr, fa, fr) = operating_point(&r.genuine, &r.impostor, 4, 1e-3);
        let line = format!(
            "{:<22} probes {:5}  impostor tries {:8}  | at FAR 1/1000: FRR {:5.1}% (thr {:.3}) | probe {:.0} ms, compare {:.0} us\n",
            r.name, r.probes, r.impostor.len(), fr * 100.0, thr, r.probe_ms, r.compare_us
        );
        let _ = fa;
        report.push_str(&line);
        all_g.extend(r.genuine);
        all_i.extend(r.impostor);
    }
    report.push_str(&format!(
        "\nall databases: {} genuine attempts, {} impostor attempts ({:.0} s)\n",
        all_g.len(),
        all_i.len(),
        t0.elapsed().as_secs_f32()
    ));
    let mut gs: Vec<f32> = all_g.iter().map(|c| c.score).collect();
    let mut is: Vec<f32> = all_i.iter().map(|c| c.score).collect();
    gs.sort_by(|a, b| a.total_cmp(b));
    is.sort_by(|a, b| a.total_cmp(b));
    let q = |v: &[f32], p: f32| if v.is_empty() { 0.0 } else { v[((v.len() - 1) as f32 * p) as usize] };
    report.push_str(&format!(
        "genuine score median {:.3} (zero for {:.0}%), impostor p99 {:.3} p99.9 {:.3} max {:.3}\n",
        q(&gs, 0.5),
        100.0 * gs.iter().filter(|&&v| v == 0.0).count() as f32 / gs.len().max(1) as f32,
        q(&is, 0.99),
        q(&is, 0.999),
        q(&is, 1.0)
    ));
    report.push_str("min pairs | FAR target | threshold | measured FAR | FRR (rejected genuine touches)\n");
    for &mp in &[3u16, 4, 5, 6] {
        for &far in &[1e-3, 1e-4, 2e-5] {
            let (thr, fa, fr) = operating_point(&all_g, &all_i, mp, far);
            report.push_str(&format!("   {mp}      | 1/{:<7} |  {thr:.3}    | {:.1e}      | {:5.1}%\n", (1.0 / far).round(), fa, fr * 100.0));
        }
    }
    print!("{report}");
    if let Some(p) = &o.report {
        std::fs::write(p, &report).with_context(|| format!("writing {}", p.display()))?;
    }
    Ok(())
}

fn list_db(dir: &Path) -> Result<Vec<(u32, u32, PathBuf)>> {
    let mut files: BTreeMap<u32, BTreeMap<u32, PathBuf>> = BTreeMap::new();
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else { continue };
        let Some((f, i)) = stem.split_once('_') else { continue };
        if let (Ok(f), Ok(i)) = (f.parse::<u32>(), i.parse::<u32>()) {
            files.entry(f).or_default().insert(i, p);
        }
    }
    Ok(files.iter().flat_map(|(f, m)| m.iter().map(move |(i, p)| (*f, *i, p.clone()))).collect())
}

/// Same protocol with the NIST NBIS reference (MINDTCT + BOZORTH3) at the images' native
/// 500 ppi. `Cmp.score` is the BOZORTH3 score, `Cmp.pairs` the probe's minutiae count.
#[cfg(feature = "nbis")]
fn nbis_count(t: &fprint_pipeline::fprint_core::Template) -> usize {
    match t {
        fprint_pipeline::fprint_core::Template::Nbis(v) => v.iter().map(Vec::len).sum(),
        _ => 0,
    }
}

#[cfg(feature = "nbis")]
pub fn eval_db_nbis(dir: &Path, o: &Opts) -> Result<DbResult> {
    use fprint_pipeline::{GrayImage, nbis_match_score, template_from_images};
    let name = dir.strip_prefix(&o.root).unwrap_or(dir).display().to_string();
    let list = list_db(dir)?;
    let imgs: Vec<Option<Img>> = par_map(&list, o.threads, |(_, _, p)| read_pgm(p).ok());
    let to_u8 = |im: &Img| im.px.iter().map(|&v| v.clamp(0.0, 255.0) as u8).collect::<Vec<u8>>();
    let tpl = |im: &Img| {
        let px = to_u8(im);
        GrayImage::new(&px, im.w, im.h, 500).ok().map(|g| template_from_images(&[g]))
    };
    let gal_idx: Vec<usize> = (0..list.len()).filter(|&k| list[k].1 <= 4 && imgs[k].is_some()).collect();
    let gal: Vec<(u32, Option<fprint_pipeline::fprint_core::Template>)> = par_map(&gal_idx, o.threads, |&k| (list[k].0, tpl(imgs[k].as_ref().unwrap())));
    let mut gallery: BTreeMap<u32, Vec<fprint_pipeline::fprint_core::Template>> = BTreeMap::new();
    for (f, t) in gal {
        if let Some(t) = t {
            gallery.entry(f).or_default().push(t);
        }
    }
    let mp = MinParams::default();
    let probe_idx: Vec<usize> = (0..list.len()).filter(|&k| list[k].1 >= 5 && imgs[k].is_some()).collect();
    let t0 = Instant::now();
    let probes: Vec<(u32, fprint_pipeline::fprint_core::Template, usize)> = par_map(&probe_idx, o.threads, |&k| {
        let img = imgs[k].as_ref().unwrap();
        let mut out = Vec::new();
        if o.probe == 0 {
            if let Some(t) = tpl(img) {
                let n = nbis_count(&t);
                out.push((list[k].0, t, n));
            }
            return out;
        }
        if img.w < o.probe || img.h < o.probe {
            return out;
        }
        let p0 = minutiae::ridge_period(img, &vec![true; img.w * img.h]).clamp(5.0, 16.0);
        let fg_full = minutiae::foreground(img, p0);
        let _ = &mp;
        let mut seed = (list[k].0 as u64) << 8 | list[k].1 as u64;
        let mut tries = 0;
        while out.len() < o.crops_per_impression && tries < 60 {
            tries += 1;
            let x0 = (lcg(&mut seed) * (img.w - o.probe) as f32) as usize;
            let y0 = (lcg(&mut seed) * (img.h - o.probe) as f32) as usize;
            let cover = (0..o.probe * o.probe).filter(|&i| fg_full[(y0 + i / o.probe) * img.w + x0 + i % o.probe]).count() as f32 / (o.probe * o.probe) as f32;
            if cover < o.min_fg {
                continue;
            }
            if let Some(t) = tpl(&img.crop(x0, y0, o.probe, o.probe)) {
                let n = nbis_count(&t);
                out.push((list[k].0, t, n));
            }
        }
        out
    })
    .into_iter()
    .flatten()
    .collect();
    let probe_ms = t0.elapsed().as_secs_f32() * 1000.0 * o.threads as f32 / probes.len().max(1) as f32;
    let fingers: Vec<u32> = gallery.keys().copied().collect();
    let t1 = Instant::now();
    let scored: Vec<(Cmp, Vec<Cmp>)> = par_map(&probes, o.threads, |(f, p, n)| {
        let best = |g: u32| -> Cmp {
            let s = gallery[&g].iter().filter_map(|e| nbis_match_score(e, p).score()).max().unwrap_or(0);
            Cmp { score: s as f32, pairs: (*n).min(u16::MAX as usize) as u16 }
        };
        let genuine = if gallery.contains_key(f) { best(*f) } else { Cmp { score: 0.0, pairs: 0 } };
        let imp = fingers.iter().filter(|&&g| g != *f).map(|&g| best(g)).collect();
        (genuine, imp)
    });
    let n_cmp = probes.len() * fingers.len() * 4;
    let compare_us = t1.elapsed().as_secs_f32() * 1e6 * o.threads as f32 / n_cmp.max(1) as f32;
    let (mut genuine, mut impostor) = (Vec::new(), Vec::new());
    for (g, imp) in scored {
        genuine.push(g);
        impostor.extend(imp);
    }
    Ok(DbResult { name, genuine, impostor, probes: probes.len(), probe_ms, compare_us })
}

#[cfg(not(feature = "nbis"))]
pub fn eval_db_nbis(_dir: &Path, _o: &Opts) -> Result<DbResult> {
    bail!("built without the `nbis` feature")
}
