//! Sensor SPI-flash partitions: cert store, firmware extension, calibration, template DB.

use anyhow::{Result, ensure};

use crate::blobs;
use crate::device::Device;
use crate::util::{Reader, Writer, check_status, status};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partition {
    pub id: u8,
    pub typ: u8,
    pub access: u16,
    pub offset: u32,
    pub size: u32,
}

#[derive(Debug)]
pub struct FlashInfo {
    pub jid0: u16,
    pub jid1: u16,
    pub blocks: u16,
    pub blocksize: u16,
    pub partitions: Vec<Partition>,
}

#[derive(Debug)]
pub struct FwInfo {
    pub major: u16,
    pub minor: u16,
    pub modules: u16,
    pub buildtime: u32,
}

impl Device {
    pub fn get_flash_info(&mut self) -> Result<FlashInfo> {
        let rsp = self.cmd(&[0x3e])?;
        check_status(&rsp)?;
        let mut r = Reader::new(&rsp[2..]);
        let (jid0, jid1, blocks, _u0, blocksize, _u1, pcnt) =
            (r.u16()?, r.u16()?, r.u16()?, r.u16()?, r.u16()?, r.u16()?, r.u16()?);
        let (fj0, fj1, fsize, _, _) = blobs::FLASH_IC;
        ensure!(
            jid0 == fj0 && jid1 == fj1 && blocks as u32 * blocksize as u32 == fsize,
            "unknown flash IC {jid0:x}:{jid1:x} {blocks}x{blocksize}"
        );
        let mut partitions = Vec::new();
        for _ in 0..pcnt {
            let mut p = Reader::new(r.take(12)?);
            partitions.push(Partition { id: p.u8()?, typ: p.u8()?, access: p.u16()?, offset: p.u32()?, size: p.u32()? });
        }
        Ok(FlashInfo { jid0, jid1, blocks, blocksize, partitions })
    }

    /// Firmware extension info for a partition; `None` when nothing is loaded.
    pub fn get_fw_info(&mut self, partition: u8) -> Result<Option<FwInfo>> {
        let rsp = self.cmd(&[0x43, partition])?;
        if rsp.len() == 2 && status(&rsp) == 0x04b0 {
            return Ok(None);
        }
        check_status(&rsp)?;
        let mut r = Reader::new(&rsp[2..]);
        Ok(Some(FwInfo { major: r.u16()?, minor: r.u16()?, modules: r.u16()?, buildtime: r.u32()? }))
    }

    pub fn write_enable(&mut self) -> Result<()> {
        check_status(&self.cmd(blobs::DB_WRITE_ENABLE)?)
    }

    pub fn call_cleanups(&mut self) -> Result<()> {
        let rsp = self.cmd(&[0x1a])?;
        if rsp.len() >= 2 && status(&rsp) == 0x0491 {
            return Ok(()); // nothing to commit
        }
        check_status(&rsp)
    }

    /// Run `f` with DB writes enabled, always committing afterwards.
    pub fn with_write<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let res = self.write_enable().and_then(|_| f(self));
        let cleanup = self.call_cleanups();
        let v = res?;
        cleanup?;
        Ok(v)
    }

    pub fn erase_flash(&mut self, partition: u8) -> Result<()> {
        self.with_write(|d| check_status(&d.cmd(&[0x3f, partition])?))
    }

    pub fn read_flash(&mut self, partition: u8, addr: u32, size: u32) -> Result<Vec<u8>> {
        let cmd = Writer::new().u8(0x40).u8(partition).u8(1).u16(0).u32(addr).u32(size).done();
        let rsp = self.cmd(&cmd)?;
        check_status(&rsp)?;
        ensure!(rsp.len() >= 8, "short flash read reply");
        let sz = u32::from_le_bytes(rsp[2..6].try_into().unwrap()) as usize;
        ensure!(rsp.len() >= 8 + sz, "flash read returned {} of {sz} bytes", rsp.len() - 8);
        Ok(rsp[8..8 + sz].to_vec())
    }

    pub fn read_flash_all(&mut self, partition: u8, start: u32, size: u32) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(size as usize);
        let mut addr = start;
        while addr < start + size {
            out.extend(self.read_flash(partition, addr, 0x1000)?);
            addr += 0x1000;
        }
        out.truncate(size as usize);
        Ok(out)
    }

    pub fn write_flash(&mut self, partition: u8, addr: u32, buf: &[u8]) -> Result<()> {
        // python-validity ignores the write-enable status here; keep that.
        self.cmd(blobs::DB_WRITE_ENABLE)?;
        let cmd = Writer::new().u8(0x41).u8(partition).u8(1).u16(0).u32(addr).u32(buf.len() as u32).bytes(buf).done();
        let res = self.cmd(&cmd).and_then(|r| check_status(&r));
        let cleanup = self.call_cleanups();
        res?;
        cleanup
    }

    pub fn write_flash_all(&mut self, partition: u8, mut addr: u32, buf: &[u8]) -> Result<()> {
        for chunk in buf.chunks(0x1000) {
            self.write_flash(partition, addr, chunk)?;
            addr += chunk.len() as u32;
        }
        Ok(())
    }

    pub fn write_fw_signature(&mut self, partition: u8, sig: &[u8]) -> Result<()> {
        let cmd = Writer::new().u8(0x42).u8(partition).u8(0).u16(sig.len() as u16).bytes(sig).done();
        check_status(&self.cmd(&cmd)?)
    }

    pub fn read_tls_flash(&mut self) -> Result<Vec<u8>> {
        self.read_flash(1, 0, 0x1000)
    }
}
