//! Capture programs, calibration, enrollment and on-chip matching for sensor type 0x199.

use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
use log::{debug, info, warn};
use sha2::{Digest, Sha256};

use crate::blobs::{self, TYPE_199};
use crate::db::Sid;
use crate::device::Device;
use crate::timeslot::{self, Chunk};
use crate::usb::{Cancelled, UsbFailure};
use crate::util::{Reader, Writer, check_status, unhex};

pub const KEY_CALIBRATION_LINE: usize = 0x38;
pub const CALIBRATION_FRAMES: usize = 3;
pub const CALIBRATION_ITERATIONS: usize = 3;
/// Minimum pause between an aborted capture and the next one.
const ABORT_SETTLE: Duration = Duration::from_millis(150);
/// Upper bound for interrupts that must arrive once the sensor is committed.
const COMMITTED_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Calibrate,
    Identify,
    Enroll,
}

#[derive(Debug, Clone, Copy)]
pub struct RomInfo {
    pub timestamp: u32,
    pub build: u32,
    pub major: u8,
    pub minor: u8,
    pub product: u8,
    pub u1: u8,
}

/// Everything `build_cmd_02` depends on; kept separate from the USB session so it can be tested.
#[derive(Clone, Default)]
pub struct CaptureState {
    pub factory_calibration_values: Vec<u8>,
    pub calib_data: Vec<u8>,
    pub lines_per_frame: usize,
}

/// Progress reported while enrolling or identifying.
#[derive(Debug)]
pub enum Progress {
    StagePassed,
    RetryScan(String),
}

/// Match result: (user record id, finger subtype, template hash).
pub type Match = (u32, u16, Vec<u8>);

pub fn is_cancelled(e: &anyhow::Error) -> bool {
    e.downcast_ref::<Cancelled>().is_some()
}

pub fn is_usb_failure(e: &anyhow::Error) -> bool {
    e.downcast_ref::<UsbFailure>().is_some()
}

// --------------------------------------------------------------- pure helpers

/// Pack bytes as (value - min) in the fewest bits each, LSB first.
pub fn bitpack(b: &[u8]) -> (u8, u8, Vec<u8>) {
    let m = *b.iter().min().unwrap_or(&0);
    let x = *b.iter().max().unwrap_or(&0) - m;
    let u = (8 - x.leading_zeros()) as usize;
    let mut out = vec![0u8; (u * b.len()).div_ceil(8)];
    for (i, &v) in b.iter().enumerate() {
        let v = (v - m) as usize;
        for bit in 0..u {
            if v >> bit & 1 == 1 {
                let pos = i * u + bit;
                out[pos / 8] |= 1 << (pos % 8);
            }
        }
    }
    (u as u8, m, out)
}

fn clip(x: i32) -> u8 {
    x.clamp(-128, 127) as i8 as u8
}

fn scale(x: u8) -> u8 {
    clip((x as i32 - 0x80) * 10 / 0x22)
}

fn add(l: u8, r: u8) -> u8 {
    clip(l as i8 as i32 + r as i8 as i32)
}

/// Multiply Call repeat counts (and bump their addresses), exactly as the DLL does.
pub fn patch_timeslot_table(b: &[u8], inc_address: bool, mult: u8) -> Result<Vec<u8>> {
    let mut b = b.to_vec();
    let mut i = 0;
    while i + 3 < b.len() {
        if b[i] & 0xf8 == 0x10 {
            if b[i + 2] > 1 {
                b[i + 2] = b[i + 2].checked_mul(mult).context("timeslot repeat overflow")?;
                if inc_address {
                    b[i + 1] = b[i + 1].checked_add(1).context("timeslot address overflow")?;
                }
            }
            i += 3;
        } else if b[i] == 0 {
            i += 1;
        } else if b[i] == 7 {
            i += 2;
        } else {
            break;
        }
    }
    Ok(b)
}

/// Point the last register write to 0x8000203c (inside the last Call) at the
/// factory calibration value for the middle of the sensor.
pub fn patch_timeslot_again(b: &[u8], factory: &[u8]) -> Result<Vec<u8>> {
    let mut b = b.to_vec();
    let mut pc = 0;
    let mut target = None;
    while pc < b.len() {
        let ins = timeslot::decode(&b[pc..])?;
        if matches!(ins.op, 1 | 2 | 4) {
            break;
        }
        if ins.op == 11 {
            target = Some(ins.b as usize);
        }
        pc += ins.len;
    }
    let Some(mut pc) = target else { return Ok(b) };
    let mut hit = None;
    while pc < b.len() {
        let ins = timeslot::decode(&b[pc..])?;
        if matches!(ins.op, 1 | 2 | 4) {
            break;
        }
        if ins.op == 13 && ins.a == 0x8000_203c {
            hit = Some(pc);
        }
        pc += ins.len;
    }
    if let Some(at) = hit {
        b[at + 1] = *factory.get(KEY_CALIBRATION_LINE).context("short factory calibration table")?;
    }
    Ok(b)
}

/// Average the interleaved lines of the second calibration frame.
pub fn average(raw: &[u8], lines_per_frame: usize) -> Result<Vec<u8>> {
    let bpl = TYPE_199.bytes_per_line;
    let frame_size = lines_per_frame * bpl;
    let interleave = lines_per_frame / TYPE_199.lines_per_calibration_data;
    ensure!(interleave == 2 && CALIBRATION_FRAMES > 1, "unsupported calibration geometry");
    let frame = raw.get(frame_size..2 * frame_size).context("short calibration capture")?;
    let mut out = Vec::with_capacity(frame_size / interleave);
    for group in frame.chunks(interleave * bpl) {
        let lines: Vec<&[u8]> = group.chunks(bpl).collect();
        for col in 0..bpl {
            let sum: usize = lines.iter().map(|l| l[col] as usize).sum();
            out.push((sum / lines.len()) as u8);
        }
    }
    Ok(out)
}

pub fn process_calibration_results(calib: &mut Vec<u8>, cooked: &[u8]) {
    let bpl = TYPE_199.bytes_per_line;
    let frame: Vec<u8> = cooked
        .chunks(bpl)
        .flat_map(|l| l[..8.min(l.len())].iter().copied().chain(l.iter().skip(8).map(|&x| scale(x))))
        .collect();
    if calib.is_empty() {
        *calib = frame;
        return;
    }
    let combined = calib
        .chunks(bpl)
        .zip(frame.chunks(bpl))
        .flat_map(|(ll, rr)| {
            let head = ll[..8].to_vec();
            head.into_iter().chain(ll[8..].iter().zip(&rr[8..]).map(|(&l, &r)| add(l, r)))
        })
        .collect();
    *calib = combined;
}

/// Deviation of a blank frame's pixels from mid-scale (0x80), which is what a
/// perfectly calibrated sensor reads with nothing on it.
pub fn residual_stats(frame: &[u8]) -> String {
    let px: Vec<i32> = frame
        .chunks(TYPE_199.bytes_per_line)
        .flat_map(|l| l.iter().skip(8).map(|&x| x as i32 - 0x80))
        .collect();
    let n = px.len().max(1) as f64;
    let mean = px.iter().sum::<i32>() as f64 / n;
    let rms = (px.iter().map(|&x| (x * x) as f64).sum::<f64>() / n).sqrt();
    let off = px.iter().filter(|&&x| x.abs() >= 8).count();
    format!(
        "mean {mean:+.2}, rms {rms:.2}, range {}..{}, {off} of {} pixels off by 8 or more",
        px.iter().min().unwrap_or(&0),
        px.iter().max().unwrap_or(&0),
        px.len()
    )
}

pub fn key_line(calib: &[u8]) -> Vec<u8> {
    let w = TYPE_199.line_width;
    if calib.is_empty() {
        return vec![0; w];
    }
    let per_line = calib.len() / TYPE_199.lines_per_calibration_data;
    let off = 8 + per_line * KEY_CALIBRATION_LINE;
    calib[off..off + w].iter().map(|&i| if i == 5 { 4 } else { i }).collect()
}

struct Line {
    mask: u32,
    flags: u32,
    data: Vec<u8>,
    v0: u8,
    v1: u8,
    v2: u16,
}

impl Line {
    fn cnt(&self) -> u32 {
        (self.flags & 0x00f0_0000) >> 20
    }
}

/// Build the "02" capture command (line-update type 1 devices).
pub fn build_cmd_02(st: &CaptureState, mode: Mode) -> Result<Vec<u8>> {
    let t = &TYPE_199;
    let mut chunks = timeslot::split_chunks(blobs::CAPTURE_PROG_199)?;
    let mut tst = None;
    for c in chunks.iter_mut().filter(|c| c.typ == 0x34) {
        let mut p = patch_timeslot_table(&c.data, true, t.repeat_multiplier)?;
        if mode != Mode::Calibrate {
            p = patch_timeslot_again(&p, &st.factory_calibration_values)?;
        }
        let mut data = key_line(&st.calib_data);
        data.extend_from_slice(p.get(t.line_width..).context("short timeslot table")?);
        c.data = data;
        tst = Some(p);
    }
    let tst = tst.context("capture program has no timeslot table")?;

    let chunk = |typ, data: Vec<u8>| Chunk { typ, data };
    chunks.push(chunk(0x17, vec![]));
    match mode {
        Mode::Identify => {
            chunks.push(chunk(0x4e, unhex("fbb20f0000000f00300000008700020067000a00018000000a0200000b1900008813b80b01091000")));
            chunks.push(chunk(0x2e, unhex("0200180002000000700070004d010000a0008c003c32321e3c0a0202")));
        }
        Mode::Enroll => {
            chunks.push(chunk(0x26, unhex("fbb20f0000000f00300000008700020067000a00018000000a0200000b19000050c360ea01091000")));
            chunks.push(chunk(0x2e, unhex("0200180023000000700070004d010000a0008c003c32321e3c0a0202")));
        }
        Mode::Calibrate => {}
    }
    chunks.push(chunk(0x44, 1u32.to_le_bytes().to_vec()));

    let mut lines = Vec::new();
    let pc = timeslot::find_nth_insn(&tst, 6, 2)?; // 2nd "Enable Rx"
    lines.push(Line {
        mask: 0xff,
        flags: (pc as u32 + 1) | 2 << 20 | 0x700_0000,
        data: blobs::CALIBRATION_BLOB_199.to_vec(),
        v0: 0xf,
        v1: 0,
        v2: 0,
    });
    let pc = timeslot::find_nth_regwrite(&tst, 0x8000_203c, 1)?;
    let (u, m, packed) = bitpack(&st.factory_calibration_values);
    lines.push(Line {
        mask: 0xff,
        flags: (pc as u32 + 1) | 3 << 20 | 0x700_0000,
        data: packed,
        v0: (u.wrapping_sub(1)) | 8,
        v1: m,
        v2: 0,
    });
    if !st.calib_data.is_empty() {
        let per_line = st.calib_data.len() / t.lines_per_calibration_data;
        for i in (0..112).step_by(4) {
            let mut data = Vec::with_capacity(112 * 4);
            for j in 0..112 {
                let p = 8 + j * per_line + i;
                data.extend_from_slice(st.calib_data.get(p..p + 4).context("short calibration data")?);
            }
            lines.push(Line { mask: 0xffff_ffff, flags: i as u32 | 0x85 << 24, data, v0: 0, v1: 0, v2: 0 });
        }
    }
    for l in &mut lines {
        let pad = l.data.len() % 4;
        if pad > 0 {
            l.data.resize(l.data.len() + 4 - pad, 0);
        }
    }

    let mut update = (lines.len() as u32).to_le_bytes().to_vec();
    for l in &lines {
        update.extend(l.mask.to_le_bytes());
        update.extend(l.flags.to_le_bytes());
    }
    for l in lines.iter().filter(|l| l.cnt() <= 1) {
        update.extend_from_slice(&l.data);
    }
    chunks.push(chunk(0x30, update));

    let mut transform = Vec::new();
    for l in lines.iter().filter(|l| l.cnt() > 1) {
        transform.extend([l.v0, l.v1]);
        transform.extend(l.v2.to_le_bytes());
        transform.extend_from_slice(&l.data);
    }
    chunks.push(chunk(0x43, transform));

    let req_lines = if mode == Mode::Calibrate { CALIBRATION_FRAMES * st.lines_per_frame + 1 } else { 0 };
    Ok(Writer::new()
        .u8(2)
        .u16(t.bytes_per_line as u16)
        .u16(req_lines as u16)
        .bytes(&timeslot::merge_chunks(&chunks))
        .done())
}

/// Parse `get_factory_bits` reply into subtag → value.
pub fn parse_factory_bits(rsp: &[u8]) -> Result<Vec<(u16, Vec<u8>)>> {
    check_status(rsp)?;
    let mut r = Reader::new(&rsp[2..]);
    let _wtf = r.u32()?;
    let entries = r.u32()?;
    let mut out = Vec::new();
    for _ in 0..entries {
        let (_ptr, l, _tag, subtag, _flags) = (r.u32()?, r.u16()?, r.u16()?, r.u16()?, r.u16()?);
        out.push((subtag, r.take(l as usize)?.to_vec()));
    }
    ensure!(r.is_empty(), "garbage at the end of factory bits");
    Ok(out)
}

pub fn make_finger_data(subtype: u16, template: &[u8], tid: &[u8]) -> Vec<u8> {
    let tinfo = Writer::new()
        .u16(1)
        .u16(template.len() as u16)
        .bytes(template)
        .u16(2)
        .u16(tid.len() as u16)
        .bytes(tid)
        .done();
    Writer::new().u16(subtype).u16(3).u16(tinfo.len() as u16).u16(0x20).bytes(&tinfo).zeros(0x20).done()
}

/// Optional pieces of an enrollment-update reply.
pub type EnrollUpdate = (Option<Vec<u8>>, Option<Vec<u8>>, Option<Vec<u8>>);

/// Split an enrollment-update reply into (header, template, tid).
pub fn parse_enroll_update(res: &[u8]) -> Result<EnrollUpdate> {
    const MAGIC: usize = 0x38;
    ensure!(res.len() >= 2, "short enrollment update");
    let l = u16::from_le_bytes([res[0], res[1]]) as usize;
    let mut res = &res[2..];
    ensure!(l == res.len(), "enrollment update size mismatch {l} != {}", res.len());
    let (mut header, mut template, mut tid) = (None, None, None);
    while !res.is_empty() {
        ensure!(res.len() >= 4, "truncated enrollment tag");
        let tag = u16::from_le_bytes([res[0], res[1]]);
        let l = u16::from_le_bytes([res[2], res[3]]) as usize;
        let end = (MAGIC + l).min(res.len());
        match tag {
            0 => template = Some(res[..end].to_vec()),
            1 => header = Some(res[MAGIC.min(end)..end].to_vec()),
            3 => tid = Some(res[MAGIC.min(end)..end].to_vec()).filter(|t| !t.is_empty()),
            _ => warn!("ignoring unknown enrollment tag {tag:x}"),
        }
        res = &res[end..];
    }
    Ok((header, template, tid))
}

// ------------------------------------------------------------ device commands

impl Device {
    pub fn rom_info(&mut self) -> Result<RomInfo> {
        let rsp = self.cmd(&[0x01])?;
        check_status(&rsp)?;
        let mut r = Reader::new(&rsp[2..]);
        let (timestamp, build, major, minor) = (r.u32()?, r.u32()?, r.u8()?, r.u8()?);
        r.take(1)?;
        let product = r.u8()?;
        r.take(3)?;
        let u1 = r.u8()?;
        Ok(RomInfo { timestamp, build, major, minor, product, u1 })
    }

    /// Returns (major, version, name) and fails for anything but a 57K0 (type 0x199).
    pub fn identify_sensor(&mut self) -> Result<(u16, u16, &'static str)> {
        let rsp = self.cmd(&[0x75])?;
        check_status(&rsp)?;
        let mut r = Reader::new(&rsp[2..]);
        let (zeroes, minor, major) = (r.u32()?, r.u16()?, r.u16()?);
        ensure!(zeroes == 0, "unexpected sensor id reply");
        ensure!(major == blobs::SENSOR_MAJOR, "sensor major {major:#x} is not a T480 57K0");
        let row = blobs::DEV_199.iter().find(|(v, mask, _)| v & mask == minor);
        let (_, _, name) = row.with_context(|| format!("unknown 57K0 sensor version {minor:#x}"))?;
        Ok((major, minor, name))
    }

    pub fn read_hw_reg32(&mut self, addr: u32) -> Result<u32> {
        let rsp = self.cmd(&Writer::new().u8(7).u32(addr).u8(4).done())?;
        check_status(&rsp)?;
        Reader::new(&rsp[2..]).u32()
    }

    pub fn write_hw_reg32(&mut self, addr: u32, val: u32) -> Result<()> {
        check_status(&self.cmd(&Writer::new().u8(8).u32(addr).u32(val).u8(4).done())?)
    }

    pub fn glow_start_scan(&mut self) -> Result<()> {
        check_status(&self.app(&unhex(blobs::GLOW_START_SCAN))?)
    }

    pub fn glow_end_scan(&mut self) -> Result<()> {
        check_status(&self.app(&unhex(blobs::GLOW_END_SCAN))?)
    }

    pub fn open_sensor(&mut self) -> Result<()> {
        let (_, _, name) = self.identify_sensor()?;
        info!("opening sensor: {name}");
        let rom = self.rom_info()?;
        ensure!(rom.major == 6 && rom.product == 0x30, "unsupported sensor ROM {rom:?}");
        let lines_2d = timeslot::split_chunks(blobs::CAPTURE_PROG_199)?
            .iter()
            .find(|c| c.typ == 0x2f)
            .map(|c| u32::from_le_bytes(c.data[..4].try_into().unwrap()))
            .context("no 2D chunk in capture program")?;
        self.capture.lines_per_frame = lines_2d as usize * TYPE_199.repeat_multiplier as usize;
        let rsp = self.cmd(&Writer::new().u8(0x6f).u16(0x0e00).u16(0).u32(0).done())?;
        let bits = parse_factory_bits(&rsp)?;
        let v = bits.iter().find(|b| b.0 == 3).context("no factory calibration values")?;
        ensure!(v.1.len() > 4 + KEY_CALIBRATION_LINE, "short factory calibration values");
        self.capture.factory_calibration_values = v.1[4..].to_vec();
        self.calibrate()
    }

    fn check_clean_slate(&mut self) -> Result<bool> {
        let start = self.read_flash(6, 0, 0x44)?;
        ensure!(start.len() == 0x44, "short calibration header");
        let magic = u16::from_le_bytes([start[0], start[1]]);
        let l = u16::from_le_bytes([start[2], start[3]]) as u32;
        if magic != 0x5002 {
            return Ok(false);
        }
        if start[0x24..0x44].iter().any(|&b| b != 0) {
            warn!("unexpected contents in calibration flash partition");
            return Ok(false);
        }
        let img = self.read_flash_all(6, 0x44, l)?;
        if Sha256::digest(&img).as_slice() != &start[4..0x24] {
            warn!("calibration flash hash mismatch");
            return Ok(false);
        }
        Ok(true)
    }

    pub fn calibrate(&mut self) -> Result<()> {
        let path = self.cfg.data_dir.join("calib-data.bin");
        // First run after python-validity: reuse its calibration (it matches the flash).
        let existing = std::iter::once(path.clone())
            .chain(self.cfg.import_dirs.iter().map(|d| d.join("calib-data.bin")))
            .find(|p| p.is_file());
        match existing.as_ref().map(std::fs::read).transpose().ok().flatten().ok_or(()) {
            Ok(d) => {
                self.capture.calib_data = d;
                let from = existing.unwrap();
                debug!("calibration data loaded from {}", from.display());
                if self.check_clean_slate()? {
                    if from != path {
                        info!("importing calibration data from {}", from.display());
                        std::fs::create_dir_all(&self.cfg.data_dir)?;
                        std::fs::write(&path, &self.capture.calib_data)?;
                    }
                    return Ok(());
                }
                info!("no calibration data on the flash; calibrating");
            }
            Err(_) => {
                self.capture.calib_data.clear();
                info!("no calibration data file; calibrating");
            }
        }
        self.run_calibration()?;
        std::fs::create_dir_all(&self.cfg.data_dir)?;
        std::fs::write(&path, &self.capture.calib_data).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    /// Calibrate from scratch and store the clean-slate image on the flash.
    pub fn run_calibration(&mut self) -> Result<()> {
        self.capture.calib_data.clear();
        for i in 0..CALIBRATION_ITERATIONS {
            debug!("calibration iteration {i}");
            let cmd = build_cmd_02(&self.capture, Mode::Calibrate)?;
            check_status(&self.cmd(&cmd)?)?;
            let raw = self.usb.read_data()?;
            let avg = average(&raw, self.capture.lines_per_frame)?;
            process_calibration_results(&mut self.capture.calib_data, &avg);
        }
        let cmd = build_cmd_02(&self.capture, Mode::Calibrate)?;
        check_status(&self.cmd(&cmd)?)?;
        let blank = average(&self.usb.read_data()?, self.capture.lines_per_frame)?;
        let mut cs = Writer::new().u16(blank.len() as u16).bytes(&blank).u16(0).done();
        let hash = Sha256::digest(&cs);
        cs = Writer::new().u16(0x5002).u16(cs.len() as u16).bytes(&hash).zeros(0x20).bytes(&cs).done();

        let start = self.read_flash(6, 0, 0x44)?;
        if start.iter().any(|&b| b != 0xff) {
            if cs[..0x44] == start[..] {
                info!("calibration data already matches the flash");
                return Ok(());
            }
            info!("erasing old calibration data on the flash");
            self.erase_flash(6)?;
        }
        self.write_flash_all(6, 0, &cs)
    }

    /// One blank frame (nobody touching the sensor) with the current calibration applied.
    fn blank_frame(&mut self) -> Result<Vec<u8>> {
        let cmd = build_cmd_02(&self.capture, Mode::Calibrate)?;
        check_status(&self.cmd(&cmd)?)?;
        average(&self.usb.read_data()?, self.capture.lines_per_frame)
    }

    /// Measure how well the stored calibration still fits the sensor, without
    /// writing anything: the residual of a blank frame under the stored
    /// calibration, the same under a fresh in-memory calibration, and how far
    /// the two calibrations are apart. Keep fingers off the sensor.
    pub fn calib_check(&mut self) -> Result<()> {
        let stored = self.capture.calib_data.clone();
        ensure!(!stored.is_empty(), "no stored calibration to check");
        let res = (|| {
            for i in 0..3 {
                println!("stored calibration, blank frame {i}: {}", residual_stats(&self.blank_frame()?));
            }
            self.capture.calib_data.clear();
            for _ in 0..CALIBRATION_ITERATIONS {
                let avg = self.blank_frame()?;
                process_calibration_results(&mut self.capture.calib_data, &avg);
            }
            for i in 0..3 {
                println!("fresh calibration,  blank frame {i}: {}", residual_stats(&self.blank_frame()?));
            }
            let bpl = TYPE_199.bytes_per_line;
            let diffs: Vec<i32> = stored
                .chunks(bpl)
                .zip(self.capture.calib_data.chunks(bpl))
                .flat_map(|(a, b)| a[8..].iter().zip(&b[8..]).map(|(&a, &b)| (a as i8 as i32 - b as i8 as i32).abs()))
                .collect();
            let n = diffs.len().max(1);
            println!(
                "stored vs fresh calibration: mean |diff| {:.2}, max {}, {} of {} values differ by 3 or more",
                diffs.iter().sum::<i32>() as f64 / n as f64,
                diffs.iter().max().unwrap_or(&0),
                diffs.iter().filter(|&&d| d >= 3).count(),
                diffs.len()
            );
            Ok(())
        })();
        self.capture.calib_data = stored;
        res
    }

    /// Wait for a finger and capture one image. `capture stop` always follows;
    /// after a failed or cancelled capture the sensor is also given time to wind
    /// the program down before the next one starts.
    pub fn capture(&mut self, mode: Mode) -> Result<(u16, u16, u16, u16)> {
        if let Some(t) = self.last_abort
            && let Some(rest) = ABORT_SETTLE.checked_sub(t.elapsed()) {
                debug!("letting the sensor settle {} ms after an aborted capture", rest.as_millis());
                std::thread::sleep(rest);
            }
            // Events that arrived while settling belong to the aborted capture.
            self.usb.drain_int();
        let res = self.capture_inner(mode);
        let stop = self.app(&[0x04]);
        if res.is_err() && !res.as_ref().is_err_and(is_usb_failure) {
            self.last_abort = Some(Instant::now());
            self.usb.drain_int();
        }
        let v = res?;
        stop?;
        Ok(v)
    }

    fn capture_inner(&mut self, mode: Mode) -> Result<(u16, u16, u16, u16)> {
        let cmd = build_cmd_02(&self.capture, mode)?;
        check_status(&self.app(&cmd)?).context("start capture")?;
        let b = self.usb.wait_int()?;
        ensure!(b.first() == Some(&0), "wait_start: unexpected interrupt {}", hex::encode(&b));
        // Waiting for the finger is the only cancellable phase.
        while self.usb.wait_int()?.first() != Some(&2) {}
        // Finger down: let the capture complete even if a cancel arrives now.
        loop {
            let b = self.usb.wait_int_for(COMMITTED_TIMEOUT)?;
            ensure!(b.first() == Some(&3) && b.len() >= 3, "unexpected interrupt {}", hex::encode(&b));
            if b[2] & 4 != 0 {
                break;
            }
        }
        let rsp = self.app(&unhex("5100200000"))?;
        check_status(&rsp).context("capture status")?;
        let mut r = Reader::new(&rsp[2..]);
        let l = r.u32()? as usize;
        ensure!(l == r.len(), "capture status size mismatch");
        let (x, y, w1, w2, error) = (r.u16()?, r.u16()?, r.u16()?, r.u16()?, r.u32()?);
        if error != 0 {
            bail!("scanning problem: {error:04x}");
        }
        Ok((x, y, w1, w2))
    }

    pub fn match_finger(&mut self) -> Result<Option<Match>> {
        let res = self.match_inner();
        let _ = self.app(&unhex("6200000000")); // cleanup, errors ignored
        res
    }

    fn match_inner(&mut self) -> Result<Option<Match>> {
        let cmd = Writer::new().u8(0x5e).u8(2).u8(0xff).u16(0).u16(0).u16(1).u16(0).u16(0).done();
        check_status(&self.app(&cmd)?)?;
        let b = self.usb.wait_int_for(COMMITTED_TIMEOUT)?;
        if b.first() != Some(&3) {
            info!("finger not recognized: {}", hex::encode(&b));
            return Ok(None);
        }
        let rsp = self.app(&unhex("6000000000"))?;
        check_status(&rsp)?;
        let mut r = Reader::new(&rsp[2..]);
        let l = r.u16()? as usize;
        ensure!(l == r.len(), "match result size mismatch");
        let (mut usr, mut sub, mut hash) = (None, None, None);
        while !r.is_empty() {
            let (t, l) = (r.u16()?, r.u16()? as usize);
            let v = r.take(l)?;
            match t {
                1 => usr = Some(Reader::new(v).u32()?),
                3 => sub = Some(Reader::new(v).u16()?),
                4 => hash = Some(v.to_vec()),
                _ => {}
            }
        }
        Ok(Some((usr.context("no user id in match")?, sub.context("no subtype in match")?, hash.unwrap_or_default())))
    }

    /// Capture until a usable image arrives, then match it on the chip.
    /// `Ok(None)` means the finger was read but matched nobody.
    pub fn identify(&mut self, progress: &mut dyn FnMut(Progress)) -> Result<Option<Match>> {
        let mut failures = 0;
        loop {
            self.glow_start_scan().context("glow")?;
            match self.capture(Mode::Identify) {
                Ok((x, y, w1, w2)) => {
                    // The sensor's own report on the image it is about to match.
                    info!("scan: x {x} y {y} w1 {w1} w2 {w2}");
                    break;
                }
                Err(e) if is_cancelled(&e) => {
                    let _ = self.glow_end_scan();
                    return Err(e);
                }
                Err(e) if is_usb_failure(&e) => return Err(e),
                Err(e) => {
                    failures += 1;
                    debug!("capture failed: {e:#}");
                    progress(Progress::RetryScan(format!("{e:#}")));
                    ensure!(failures < 50, "too many consecutive capture failures: {e:#}");
                    std::thread::sleep(self.cfg.retry_delay);
                }
            }
        }
        self.match_finger()
    }

    fn enrollment_update(&mut self, prev: &[u8]) -> Result<Vec<u8>> {
        let mut cmd = vec![0x6b];
        cmd.extend_from_slice(prev);
        self.with_write(|d| {
            let rsp = d.app(&cmd)?;
            check_status(&rsp)?;
            Ok(rsp[2..].to_vec())
        })
    }

    #[allow(clippy::type_complexity)]
    fn enroll_step(&mut self, key: &mut u32, template: &mut Vec<u8>) -> Result<(Option<Vec<u8>>, Option<Vec<u8>>)> {
        self.glow_start_scan()?;
        self.capture(Mode::Enroll)?;
        let rsp = self.app(&Writer::new().u8(0x68).u32(*key).u32(0).done())?;
        check_status(&rsp)?;
        *key = Reader::new(&rsp[2..]).u32()?;
        self.usb.wait_int_for(COMMITTED_TIMEOUT)?;
        self.enrollment_update(template)?;
        self.usb.wait_int_for(COMMITTED_TIMEOUT)?;
        let res = self.enrollment_update(template)?;
        let (header, tmpl, tid) = parse_enroll_update(&res)?;
        if let Some(t) = tmpl {
            *template = t;
        }
        Ok((header, tid))
    }

    /// Enroll one finger for `sid`; returns the new finger record id.
    pub fn enroll(&mut self, sid: &Sid, subtype: u16, progress: &mut dyn FnMut(Progress)) -> Result<u16> {
        let (mut key, mut template) = (0u32, Vec::new());
        check_status(&self.app(&Writer::new().u8(0x69).u32(1).done())?)?;
        let mut failures = 0;
        let tid = loop {
            let step = self.enroll_step(&mut key, &mut template);
            let end = self.app(&Writer::new().u8(0x69).u32(0).done()).and_then(|r| check_status(&r));
            match step {
                Ok((_, Some(tid))) => {
                    end?;
                    progress(Progress::StagePassed);
                    break tid;
                }
                Ok((header, None)) => {
                    end?;
                    failures = 0;
                    debug!("enroll stage passed (header {})", header.map(hex::encode).unwrap_or_default());
                    progress(Progress::StagePassed);
                }
                Err(e) if is_cancelled(&e) => {
                    let _ = self.glow_end_scan();
                    return Err(e);
                }
                Err(e) if is_usb_failure(&e) => return Err(e),
                Err(e) => {
                    failures += 1;
                    debug!("enroll capture failed: {e:#}");
                    progress(Progress::RetryScan(e.to_string()));
                    ensure!(failures < 20, "too many consecutive enrollment failures: {e:#}");
                }
            }
        };
        // python-validity ends the enrollment twice; the second one may report "nothing to end".
        let _ = self.app(&Writer::new().u8(0x69).u32(0).done());
        ensure!(!template.is_empty(), "enrollment finished without a template");

        let tinfo = make_finger_data(subtype, &template, &tid);
        let user = match self.lookup_user(sid)? {
            Some(u) => {
                // The sensor keeps one record per finger (a second one fails with 04c3):
                // re-enrolling replaces the old print, now that a new template exists.
                for f in u.fingers.iter().filter(|f| f.subtype == subtype) {
                    info!("replacing the enrolled {} (record {})", blobs::finger_name(subtype), f.dbid);
                    self.del_record(f.dbid).context("delete the old print")?;
                }
                u.dbid
            }
            None => self.new_user(sid).context("create the user record")?,
        };
        let rec = self.new_finger(user, &tinfo).context("store the new print")?;
        self.usb.wait_int_for(COMMITTED_TIMEOUT)?;
        self.glow_end_scan()?;
        Ok(rec)
    }

    pub fn reboot(&mut self) -> Result<()> {
        check_status(&self.cmd(&unhex("050200"))?)
    }

    /// Wipe pairing, firmware and templates; the sensor comes back unpaired.
    pub fn factory_reset(&mut self) -> Result<()> {
        check_status(&self.usb.cmd(blobs::RESET_BLOB)?)?;
        let mut c = vec![0x10];
        c.resize(0x62, 0);
        check_status(&self.usb.cmd(&c)?)?;
        self.reboot()
    }

    pub fn led_test(&mut self) -> Result<()> {
        self.glow_start_scan()?;
        std::thread::sleep(Duration::from_millis(1500));
        self.glow_end_scan()
    }
}
