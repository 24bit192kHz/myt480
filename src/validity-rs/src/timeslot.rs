//! Capture-program chunks and the timeslot-table instruction decoder.

use anyhow::{Result, bail, ensure};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub typ: u16,
    pub data: Vec<u8>,
}

pub fn split_chunks(mut b: &[u8]) -> Result<Vec<Chunk>> {
    let mut out = Vec::new();
    while !b.is_empty() {
        ensure!(b.len() >= 4, "truncated chunk header");
        let typ = u16::from_le_bytes([b[0], b[1]]);
        let sz = u16::from_le_bytes([b[2], b[3]]) as usize;
        let end = (4 + sz).min(b.len());
        out.push(Chunk { typ, data: b[4..end].to_vec() });
        b = &b[end..];
    }
    Ok(out)
}

pub fn merge_chunks(cs: &[Chunk]) -> Vec<u8> {
    let mut v = Vec::new();
    for c in cs {
        v.extend(c.typ.to_le_bytes());
        v.extend((c.data.len() as u16).to_le_bytes());
        v.extend_from_slice(&c.data);
    }
    v
}

/// A decoded instruction: opcode number, byte length, operands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Insn {
    pub op: u8,
    pub len: usize,
    pub a: u32,
    pub b: u32,
}

pub fn decode(b: &[u8]) -> Result<Insn> {
    ensure!(!b.is_empty(), "decode past end of table");
    let b0 = b[0];
    let at = |i: usize| -> Result<u8> {
        match b.get(i) {
            Some(&v) => Ok(v),
            None => bail!("truncated instruction {b0:02x}"),
        }
    };
    let i = |op, len, a, b| Ok(Insn { op, len, a, b });
    match b0 {
        0..=4 => i(b0, 1, 0, 0),
        5 | 6 => i(b0, 2, at(1)? as u32, 0),
        7 => {
            let v = at(1)?;
            i(7, 2, if v == 0 { 0x100 } else { v as u32 }, 0)
        }
        _ if b0 & 0xfe == 8 => i(8, 2, ((b0 & 1) as u32) << 8 | at(1)? as u32, 0),
        _ if b0 & 0xfe == 0xa => i(9, 2, ((b0 & 1) as u32) << 8 | at(1)? as u32, 0),
        _ if b0 & 0xfc == 0xc => i(10, 1, (b0 & 3) as u32, 0),
        // Call: rx_inc, addr, repeat (repeat is not needed by the callers)
        _ if b0 & 0xf8 == 0x10 => {
            at(2)?;
            i(11, 3, (b0 & 7) as u32, (at(1)? as u32) << 2)
        }
        _ if b0 & 0xe0 == 0x20 => i(12, 1, (b0 & 0x1f) as u32, 0),
        _ if b0 & 0xc0 == 0x40 => {
            i(13, 3, (b0 & 0x3f) as u32 * 4 + 0x8000_2000, at(1)? as u32 | (at(2)? as u32) << 8)
        }
        _ if b0 & 0xc0 == 0x80 => i(14, 1, ((b0 & 0x38) >> 3) as u32, (b0 & 7) as u32),
        _ if b0 & 0xc0 == 0xc0 => {
            at(1)?;
            i(15, 2, ((b0 & 0x38) >> 3) as u32, (b0 & 7) as u32)
        }
        _ => bail!("unhandled instruction {b0:02x}"),
    }
}

/// Offset of the n-th (1-based) instruction with `opcode`.
pub fn find_nth_insn(b: &[u8], opcode: u8, n: usize) -> Result<usize> {
    let (mut pc, mut left) = (0, n);
    while pc < b.len() {
        let ins = decode(&b[pc..])?;
        ensure!(ins.len <= b.len() - pc, "truncated instruction");
        if ins.op == opcode {
            left -= 1;
            if left == 0 {
                return Ok(pc);
            }
        }
        pc += ins.len;
    }
    bail!("instruction {opcode} #{n} not found")
}

/// Offset of the n-th (1-based) register write to `reg`.
pub fn find_nth_regwrite(b: &[u8], reg: u32, n: usize) -> Result<usize> {
    let (mut pc, mut left) = (0, n);
    while pc < b.len() {
        let ins = decode(&b[pc..])?;
        ensure!(ins.len <= b.len() - pc, "truncated instruction");
        if ins.op == 13 && ins.a == reg {
            left -= 1;
            if left == 0 {
                return Ok(pc);
            }
        }
        pc += ins.len;
    }
    bail!("register write {reg:#x} #{n} not found")
}
