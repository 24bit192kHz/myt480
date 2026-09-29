//! The sensor's TLS 1.2 dialect: ECDH_ECDSA_WITH_AES_256_CBC with HMAC-SHA256
//! records, a pre-shared "GWK" key tied to the host's DMI identity, and a
//! handful of non-standard framing quirks copied from python-validity.

use aes::Aes256;
use anyhow::{Context, Result, bail, ensure};
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::NoPadding};
use hmac::{Hmac, Mac};
use log::trace;
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::signature::hazmat::PrehashSigner;
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use p256::elliptic_curve::sec1::{FromEncodedPoint, ToEncodedPoint};
use p256::{EncodedPoint, PublicKey, SecretKey};
use rand::RngCore;
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};

use crate::usb::Transport;
use crate::util::unhex;

type HmacSha256 = Hmac<Sha256>;

const PASSWORD_HARDCODED: &str = "717cd72d0962bc4a2846138dbb2c24192512a76407065f383846139d4bec2033";
const GWK_SIGN_HARDCODED: &str = "3a4c76b76a97981d1274247e166610e77f4d9c9d07d3c728e532916bdd28b454";

/// Firmware ECDH-signing key hardcoded in synaWudfBioUsb.dll.
const FW_PUB_X: &str = "f727653b4e16ce0665a6894d7f3a30d7d0a0be310d1292a743671fdf69f6a8d3";
const FW_PUB_Y: &str = "a85538f8b6bec50d6eef8bd5f4d07a886243c58b2393948df761a84721a6ca94";

pub fn hmac256(key: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut m = HmacSha256::new_from_slice(key).expect("hmac accepts any key length");
    for p in parts {
        m.update(p);
    }
    m.finalize().into_bytes().into()
}

/// TLS 1.2 P_SHA256.
pub fn prf(secret: &[u8], seed: &[u8], len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len + 32);
    let mut a = hmac256(secret, &[seed]);
    while out.len() < len {
        out.extend_from_slice(&hmac256(secret, &[&a, seed]));
        a = hmac256(secret, &[&a]);
    }
    out.truncate(len);
    out
}

/// Private key the host uses to sign its pairing certificate.
pub fn hs_key() -> SecretKey {
    let pw = unhex(PASSWORD_HARDCODED);
    let mut seed = b"HS_KEY_PAIR_GEN".to_vec();
    seed.extend_from_slice(&pw[0x10..]);
    seed.extend_from_slice(&[0xaa, 0xaa]);
    let mut k = prf(&pw[..0x10], &seed, 0x20);
    k.reverse(); // little-endian scalar
    SecretKey::from_slice(&k).expect("hs key is a valid scalar")
}

/// The host identity that binds a pairing: DMI product name and serial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HwKey {
    pub product_name: String,
    pub serial: String,
}

impl HwKey {
    pub fn from_dmi() -> HwKey {
        let read = |f: &str| std::fs::read_to_string(format!("/sys/class/dmi/id/{f}"));
        match (read("product_name"), read("product_serial")) {
            (Ok(n), Ok(s)) => HwKey { product_name: n.trim().into(), serial: s.trim().into() },
            _ => HwKey { product_name: "VirtualBox".into(), serial: "0".into() },
        }
    }

    /// (psk_encryption_key, psk_validation_key)
    pub fn psk(&self) -> ([u8; 32], [u8; 32]) {
        let mut hw = self.product_name.as_bytes().to_vec();
        hw.push(0);
        hw.extend_from_slice(self.serial.as_bytes());
        hw.push(0);
        let pw = unhex(PASSWORD_HARDCODED);
        let enc: [u8; 32] = prf(&pw, &[b"GWK".as_slice(), &hw].concat(), 0x20).try_into().unwrap();
        let sign_seed = [b"GWK_SIGN".as_slice(), &unhex(GWK_SIGN_HARDCODED)].concat();
        let val: [u8; 32] = prf(&enc, &sign_seed, 0x20).try_into().unwrap();
        (enc, val)
    }
}

fn aes_cbc_encrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Vec<u8> {
    cbc::Encryptor::<Aes256>::new_from_slices(key, iv)
        .expect("32-byte key, 16-byte iv")
        .encrypt_padded_vec_mut::<NoPadding>(data)
}

fn aes_cbc_decrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    ensure!(data.len().is_multiple_of(16) && !data.is_empty(), "ciphertext not block aligned");
    cbc::Decryptor::<Aes256>::new_from_slices(key, iv)
        .expect("32-byte key, 16-byte iv")
        .decrypt_padded_vec_mut::<NoPadding>(data)
        .map_err(|_| anyhow::anyhow!("aes-cbc decrypt failed"))
}

fn random<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    OsRng.fill_bytes(&mut b);
    b
}

fn le32(b: &[u8]) -> [u8; 32] {
    let mut v: [u8; 32] = b.try_into().expect("32 bytes");
    v.reverse();
    v
}

pub fn point_from_le(x: &[u8], y: &[u8]) -> Result<PublicKey> {
    let p = EncodedPoint::from_affine_coordinates(&le32(x).into(), &le32(y).into(), false);
    Option::from(PublicKey::from_encoded_point(&p)).context("point is not on P-256")
}

/// (x, y) of a public key, little-endian as the sensor stores them.
pub fn point_to_le(p: &PublicKey) -> ([u8; 32], [u8; 32]) {
    let e = p.to_encoded_point(false);
    let mut x: [u8; 32] = e.x().unwrap().as_slice().try_into().unwrap();
    let mut y: [u8; 32] = e.y().unwrap().as_slice().try_into().unwrap();
    x.reverse();
    y.reverse();
    (x, y)
}

fn size3(b: &[u8]) -> Vec<u8> {
    let n = b.len();
    let mut v = vec![(n >> 16) as u8, (n >> 8) as u8, n as u8];
    v.extend_from_slice(b);
    v
}

fn size2(b: &[u8]) -> Vec<u8> {
    let mut v = (b.len() as u16).to_be_bytes().to_vec();
    v.extend_from_slice(b);
    v
}

/// Pad to the block size TLS-style: every pad byte holds (pad length - 1).
fn pad(b: &[u8]) -> Vec<u8> {
    let l = 16 - b.len() % 16;
    let mut v = b.to_vec();
    v.resize(b.len() + l, (l - 1) as u8);
    v
}

pub struct Tls {
    pub psk_enc: [u8; 32],
    pub psk_val: [u8; 32],
    pub priv_key: Option<SigningKey>,
    pub priv_blob: Vec<u8>,
    pub tls_cert: Vec<u8>,
    pub ecdh_q: Option<PublicKey>,
    pub ecdh_blob: Vec<u8>,
    pub secure_rx: bool,
    pub secure_tx: bool,
    hs_hash: Sha256,
    client_random: [u8; 32],
    server_random: Vec<u8>,
    master_secret: Vec<u8>,
    sign_key: Vec<u8>,
    validation_key: Vec<u8>,
    encryption_key: Vec<u8>,
    decryption_key: Vec<u8>,
    session_public: Option<PublicKey>,
}

impl Tls {
    pub fn new(hw: &HwKey) -> Tls {
        let (psk_enc, psk_val) = hw.psk();
        Tls {
            psk_enc,
            psk_val,
            priv_key: None,
            priv_blob: vec![],
            tls_cert: vec![],
            ecdh_q: None,
            ecdh_blob: vec![],
            secure_rx: false,
            secure_tx: false,
            hs_hash: Sha256::new(),
            client_random: [0; 32],
            server_random: vec![],
            master_secret: vec![],
            sign_key: vec![],
            validation_key: vec![],
            encryption_key: vec![],
            decryption_key: vec![],
            session_public: None,
        }
    }

    pub fn reset(&mut self) {
        self.secure_rx = false;
        self.secure_tx = false;
    }

    pub fn is_secure(&self) -> bool {
        self.secure_rx && self.secure_tx
    }

    /// Send a command through the TLS channel once it is up, raw before that.
    pub fn cmd(&mut self, usb: &impl Transport, cmd: &[u8]) -> Result<Vec<u8>> {
        if self.is_secure() { self.app(usb, cmd) } else { usb.cmd(cmd) }
    }

    pub fn app(&mut self, usb: &impl Transport, cmd: &[u8]) -> Result<Vec<u8>> {
        let rec = self.make_app_data(cmd)?;
        let rsp = usb.cmd(&rec)?;
        self.parse_response(&rsp)
    }

    // ---------------------------------------------------------------- handshake

    pub fn open(&mut self, usb: &impl Transport) -> Result<()> {
        self.reset();
        self.hs_hash = Sha256::new();

        let hello = self.make_client_hello();
        let mut out = vec![0x44, 0, 0, 0];
        out.extend(self.make_handshake(&hello));
        let rsp = usb.cmd(&out)?;
        self.parse_response(&rsp).context("server hello")?;

        self.make_keys()?;

        let mut flight = self.make_certs();
        flight.extend(self.make_client_kex());
        flight.extend(self.make_cert_verify()?);
        let mut out = vec![0x44, 0, 0, 0];
        out.extend(self.make_handshake(&flight));
        out.extend([0x14, 0x03, 0x03, 0x00, 0x01, 0x01]); // ChangeCipherSpec
        let finish = self.make_finish();
        out.extend(self.make_handshake(&finish));
        let rsp = usb.cmd(&out)?;
        if let Err(e) = self.parse_response(&rsp) {
            self.reset();
            return Err(e.context("server finished"));
        }
        ensure!(self.is_secure(), "handshake completed without a secure channel");
        Ok(())
    }

    fn with_neg_hdr(&mut self, t: u8, b: &[u8]) -> Vec<u8> {
        let mut v = vec![t];
        v.extend(size3(b));
        self.hs_hash.update(&v);
        v
    }

    fn make_client_hello(&mut self) -> Vec<u8> {
        self.client_random = random();
        let mut h = vec![0x03, 0x03];
        h.extend_from_slice(&self.client_random);
        h.push(7);
        h.extend_from_slice(&[0; 7]); // session id
        h.extend(size2(&[0xc0, 0x05, 0x00, 0x3d, 0x00, 0x8d]));
        h.push(0); // no compression
        let mut exts = Vec::new();
        exts.extend([0x00, 0x04]); // truncated_hmac
        exts.extend(size2(&[0x00, 0x17]));
        exts.extend([0x00, 0x0b]); // ec point formats: uncompressed
        exts.extend(size2(&[0x01, 0x00]));
        // The sensor wants the extensions length understated by 2.
        h.extend(((exts.len() - 2) as u16).to_be_bytes());
        h.extend(exts);
        self.with_neg_hdr(0x01, &h)
    }

    fn make_keys(&mut self) -> Result<()> {
        let q = self.ecdh_q.context("no sensor ECDH key")?;
        let sk = p256::ecdh::EphemeralSecret::random(&mut OsRng);
        self.session_public = Some(PublicKey::from(&sk));
        let pms = sk.diffie_hellman(&q);
        let seed = [self.client_random.as_slice(), &self.server_random].concat();
        self.master_secret = prf(pms.raw_secret_bytes(), &[b"master secret".as_slice(), &seed].concat(), 0x30);
        let kb = prf(&self.master_secret, &[b"key expansion".as_slice(), &seed].concat(), 0x120);
        self.sign_key = kb[0x00..0x20].to_vec();
        self.validation_key = kb[0x20..0x40].to_vec();
        self.encryption_key = kb[0x40..0x60].to_vec();
        self.decryption_key = kb[0x60..0x80].to_vec();
        Ok(())
    }

    fn make_certs(&mut self) -> Vec<u8> {
        let n = self.tls_cert.len();
        // Both length prefixes carry the bare cert length (not the standard nested lengths).
        let mut c = vec![0, (n >> 8) as u8, n as u8, 0, (n >> 8) as u8, n as u8, 0xac, 0x16];
        c.extend_from_slice(&self.tls_cert);
        self.with_neg_hdr(0x0b, &c)
    }

    fn make_client_kex(&mut self) -> Vec<u8> {
        let e = self.session_public.unwrap().to_encoded_point(false);
        let mut b = vec![0x04];
        b.extend_from_slice(e.x().unwrap());
        b.extend_from_slice(e.y().unwrap());
        self.with_neg_hdr(0x10, &b)
    }

    fn make_cert_verify(&mut self) -> Result<Vec<u8>> {
        let hash = self.hs_hash.clone().finalize();
        let key = self.priv_key.as_ref().context("no pairing key")?;
        let sig: Signature = key.sign_prehash(&hash).context("sign cert verify")?;
        Ok(self.with_neg_hdr(0x0f, sig.to_der().as_bytes()))
    }

    fn make_finish(&mut self) -> Vec<u8> {
        self.secure_tx = true;
        let hash = self.hs_hash.clone().finalize();
        let vd = prf(&self.master_secret, &[b"client finished".as_slice(), &hash].concat(), 0xc);
        let mut v = vec![0x14];
        v.extend(size3(&vd));
        v
    }

    fn make_handshake(&mut self, b: &[u8]) -> Vec<u8> {
        let body = if self.secure_tx { self.encrypt(&self.sign(0x16, b)) } else { b.to_vec() };
        let mut v = vec![0x16, 0x03, 0x03];
        v.extend(size2(&body));
        v
    }

    fn make_app_data(&mut self, b: &[u8]) -> Result<Vec<u8>> {
        ensure!(self.secure_tx, "app payload before secure connection established");
        let body = self.encrypt(&self.sign(0x17, b));
        let mut v = vec![0x17, 0x03, 0x03];
        v.extend(size2(&body));
        Ok(v)
    }

    // ------------------------------------------------------------------ records

    fn record_mac(key: &[u8], t: u8, b: &[u8]) -> [u8; 32] {
        let n = b.len() as u16;
        hmac256(key, &[&[t, 3, 3, (n >> 8) as u8, n as u8], b])
    }

    fn sign(&self, t: u8, b: &[u8]) -> Vec<u8> {
        trace!(">tls> {t:02x}: {}", hex::encode(b));
        let mut v = b.to_vec();
        v.extend_from_slice(&Self::record_mac(&self.sign_key, t, b));
        v
    }

    fn validate(&self, t: u8, b: &[u8]) -> Result<Vec<u8>> {
        ensure!(b.len() >= 32, "record too short for a MAC");
        let (body, mac) = b.split_at(b.len() - 32);
        let mut m = HmacSha256::new_from_slice(&self.validation_key).unwrap();
        let n = body.len() as u16;
        m.update(&[t, 3, 3, (n >> 8) as u8, n as u8]);
        m.update(body);
        m.verify_slice(mac).map_err(|_| anyhow::anyhow!("packet signature validation check failed"))?;
        trace!("<tls< {t:02x}: {}", hex::encode(body));
        Ok(body.to_vec())
    }

    fn encrypt(&self, b: &[u8]) -> Vec<u8> {
        let iv: [u8; 16] = random();
        let mut v = iv.to_vec();
        v.extend(aes_cbc_encrypt(&self.encryption_key, &iv, &pad(b)));
        v
    }

    fn decrypt(&self, c: &[u8]) -> Result<Vec<u8>> {
        ensure!(c.len() >= 32, "ciphertext too short");
        let (iv, c) = c.split_at(16);
        let mut m = aes_cbc_decrypt(&self.decryption_key, iv, c)?;
        let p = *m.last().unwrap() as usize + 1;
        ensure!(p <= m.len(), "bad padding");
        m.truncate(m.len() - p);
        Ok(m)
    }

    pub fn parse_response(&mut self, rsp: &[u8]) -> Result<Vec<u8>> {
        let mut rsp = rsp.to_vec();
        let mut app = Vec::new();
        while !rsp.is_empty() {
            if rsp.len() < 5 {
                rsp.resize(5, 0);
            }
            let (t, mj, mn) = (rsp[0], rsp[1], rsp[2]);
            let sz = u16::from_be_bytes([rsp[3], rsp[4]]) as usize;
            let end = (5 + sz).min(rsp.len());
            let pkt = rsp[5..end].to_vec();
            rsp.drain(..end);
            ensure!(mj == 3 && mn == 3, "unexpected TLS version {mj} {mn}");
            match t {
                0x16 => self.handle_handshake(&pkt)?,
                0x14 => {
                    ensure!(pkt == [1], "unexpected ChangeCipherSpec payload");
                    self.secure_rx = true;
                }
                0x17 => {
                    ensure!(self.secure_rx, "app payload before secure connection established");
                    app.extend(self.validate(0x17, &self.decrypt(&pkt)?)?);
                }
                _ => bail!("unknown TLS record type {t:02x}"),
            }
        }
        Ok(app)
    }

    fn handle_handshake(&mut self, pkt: &[u8]) -> Result<()> {
        let mut hs = if self.secure_rx { self.validate(0x16, &self.decrypt(pkt)?)? } else { pkt.to_vec() };
        while !hs.is_empty() {
            if hs.len() < 4 {
                hs.resize(4, 0);
            }
            let t = hs[0];
            let l = (hs[1] as usize) << 16 | (hs[2] as usize) << 8 | hs[3] as usize;
            let end = (4 + l).min(hs.len());
            let hdr = hs[..4].to_vec();
            let p = hs[4..end].to_vec();
            hs.drain(..end);
            match t {
                0x02 => self.handle_server_hello(&p)?,
                0x0d => {
                    ensure!(p.len() >= 4, "short certificate request");
                    let algo = u16::from_be_bytes([p[0], p[1]]);
                    ensure!(algo == 0x140, "unsupported sign/hash algo {algo:04x}");
                    ensure!(p[2..] == [0, 0], "server sent a non-empty CA list");
                }
                0x0e => ensure!(p.is_empty(), "unexpected body in server hello done"),
                0x14 => {
                    let hash = self.hs_hash.clone().finalize();
                    let vd = prf(&self.master_secret, &[b"server finished".as_slice(), &hash].concat(), 0xc);
                    ensure!(vd == p, "final handshake check failed");
                }
                _ => bail!("unknown handshake packet {t:02x}"),
            }
            self.hs_hash.update(&hdr);
            self.hs_hash.update(&p);
        }
        Ok(())
    }

    fn handle_server_hello(&mut self, p: &[u8]) -> Result<()> {
        ensure!(p.len() >= 35 && p[..2] == [3, 3], "unexpected TLS version in server hello");
        self.server_random = p[2..34].to_vec();
        let l = p[34] as usize;
        let rest = p.get(35 + l..).context("short server hello")?;
        ensure!(rest.len() == 3, "unexpected server hello tail");
        let suite = u16::from_be_bytes([rest[0], rest[1]]);
        ensure!(suite == 0xc005, "server chose unsupported cipher suite {suite:04x}");
        ensure!(rest[2] == 0, "server enabled compression");
        Ok(())
    }

    // ----------------------------------------------------- pairing data (flash)

    /// Parse partition 1 (cert store): private key, sensor ECDH key, host cert.
    pub fn parse_tls_flash(&mut self, mut reply: &[u8]) -> Result<()> {
        while reply.len() >= 4 {
            let id = u16::from_le_bytes([reply[0], reply[1]]);
            let sz = u16::from_le_bytes([reply[2], reply[3]]) as usize;
            if id == 0xffff {
                break;
            }
            ensure!(reply.len() >= 0x24 + sz, "truncated tls flash block {id:04x}");
            let hs = &reply[4..0x24];
            let body = &reply[0x24..0x24 + sz];
            reply = &reply[0x24 + sz..];
            ensure!(Sha256::digest(body).as_slice() == hs, "tls flash block {id:04x}: hash mismatch");
            match id {
                4 => self.handle_priv(body)?,
                6 => self.handle_ecdh(body)?,
                3 => self.tls_cert = body.to_vec(),
                0..=2 => ensure!(body.iter().all(|&b| b == 0), "expected empty block {id}"),
                _ => trace!("unhandled tls flash block {id:04x} ({sz} bytes)"),
            }
        }
        ensure!(self.priv_key.is_some(), "no private key in tls flash (sensor not paired)");
        ensure!(self.ecdh_q.is_some(), "no ECDH key in tls flash");
        Ok(())
    }

    pub fn handle_ecdh(&mut self, body: &[u8]) -> Result<()> {
        ensure!(body.len() >= 0x94, "short ECDH blob");
        let key = &body[..0x90];
        let q = point_from_le(&key[0x08..0x28], &key[0x4c..0x6c])?;
        let l = u32::from_le_bytes(body[0x90..0x94].try_into().unwrap()) as usize;
        let sig = body.get(0x94..0x94 + l).context("short ECDH signature")?;
        ensure!(body[0x94 + l..].iter().all(|&b| b == 0), "zeroes expected after ECDH signature");
        let fw = EncodedPoint::from_affine_coordinates(
            unhex(FW_PUB_X).as_slice().into(),
            unhex(FW_PUB_Y).as_slice().into(),
            false,
        );
        let vk = VerifyingKey::from_encoded_point(&fw).context("firmware key")?;
        let sig = Signature::from_der(sig).context("ECDH signature encoding")?;
        let sig = sig.normalize_s().unwrap_or(sig);
        vk.verify(key, &sig).context("sensor ECDH key is not signed by Synaptics firmware")?;
        self.ecdh_blob = body.to_vec();
        self.ecdh_q = Some(q);
        Ok(())
    }

    pub fn handle_priv(&mut self, body: &[u8]) -> Result<()> {
        ensure!(body.len() > 0x31 && body[0] == 2, "unknown private key blob format");
        let (c, mac) = body[1..].split_at(body.len() - 1 - 0x20);
        let mut m = HmacSha256::new_from_slice(&self.psk_val).unwrap();
        m.update(c);
        m.verify_slice(mac).map_err(|_| PairingMismatch)?;
        let (iv, c) = c.split_at(16);
        let mut m = aes_cbc_decrypt(&self.psk_enc, iv, c)?;
        let p = *m.last().unwrap() as usize;
        ensure!((1..=16).contains(&p) && p <= m.len(), "bad private key padding");
        m.truncate(m.len() - p);
        ensure!(m.len() >= 0x60, "short private key");
        // x and y may be zero when the pairing came from the Windows driver; use d only.
        let d = le32(&m[0x40..0x60]);
        let sk = SecretKey::from_slice(&d).context("invalid private scalar")?;
        self.priv_key = Some(SigningKey::from(sk));
        self.priv_blob = body.to_vec();
        Ok(())
    }

    /// Serialize partition 1 for a fresh pairing.
    pub fn make_tls_flash(&self) -> Vec<u8> {
        let block = |id: u16, body: &[u8]| {
            let mut v = id.to_le_bytes().to_vec();
            v.extend((body.len() as u16).to_le_bytes());
            v.extend(Sha256::digest(body));
            v.extend_from_slice(body);
            v
        };
        let mut b = block(0, &[0]);
        b.extend(block(4, &self.priv_blob));
        b.extend(block(3, &self.tls_cert));
        b.extend(block(5, crate::blobs::CRT_HARDCODED));
        b.extend(block(1, &[0; 0x100]));
        b.extend(block(2, &[0; 0x100]));
        b.extend(block(6, &self.ecdh_blob));
        b.resize(0x1000, 0xff);
        b
    }

    /// Wrap a freshly generated pairing key the way the sensor stores it.
    pub fn encrypt_key(&self, sk: &SecretKey) -> Vec<u8> {
        let (x, y) = point_to_le(&sk.public_key());
        let mut d: [u8; 32] = sk.to_bytes().into();
        d.reverse();
        let mut m = [x, y, d].concat();
        let l = 16 - m.len() % 16;
        m.resize(m.len() + l, l as u8); // standard PKCS#7 here
        let iv: [u8; 16] = random();
        let mut c = iv.to_vec();
        c.extend(aes_cbc_encrypt(&self.psk_enc, &iv, &m));
        let mac = hmac256(&self.psk_val, &[&c]);
        let mut v = vec![2];
        v.extend(c);
        v.extend(mac);
        v
    }
}

/// The pairing blob was sealed with a different host identity.
#[derive(Debug)]
pub struct PairingMismatch;

impl std::fmt::Display for PairingMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("pairing key does not match this host identity (paired with another computer or firmware?)")
    }
}

impl std::error::Error for PairingMismatch {}

/// Host certificate for pairing: our public key signed with the hardcoded hs key.
pub fn make_cert(client: &PublicKey) -> Vec<u8> {
    use p256::ecdsa::signature::Signer;
    let (x, y) = point_to_le(client);
    let mut msg = Vec::new();
    msg.extend(0x17u32.to_le_bytes());
    msg.extend(0x20u32.to_le_bytes());
    msg.extend(x);
    msg.extend([0u8; 0x24]);
    msg.extend(y);
    msg.extend([0u8; 0x4c]);
    let sig: Signature = SigningKey::from(hs_key()).sign(&msg);
    let der = sig.to_der();
    msg.extend((der.as_bytes().len() as u32).to_le_bytes());
    msg.extend_from_slice(der.as_bytes());
    msg.resize(444, 0);
    msg
}
