//! An emulated sensor speaking the server side of the TLS dialect in tls.rs,
//! so the handshake and record layer run end to end without hardware. The
//! emulator is written independently of the client and is strict about the
//! wire format (lengths, padding bytes, MACs), so a drift in either shows up.

use std::cell::RefCell;

use aes::Aes256;
use anyhow::{Context, Result, bail, ensure};
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::NoPadding};
use p256::ecdsa::signature::hazmat::PrehashVerifier;
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use p256::{PublicKey, SecretKey};
use rand::RngCore;
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};

use crate::tls::{self, HwKey, Tls};
use crate::usb::Transport;

const CCS: [u8; 6] = [0x14, 0x03, 0x03, 0x00, 0x01, 0x01];

/// Deliberate misbehaviour, armed per test.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Fault {
    #[default]
    None,
    /// Send a server Finished whose verify_data is off by one bit.
    WrongFinished,
    /// Flip a bit in the MAC of the next record the sensor encrypts.
    ServerMac,
    /// Flip a bit in the IV of the next record the client sent.
    ClientRecord,
}

/// Keys from the sensor's point of view: the client's keys with roles swapped.
#[derive(Default)]
struct Keys {
    mac_tx: Vec<u8>, // client validation key
    mac_rx: Vec<u8>, // client sign key
    enc_tx: Vec<u8>, // client decryption key
    enc_rx: Vec<u8>, // client encryption key
}

#[derive(Default)]
struct State {
    hs_hash: Sha256,
    client_random: Vec<u8>,
    server_random: Vec<u8>,
    master_secret: Vec<u8>,
    keys: Keys,
    established: bool,
    /// Every command payload the sensor saw (decrypted app data, or raw commands before TLS).
    received: Vec<Vec<u8>>,
    /// What the sensor answers to the next command.
    reply: Vec<u8>,
    fault: Fault,
}

struct Sensor {
    ecdh: SecretKey,
    client_key: VerifyingKey,
    client_cert: Vec<u8>,
    st: RefCell<State>,
}

fn random(n: usize) -> Vec<u8> {
    let mut b = vec![0u8; n];
    OsRng.fill_bytes(&mut b);
    b
}

fn be24(n: usize) -> [u8; 3] {
    [(n >> 16) as u8, (n >> 8) as u8, n as u8]
}

fn record(t: u8, body: &[u8]) -> Vec<u8> {
    let mut v = vec![t, 3, 3];
    v.extend((body.len() as u16).to_be_bytes());
    v.extend_from_slice(body);
    v
}

fn message(t: u8, body: &[u8]) -> Vec<u8> {
    let mut v = vec![t];
    v.extend(be24(body.len()));
    v.extend_from_slice(body);
    v
}

/// Split a buffer into TLS records, insisting on exact lengths.
fn records(mut b: &[u8]) -> Result<Vec<(u8, Vec<u8>)>> {
    let mut out = Vec::new();
    while !b.is_empty() {
        ensure!(b.len() >= 5, "truncated record header");
        ensure!(b[1..3] == [3, 3], "record version {:02x}{:02x}", b[1], b[2]);
        let n = u16::from_be_bytes([b[3], b[4]]) as usize;
        ensure!(b.len() >= 5 + n, "truncated record body");
        out.push((b[0], b[5..5 + n].to_vec()));
        b = &b[5 + n..];
    }
    Ok(out)
}

/// Split a handshake record into (type, header + body, body).
type Message = (u8, Vec<u8>, Vec<u8>);

fn messages(mut b: &[u8]) -> Result<Vec<Message>> {
    let mut out = Vec::new();
    while !b.is_empty() {
        ensure!(b.len() >= 4, "truncated handshake header");
        let n = (b[1] as usize) << 16 | (b[2] as usize) << 8 | b[3] as usize;
        ensure!(b.len() >= 4 + n, "truncated handshake message {:02x}", b[0]);
        out.push((b[0], b[..4 + n].to_vec(), b[4..4 + n].to_vec()));
        b = &b[4 + n..];
    }
    Ok(out)
}

fn record_mac(key: &[u8], t: u8, b: &[u8]) -> [u8; 32] {
    let n = b.len() as u16;
    tls::hmac256(key, &[&[t, 3, 3, (n >> 8) as u8, n as u8], b])
}

impl Sensor {
    fn new(client_key: VerifyingKey, client_cert: Vec<u8>) -> Sensor {
        Sensor { ecdh: SecretKey::random(&mut OsRng), client_key, client_cert, st: RefCell::default() }
    }

    fn arm(&self, f: Fault) {
        self.st.borrow_mut().fault = f;
    }

    fn set_reply(&self, r: &[u8]) {
        self.st.borrow_mut().reply = r.to_vec();
    }

    fn last_received(&self) -> Vec<u8> {
        self.st.borrow().received.last().cloned().unwrap_or_default()
    }

    fn take_fault(st: &mut State, f: Fault) -> bool {
        let hit = st.fault == f;
        if hit {
            st.fault = Fault::None;
        }
        hit
    }

    /// MAC-then-pad-then-encrypt with the sensor's transmit keys.
    fn seal(st: &mut State, t: u8, b: &[u8]) -> Vec<u8> {
        let mut m = b.to_vec();
        let mut mac = record_mac(&st.keys.mac_tx, t, b);
        if Self::take_fault(st, Fault::ServerMac) {
            mac[7] ^= 0x10;
        }
        m.extend(mac);
        let p = 16 - m.len() % 16;
        m.resize(m.len() + p, (p - 1) as u8);
        let iv = random(16);
        let c = cbc::Encryptor::<Aes256>::new_from_slices(&st.keys.enc_tx, &iv)
            .unwrap()
            .encrypt_padded_vec_mut::<NoPadding>(&m);
        [iv, c].concat()
    }

    /// Decrypt, check every padding byte, then check the MAC. Both failures
    /// read "bad_record_mac", as a TLS alert would.
    fn unseal(st: &mut State, t: u8, c: &[u8]) -> Result<Vec<u8>> {
        let mut c = c.to_vec();
        if Self::take_fault(st, Fault::ClientRecord) {
            c[0] ^= 0x01;
        }
        ensure!(c.len() >= 16 + 48 && c.len().is_multiple_of(16), "bad ciphertext length {}", c.len());
        let (iv, c) = c.split_at(16);
        let mut m = cbc::Decryptor::<Aes256>::new_from_slices(&st.keys.enc_rx, iv)
            .unwrap()
            .decrypt_padded_vec_mut::<NoPadding>(c)
            .map_err(|_| anyhow::anyhow!("decrypt"))?;
        let last = *m.last().unwrap();
        let p = last as usize + 1;
        ensure!(p <= 16 && m[m.len() - p..].iter().all(|&b| b == last), "bad_record_mac (padding)");
        m.truncate(m.len() - p);
        ensure!(m.len() >= 32, "no room for a MAC");
        let (body, mac) = m.split_at(m.len() - 32);
        ensure!(record_mac(&st.keys.mac_rx, t, body) == mac, "bad_record_mac");
        Ok(body.to_vec())
    }

    fn client_hello(&self, st: &mut State, recs: &[(u8, Vec<u8>)]) -> Result<Vec<u8>> {
        ensure!(recs.len() == 1 && recs[0].0 == 0x16, "hello must be a single handshake record");
        let msgs = messages(&recs[0].1)?;
        ensure!(msgs.len() == 1 && msgs[0].0 == 0x01, "expected a lone client hello");
        let (_, raw, h) = &msgs[0];

        ensure!(h.len() >= 35 && h[..2] == [3, 3], "client hello version");
        let client_random = h[2..34].to_vec();
        let sid = h[34] as usize;
        let mut i = 35 + sid;
        let ns = u16::from_be_bytes([h[i], h[i + 1]]) as usize;
        let suites: Vec<u16> = h[i + 2..i + 2 + ns].chunks(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
        ensure!(suites.contains(&0xc005), "client does not offer ECDH_ECDSA_WITH_AES_256_CBC_SHA: {suites:04x?}");
        i += 2 + ns;
        ensure!(h[i] == 0, "compression");
        i += 1;
        // The dialect understates the extension block length by 2.
        let ne = u16::from_be_bytes([h[i], h[i + 1]]) as usize;
        let exts = &h[i + 2..];
        ensure!(exts.len() == ne + 2, "extension length {ne} vs {}", exts.len());
        ensure!(exts == [0x00, 0x04, 0x00, 0x02, 0x00, 0x17, 0x00, 0x0b, 0x00, 0x02, 0x01, 0x00], "extensions");

        // A new hello starts a fresh session; keep only the test's knobs and log.
        let (reply, received, fault) = (std::mem::take(&mut st.reply), std::mem::take(&mut st.received), st.fault);
        *st = State { reply, received, fault, ..State::default() };
        st.hs_hash.update(raw);
        st.client_random = client_random;
        st.server_random = random(32);

        let mut sh = vec![3, 3];
        sh.extend(&st.server_random);
        sh.push(7);
        sh.extend(random(7)); // session id
        sh.extend([0xc0, 0x05, 0x00]); // suite, no compression
        let mut out = Vec::new();
        for m in [message(0x02, &sh), message(0x0d, &[0x01, 0x40, 0x00, 0x00]), message(0x0e, &[])] {
            st.hs_hash.update(&m);
            out.extend(m);
        }
        Ok(record(0x16, &out))
    }

    fn derive(&self, st: &mut State, client_pub: &PublicKey) {
        let pms = p256::ecdh::diffie_hellman(self.ecdh.to_nonzero_scalar(), client_pub.as_affine());
        let seed = [st.client_random.as_slice(), &st.server_random].concat();
        st.master_secret = tls::prf(pms.raw_secret_bytes(), &[b"master secret".as_slice(), &seed].concat(), 0x30);
        let kb = tls::prf(&st.master_secret, &[b"key expansion".as_slice(), &seed].concat(), 0x120);
        st.keys = Keys {
            mac_rx: kb[0x00..0x20].to_vec(),
            mac_tx: kb[0x20..0x40].to_vec(),
            enc_rx: kb[0x40..0x60].to_vec(),
            enc_tx: kb[0x60..0x80].to_vec(),
        };
    }

    fn client_flight(&self, st: &mut State, recs: &[(u8, Vec<u8>)]) -> Result<Vec<u8>> {
        ensure!(recs.len() == 3, "expected handshake + ChangeCipherSpec + Finished records, got {}", recs.len());
        ensure!(recs[0].0 == 0x16 && recs[2].0 == 0x16, "record types");
        ensure!(recs[1] == (0x14, vec![1]), "ChangeCipherSpec");

        let msgs = messages(&recs[0].1)?;
        let types: Vec<u8> = msgs.iter().map(|m| m.0).collect();
        ensure!(types == [0x0b, 0x10, 0x0f], "client flight {types:02x?}");

        // Certificate: both 24-bit lengths carry the bare cert length, then ac 16.
        let c = &msgs[0].2;
        let n = self.client_cert.len();
        ensure!(c[..3] == be24(n) && c[3..6] == be24(n) && c[6..8] == [0xac, 0x16], "certificate framing");
        ensure!(c[8..] == self.client_cert[..], "unexpected client certificate");
        st.hs_hash.update(&msgs[0].1);

        // ClientKeyExchange: a bare uncompressed point, no length byte.
        let k = &msgs[1].2;
        ensure!(k.len() == 65 && k[0] == 4, "client key exchange");
        let client_pub = PublicKey::from_sec1_bytes(k).context("client ECDH point")?;
        st.hs_hash.update(&msgs[1].1);
        self.derive(st, &client_pub);

        // CertificateVerify: DER ECDSA over the transcript hash so far.
        let sig = Signature::from_der(&msgs[2].2).context("certificate verify encoding")?;
        let hash = st.hs_hash.clone().finalize();
        self.client_key.verify_prehash(&hash, &sig).context("certificate verify signature")?;
        st.hs_hash.update(&msgs[2].1);

        // Finished, encrypted. Like python-validity, neither side hashes the client Finished.
        let fin = Self::unseal(st, 0x16, &recs[2].1)?;
        let msgs = messages(&fin)?;
        ensure!(msgs.len() == 1 && msgs[0].0 == 0x14, "expected client Finished");
        let hash = st.hs_hash.clone().finalize();
        let want = tls::prf(&st.master_secret, &[b"client finished".as_slice(), &hash].concat(), 0xc);
        ensure!(msgs[0].2 == want, "client Finished mismatch");

        let mut vd = tls::prf(&st.master_secret, &[b"server finished".as_slice(), &hash].concat(), 0xc);
        if Self::take_fault(st, Fault::WrongFinished) {
            vd[0] ^= 0x01;
        }
        let mut out = CCS.to_vec();
        out.extend(record(0x16, &Self::seal(st, 0x16, &message(0x14, &vd))));
        st.established = true;
        Ok(out)
    }

    fn app_data(&self, st: &mut State, out: &[u8]) -> Result<Vec<u8>> {
        let recs = records(out)?;
        ensure!(recs.len() == 1 && recs[0].0 == 0x17, "expected one app data record");
        let body = &recs[0].1;
        let p = Self::unseal(st, 0x17, body)?;
        ensure!(body.len() == 16 + (p.len() + 32) / 16 * 16 + 16, "record not padded to the next block");
        st.received.push(p);
        let reply = st.reply.clone();
        Ok(record(0x17, &Self::seal(st, 0x17, &reply)))
    }
}

impl Transport for Sensor {
    fn cmd(&self, out: &[u8]) -> Result<Vec<u8>> {
        let mut st = self.st.borrow_mut();
        let st = &mut *st;
        if let Some(rest) = out.strip_prefix(&[0x44, 0, 0, 0]) {
            let recs = records(rest)?;
            match recs.first() {
                Some((0x16, b)) if b.first() == Some(&0x01) => self.client_hello(st, &recs),
                Some(_) if !st.client_random.is_empty() && !st.established => self.client_flight(st, &recs),
                _ => bail!("unexpected handshake traffic"),
            }
        } else if st.established {
            self.app_data(st, out)
        } else {
            st.received.push(out.to_vec());
            Ok(st.reply.clone())
        }
    }
}

/// A paired client/sensor: the sensor knows the client's pairing key and
/// certificate, the client knows the sensor's ECDH key.
fn setup() -> (Tls, Sensor) {
    let pairing = SecretKey::random(&mut OsRng);
    let cert = tls::make_cert(&pairing.public_key());
    let sk = SigningKey::from(pairing);
    let sensor = Sensor::new(VerifyingKey::from(&sk), cert.clone());
    let mut t = Tls::new(&HwKey { product_name: "T480".into(), serial: "123456789".into() });
    t.priv_key = Some(sk);
    t.tls_cert = cert;
    t.ecdh_q = Some(sensor.ecdh.public_key());
    (t, sensor)
}

fn payload(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 7 + n) as u8).collect()
}

const SIZES: [usize; 16] = [0, 1, 2, 15, 16, 17, 31, 32, 33, 47, 48, 64, 255, 256, 1000, 4096];

#[test]
fn emulated_handshake_completes() {
    let (mut t, s) = setup();
    assert!(!t.is_secure());
    t.open(&s).unwrap();
    assert!(t.is_secure());
    assert!(s.st.borrow().established);
}

#[test]
fn emulated_app_data_round_trips() {
    let (mut t, s) = setup();
    t.open(&s).unwrap();
    for n in SIZES {
        // client -> sensor
        let sent = payload(n);
        s.set_reply(b"ok");
        assert_eq!(t.app(&s, &sent).unwrap(), b"ok");
        assert_eq!(s.last_received(), sent, "client -> sensor, {n} bytes");
        // sensor -> client
        let back = payload(n + 3).split_off(3);
        s.set_reply(&back);
        assert_eq!(t.app(&s, &[0x01]).unwrap(), back, "sensor -> client, {n} bytes");
        // both at once, through the dispatching cmd()
        s.set_reply(&sent);
        assert_eq!(t.cmd(&s, &sent).unwrap(), sent, "echo, {n} bytes");
    }
}

#[test]
fn emulated_cmd_is_raw_before_handshake() {
    let (mut t, s) = setup();
    s.set_reply(&[0, 0, 0x42]);
    assert_eq!(t.cmd(&s, &[0x01]).unwrap(), [0, 0, 0x42]);
    assert_eq!(s.last_received(), [0x01]);
    assert!(t.app(&s, &[0x01]).is_err(), "app data must not go out before the handshake");
}

#[test]
fn emulated_reopen_rekeys() {
    let (mut t, s) = setup();
    t.open(&s).unwrap();
    let first = s.st.borrow().keys.enc_tx.clone();
    t.open(&s).unwrap();
    assert_ne!(s.st.borrow().keys.enc_tx, first);
    s.set_reply(b"again");
    assert_eq!(t.app(&s, b"x").unwrap(), b"again");
}

#[test]
fn emulated_wrong_server_finished_is_rejected() {
    let (mut t, s) = setup();
    s.arm(Fault::WrongFinished);
    let err = t.open(&s).unwrap_err();
    assert!(format!("{err:#}").contains("final handshake check failed"), "{err:#}");
}

#[test]
fn emulated_tampered_server_mac_is_rejected() {
    // on the encrypted server Finished
    let (mut t, s) = setup();
    s.arm(Fault::ServerMac);
    let err = t.open(&s).unwrap_err();
    assert!(format!("{err:#}").contains("packet signature validation check failed"), "{err:#}");

    // on app data, for payloads with and without a partial last block
    let (mut t, s) = setup();
    t.open(&s).unwrap();
    for n in [0, 5, 16, 32] {
        s.set_reply(&payload(n));
        s.arm(Fault::ServerMac);
        let err = t.app(&s, b"x").unwrap_err();
        assert!(format!("{err:#}").contains("packet signature validation check failed"), "{n}: {err:#}");
        assert_eq!(t.app(&s, b"x").unwrap(), payload(n), "channel still usable after a rejected record");
    }
}

#[test]
fn emulated_sensor_rejects_tampered_client_record() {
    let (mut t, s) = setup();
    t.open(&s).unwrap();
    s.arm(Fault::ClientRecord);
    let err = t.app(&s, &payload(16)).unwrap_err();
    assert!(format!("{err:#}").contains("bad_record_mac"), "{err:#}");
}

#[test]
fn emulated_sensor_rejects_tampered_client_finished() {
    let (mut t, s) = setup();
    s.arm(Fault::ClientRecord); // the first sealed client record is the Finished
    let err = t.open(&s).unwrap_err();
    assert!(format!("{err:#}").contains("bad_record_mac"), "{err:#}");
    assert!(!t.is_secure());
    assert!(!s.st.borrow().established);
}

#[test]
fn emulated_sensor_rejects_foreign_pairing_key() {
    let (mut t, s) = setup();
    t.priv_key = Some(SigningKey::random(&mut OsRng));
    let err = t.open(&s).unwrap_err();
    assert!(format!("{err:#}").contains("certificate verify signature"), "{err:#}");
}

#[test]
fn emulated_handshake_needs_sensor_ecdh_key() {
    let (mut t, s) = setup();
    t.ecdh_q = Some(SecretKey::random(&mut OsRng).public_key());
    // Wrong sensor key: the sides derive different keys and the client Finished is garbage to the sensor.
    let err = t.open(&s).unwrap_err();
    assert!(format!("{err:#}").contains("bad_record_mac"), "{err:#}");
    t.ecdh_q = None;
    assert!(t.open(&s).is_err());
}
