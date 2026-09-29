//! Byte-for-byte checks against vectors produced by python-validity on a real
//! T480 (tests/golden/extract.py). The golden file holds this machine's pairing
//! blob, so it is not committed; tests that need it are skipped without it.

use serde_json::Value;

use crate::db::{Sid, parse_storage, parse_user};
use crate::sensor::{self, CaptureState, Mode};
use crate::tls::{self, HwKey, PairingMismatch, Tls};
use crate::{blobs, timeslot};

fn golden() -> Option<Value> {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/golden.json");
    let text = std::fs::read_to_string(p).ok()?;
    Some(serde_json::from_str(&text).unwrap())
}

macro_rules! golden_or_skip {
    () => {
        match golden() {
            Some(g) => g,
            None => {
                eprintln!("golden.json missing; skipped");
                return;
            }
        }
    };
}

fn h(v: &Value) -> Vec<u8> {
    hex::decode(v.as_str().unwrap()).unwrap()
}

fn coreboot_hwkey() -> HwKey {
    HwKey { product_name: "T480".into(), serial: "123456789".into() }
}

fn capture_state(g: &Value) -> CaptureState {
    CaptureState {
        factory_calibration_values: h(&g["factory_bits"]["3"])[4..].to_vec(),
        calib_data: h(&g["calib_data"]),
        lines_per_frame: g["lines_per_frame"].as_u64().unwrap() as usize,
    }
}

#[test]
fn prf_matches_python() {
    let g = golden_or_skip!();
    for v in g["prf"].as_array().unwrap() {
        let out = tls::prf(&h(&v["secret"]), &h(&v["seed"]), v["len"].as_u64().unwrap() as usize);
        assert_eq!(out, h(&v["out"]));
    }
}

#[test]
fn hs_key_matches_python() {
    let g = golden_or_skip!();
    assert_eq!(hex::encode(tls::hs_key().to_bytes()), g["hs_key"].as_str().unwrap());
}

#[test]
fn psk_matches_python() {
    let g = golden_or_skip!();
    let (enc, val) = coreboot_hwkey().psk();
    assert_eq!(enc.to_vec(), h(&g["psk_enc"]));
    assert_eq!(val.to_vec(), h(&g["psk_val"]));
}

#[test]
fn tls_flash_parses_and_reserializes() {
    let g = golden_or_skip!();
    let flash = h(&g["tls_flash"]);
    let mut t = Tls::new(&coreboot_hwkey());
    t.parse_tls_flash(&flash).unwrap();
    assert!(t.priv_key.is_some() && t.ecdh_q.is_some());
    assert_eq!(t.make_tls_flash(), flash, "cert store must round-trip byte-for-byte");
}

#[test]
fn tls_flash_rejects_other_identity() {
    let g = golden_or_skip!();
    let stock = HwKey { product_name: "20L6S4VC00".into(), serial: "PF0XXXXX".into() };
    let err = Tls::new(&stock).parse_tls_flash(&h(&g["tls_flash"])).unwrap_err();
    assert!(err.downcast_ref::<PairingMismatch>().is_some(), "{err:#}");
}

#[test]
fn private_key_wrap_round_trips() {
    let hw = coreboot_hwkey();
    let mut t = Tls::new(&hw);
    let sk = p256::SecretKey::random(&mut rand::rngs::OsRng);
    let blob = t.encrypt_key(&sk);
    t.handle_priv(&blob).unwrap();
    let got = t.priv_key.unwrap();
    assert_eq!(got.as_nonzero_scalar().to_bytes(), sk.to_nonzero_scalar().to_bytes());
    let err = Tls::new(&HwKey { product_name: "x".into(), serial: "y".into() }).handle_priv(&blob).unwrap_err();
    assert!(err.downcast_ref::<PairingMismatch>().is_some());
}

#[test]
fn pairing_cert_is_signed_by_hs_key() {
    use p256::ecdsa::signature::Verifier;
    let sk = p256::SecretKey::random(&mut rand::rngs::OsRng);
    let cert = tls::make_cert(&sk.public_key());
    assert_eq!(cert.len(), 444);
    let msg = &cert[..0xb8];
    let l = u32::from_le_bytes(cert[0xb8..0xbc].try_into().unwrap()) as usize;
    let sig = p256::ecdsa::Signature::from_der(&cert[0xbc..0xbc + l]).unwrap();
    let vk = p256::ecdsa::VerifyingKey::from(&p256::ecdsa::SigningKey::from(tls::hs_key()));
    vk.verify(msg, &sig).unwrap();
    let (x, y) = tls::point_to_le(&sk.public_key());
    assert_eq!(&msg[8..40], &x);
    assert_eq!(&msg[0x4c..0x6c], &y);
}

#[test]
fn bitpack_matches_python() {
    let g = golden_or_skip!();
    for v in g["bitpack"].as_array().unwrap() {
        let (u, m, out) = sensor::bitpack(&h(&v["in"]));
        assert_eq!(u as u64, v["u"].as_u64().unwrap());
        assert_eq!(m as u64, v["m"].as_u64().unwrap());
        assert_eq!(out, h(&v["out"]));
    }
}

#[test]
fn capture_commands_match_python() {
    let g = golden_or_skip!();
    let st = capture_state(&g);
    for (mode, name) in [(Mode::Calibrate, "CALIBRATE"), (Mode::Identify, "IDENTIFY"), (Mode::Enroll, "ENROLL")] {
        let ours = sensor::build_cmd_02(&st, mode).unwrap();
        let theirs = h(&g["cmd02"][name]);
        let first_diff = ours.iter().zip(&theirs).position(|(a, b)| a != b);
        assert!(ours == theirs, "{name}: len {} vs {}, first diff at {first_diff:?}", ours.len(), theirs.len());
    }
}

#[test]
fn capture_command_without_calibration() {
    // First-run calibration builds the program with empty calib data.
    let g = golden_or_skip!();
    let mut st = capture_state(&g);
    st.calib_data.clear();
    let cmd = sensor::build_cmd_02(&st, Mode::Calibrate).unwrap();
    assert_eq!(cmd[0], 2);
    assert_eq!(u16::from_le_bytes([cmd[3], cmd[4]]) as usize, 3 * st.lines_per_frame + 1);
}

#[test]
fn calibration_pipeline_matches_python() {
    let g = golden_or_skip!();
    let v = &g["calib_vec"];
    assert_eq!(v["frames"].as_u64(), Some(sensor::CALIBRATION_FRAMES as u64));
    assert_eq!(v["key_line"].as_u64(), Some(sensor::KEY_CALIBRATION_LINE as u64));
    let avg = sensor::average(&h(&v["raw"]), g["lines_per_frame"].as_u64().unwrap() as usize).unwrap();
    assert_eq!(avg, h(&v["avg"]));
    let mut calib = Vec::new();
    sensor::process_calibration_results(&mut calib, &avg);
    assert_eq!(calib, h(&v["c1"]));
    sensor::process_calibration_results(&mut calib, &avg);
    assert_eq!(calib, h(&v["c2"]));
}

#[test]
fn geometry_matches_python() {
    let g = golden_or_skip!();
    let t = &g["type_info"];
    assert_eq!(t["bytes_per_line"].as_u64(), Some(blobs::TYPE_199.bytes_per_line as u64));
    assert_eq!(t["line_width"].as_u64(), Some(blobs::TYPE_199.line_width as u64));
    assert_eq!(t["lines_per_calibration_data"].as_u64(), Some(blobs::TYPE_199.lines_per_calibration_data as u64));
    assert_eq!(t["repeat_multiplier"].as_u64(), Some(blobs::TYPE_199.repeat_multiplier as u64));
    assert_eq!(h(&t["calibration_blob"]), blobs::CALIBRATION_BLOB_199);
    assert_eq!(h(&g["hardcoded_prog"]), blobs::CAPTURE_PROG_199);
    for (k, v) in g["blobs"].as_object().unwrap() {
        let ours: &[u8] = match k.as_str() {
            "init_hardcoded" => blobs::INIT_HARDCODED,
            "init_hardcoded_clean_slate" => blobs::INIT_HARDCODED_CLEAN_SLATE,
            "reset_blob" => blobs::RESET_BLOB,
            "db_write_enable" => blobs::DB_WRITE_ENABLE,
            _ => continue,
        };
        assert_eq!(h(v), ours, "{k}");
    }
}

#[test]
fn chunks_round_trip() {
    let c = timeslot::split_chunks(blobs::CAPTURE_PROG_199).unwrap();
    assert_eq!(timeslot::merge_chunks(&c), blobs::CAPTURE_PROG_199);
    assert!(c.iter().any(|c| c.typ == 0x34) && c.iter().any(|c| c.typ == 0x2f));
}

#[test]
fn db_replies_parse_like_python() {
    let g = golden_or_skip!();
    let stg = parse_storage(&h(&g["db_storage_rsp"])).unwrap().unwrap();
    assert_eq!(stg.name, b"StgWindsor\0");
    for u in g["db_users"].as_array().unwrap() {
        let user = parse_user(&h(&u["raw"])).unwrap();
        assert_eq!(user.identity.to_string(), u["identity"].as_str().unwrap());
        let fingers = u["fingers"].as_array().unwrap();
        assert_eq!(user.fingers.len(), fingers.len());
        for (a, b) in user.fingers.iter().zip(fingers) {
            assert_eq!(a.dbid as u64, b["dbid"].as_u64().unwrap());
            assert_eq!(a.subtype as u64, b["subtype"].as_u64().unwrap());
            assert_eq!(a.value_size as u64, b["valueSize"].as_u64().unwrap());
        }
        assert!(stg.users.iter().any(|s| s.0 == user.dbid));
    }
}

#[test]
fn sid_mapping() {
    let s = Sid::for_uid(1000);
    assert_eq!(s.to_string(), "S-1-5-21-111111111-1111111111-1111111111-1000");
    assert_eq!(Sid::parse(&s.to_string()).unwrap(), s);
    assert_eq!(Sid::from_bytes(&s.to_bytes()).unwrap(), s);
    let id = s.identity_bytes();
    assert_eq!(id.len(), 0x4c);
    assert_eq!(&id[..4], &3u32.to_le_bytes());
}

#[test]
fn enroll_update_parsing() {
    // tag 1 (header), tag 0 (template), tag 3 (tid), each with the 0x38-byte magic prefix
    let mut body = Vec::new();
    for (tag, payload) in [(1u16, vec![0xaa; 4]), (0, vec![0xbb; 6]), (3, vec![0xcc; 2])] {
        let mut rec = tag.to_le_bytes().to_vec();
        rec.extend((payload.len() as u16).to_le_bytes());
        rec.resize(0x38, 0x11);
        rec.extend(&payload);
        body.extend(rec);
    }
    let mut res = (body.len() as u16).to_le_bytes().to_vec();
    res.extend(&body);
    let (hdr, tmpl, tid) = sensor::parse_enroll_update(&res).unwrap();
    assert_eq!(hdr.unwrap(), vec![0xaa; 4]);
    assert_eq!(tid.unwrap(), vec![0xcc; 2]);
    let t = tmpl.unwrap();
    assert_eq!(t.len(), 0x38 + 6);
    assert_eq!(&t[..2], &0u16.to_le_bytes());
}

#[test]
fn finger_names() {
    assert_eq!(blobs::finger_id("right-index-finger"), Some(2));
    assert_eq!(blobs::finger_name(2), "right-index-finger");
    assert_eq!(blobs::finger_name(0x77), "Unknown");
}
