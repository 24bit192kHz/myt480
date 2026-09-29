//! Small byte helpers shared by the protocol modules.

use anyhow::{bail, ensure, Result};

/// Check the little-endian u16 status word that starts every sensor reply.
pub fn check_status(rsp: &[u8]) -> Result<()> {
    ensure!(rsp.len() >= 2, "short reply ({} bytes)", rsp.len());
    match status(rsp) {
        0 => Ok(()),
        0x44f => bail!("signature validation failed: 044f"),
        s => bail!("sensor status {s:04x}"),
    }
}

pub fn status(rsp: &[u8]) -> u16 {
    u16::from_le_bytes([rsp[0], rsp[1]])
}

/// Little-endian cursor over a reply. Reads past the end are errors, not panics.
pub struct Reader<'a> {
    buf: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Reader { buf }
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        ensure!(self.buf.len() >= n, "truncated reply: need {n}, have {}", self.buf.len());
        let (head, tail) = self.buf.split_at(n);
        self.buf = tail;
        Ok(head)
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }
}

/// Little-endian builder for commands.
#[derive(Default)]
pub struct Writer(pub Vec<u8>);

impl Writer {
    pub fn new() -> Self {
        Writer(Vec::new())
    }

    pub fn u8(mut self, v: u8) -> Self {
        self.0.push(v);
        self
    }

    pub fn u16(mut self, v: u16) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn u32(mut self, v: u32) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn bytes(mut self, b: &[u8]) -> Self {
        self.0.extend_from_slice(b);
        self
    }

    pub fn zeros(mut self, n: usize) -> Self {
        self.0.resize(self.0.len() + n, 0);
        self
    }

    pub fn done(self) -> Vec<u8> {
        self.0
    }
}

pub fn unhex(s: &str) -> Vec<u8> {
    let clean: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    hex::decode(clean).expect("valid hex literal")
}
