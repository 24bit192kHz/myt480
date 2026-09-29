//! First-run pairing (flash partitioning + key exchange) and firmware-extension upload.

use std::path::PathBuf;

use anyhow::{Context, Result, bail, ensure};
use log::info;
use p256::SecretKey;
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};

use crate::blobs;
use crate::device::{Device, Rebooted};
use crate::tls::make_cert;
use crate::util::{Writer, check_status};

fn with_hdr(id: u16, buf: &[u8]) -> Vec<u8> {
    Writer::new().u16(id).u16(buf.len() as u16).bytes(buf).done()
}

fn serialize_partition(p: &(u8, u8, u16, u32, u32)) -> Vec<u8> {
    let b = Writer::new().u8(p.0).u8(p.1).u16(p.2).u32(p.3).u32(p.4).done();
    let hash = Sha256::digest(&b);
    Writer::new().bytes(&b).zeros(4).bytes(&hash).done()
}

impl Device {
    /// Pair a factory-fresh sensor. No-op when the flash is already partitioned.
    pub fn init_flash(&mut self) -> Result<()> {
        let info = self.get_flash_info()?;
        if !info.partitions.is_empty() {
            return Ok(());
        }
        info!("sensor flash is blank: pairing with {:?} / {:?}", self.hwkey.product_name, self.hwkey.serial);
        check_status(&self.usb.cmd(blobs::RESET_BLOB)?)?;

        let client = SecretKey::random(&mut OsRng);
        let (_, _, size, sector, erase) = blobs::FLASH_IC;
        let params = Writer::new().u32(size).u32(sector).zeros(2).u8(erase).zeros(1).done();
        let mut parts: Vec<u8> = blobs::FLASH_LAYOUT.iter().flat_map(serialize_partition).collect();
        parts.extend_from_slice(blobs::PARTITION_SIGNATURE);
        let mut cmd = vec![0x4f, 0, 0, 0, 0];
        cmd.extend(with_hdr(0, &params));
        cmd.extend(with_hdr(1, &parts));
        cmd.extend(with_hdr(5, &make_cert(&client.public_key())));
        cmd.extend(with_hdr(3, blobs::CRT_HARDCODED));
        let rsp = self.cmd(&cmd)?;
        check_status(&rsp)?;
        ensure!(rsp.len() >= 6, "short partition reply");
        let crt_len = u32::from_le_bytes(rsp[2..6].try_into().unwrap()) as usize;
        self.tls.tls_cert = rsp.get(6..6 + crt_len).context("short host cert")?.to_vec();

        self.rom_info()?;
        let rsp = self.usb.cmd(&[0x50]);
        let cleanup = self.call_cleanups();
        let rsp = rsp?;
        check_status(&rsp)?;
        cleanup?;
        let rsp = &rsp[2..];
        ensure!(rsp.len() >= 404, "short ECDH reply");
        let l = u32::from_le_bytes(rsp[..4].try_into().unwrap()) as usize;
        ensure!(l == rsp.len(), "ECDH reply length mismatch");
        let (zeroes, ecdh) = rsp[4..].split_at(rsp.len() - 4 - 400);
        ensure!(zeroes.iter().all(|&b| b == 0), "expected zeroes before the ECDH blob");
        self.tls.handle_ecdh(ecdh)?;
        let blob = self.tls.encrypt_key(&client);
        self.tls.handle_priv(&blob)?;
        self.tls.open(&self.usb).context("TLS handshake after pairing")?;

        for p in [1, 2, 5, 6, 4] {
            self.erase_flash(p)?;
        }
        let tls_flash = self.tls.make_tls_flash();
        self.write_flash(1, 0, &tls_flash)?;
        info!("pairing stored; rebooting the sensor");
        self.reboot()?;
        Err(Rebooted.into())
    }

    fn fwext_path(&self) -> Result<PathBuf> {
        std::iter::once(&self.cfg.data_dir)
            .chain(&self.cfg.import_dirs)
            .map(|d| d.join(blobs::FWEXT_NAME))
            .find(|p| p.is_file())
            .with_context(|| format!("firmware {} not found; extract it with validity-sensors-firmware", blobs::FWEXT_NAME))
    }

    /// Upload the firmware extension when the sensor has none (after pairing).
    pub fn upload_fwext(&mut self) -> Result<()> {
        if let Some(fw) = self.get_fw_info(2)? {
            info!("firmware extension {}.{} ({} modules, built {})", fw.major, fw.minor, fw.modules, fw.buildtime);
            return Ok(());
        }
        info!("no firmware extension on the sensor; uploading");
        self.write_hw_reg32(0x8000_205c, 7)?;
        let r = self.read_hw_reg32(0x8000_2080)?;
        ensure!(r == 2 || r == 3, "unexpected register value {r:#x}");
        self.identify_sensor()?;

        let path = self.fwext_path()?;
        let file = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        let start = file.iter().position(|&b| b == 0x1a).context("fwext: no 0x1a header terminator")? + 1;
        let body = &file[start..];
        ensure!(body.len() > 0x100, "fwext too short");
        let (fw, sig) = body.split_at(body.len() - 0x100);
        self.write_flash_all(2, 0, fw)?;
        self.write_fw_signature(2, sig)?;
        let Some(fw) = self.get_fw_info(2)? else { bail!("firmware upload did not take") };
        info!("loaded firmware extension {}.{} ({} modules)", fw.major, fw.minor, fw.modules);
        self.reboot()?;
        Err(Rebooted.into())
    }
}
