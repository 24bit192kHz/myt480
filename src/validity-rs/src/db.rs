//! On-chip template database: StgWindsor storage → users (Windows SIDs) → fingers.

use anyhow::{Context, Result, bail, ensure};

use crate::device::Device;
use crate::util::{Reader, Writer, check_status, status};

const NOT_FOUND: u16 = 0x04b3;
const STORAGE_NAME: &str = "StgWindsor";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sid {
    pub revision: u8,
    pub auth: u64,
    pub subauth: Vec<u32>,
}

impl Sid {
    pub fn parse(s: &str) -> Result<Sid> {
        let mut parts = s.split('-');
        ensure!(parts.next() == Some("S"), "SID must start with S-");
        let nums: Vec<u64> = parts.map(|p| p.parse::<u64>()).collect::<Result<_, _>>().context("SID component")?;
        ensure!(nums.len() >= 2, "SID too short");
        Ok(Sid {
            revision: nums[0] as u8,
            auth: nums[1],
            subauth: nums[2..].iter().map(|&n| n as u32).collect(),
        })
    }

    /// python-validity's default mapping for a local uid.
    pub fn for_uid(uid: u32) -> Sid {
        Sid { revision: 1, auth: 5, subauth: vec![21, 111111111, 1111111111, 1111111111, uid] }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = vec![self.revision, self.subauth.len() as u8];
        b.extend(((self.auth >> 32) as u16).to_be_bytes());
        b.extend((self.auth as u32).to_be_bytes());
        for s in &self.subauth {
            b.extend(s.to_le_bytes());
        }
        b
    }

    pub fn from_bytes(b: &[u8]) -> Result<Sid> {
        ensure!(b.len() >= 8, "short SID");
        let auth = b[2..8].iter().fold(0u64, |a, &x| a << 8 | x as u64);
        let n = b[1] as usize;
        ensure!(b.len() >= 8 + 4 * n, "short SID sub-authorities");
        let subauth = (0..n).map(|i| u32::from_le_bytes(b[8 + 4 * i..12 + 4 * i].try_into().unwrap())).collect();
        Ok(Sid { revision: b[0], auth, subauth })
    }

    /// Identity record key: type 3 (SID), padded to the 0x4c-byte Windows union.
    pub fn identity_bytes(&self) -> Vec<u8> {
        let sid = self.to_bytes();
        let mut b = Writer::new().u32(3).u32(sid.len() as u32).bytes(&sid).done();
        if b.len() < 0x4c {
            b.resize(0x4c, 0);
        }
        b
    }
}

impl std::fmt::Display for Sid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "S-{}-{}", self.revision, self.auth)?;
        for s in &self.subauth {
            write!(f, "-{s}")?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Storage {
    pub dbid: u16,
    #[cfg_attr(not(test), allow(dead_code))]
    pub name: Vec<u8>,
    pub users: Vec<(u16, u16)>, // (dbid, value size)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finger {
    pub dbid: u16,
    pub subtype: u16,
    pub storage: u16,
    pub value_size: u16,
}

#[derive(Debug)]
pub struct User {
    pub dbid: u16,
    pub identity: Sid,
    pub fingers: Vec<Finger>,
}

#[derive(Debug)]
pub struct DbInfo {
    pub total: u32,
    pub used: u32,
    pub free: u32,
    pub records: u16,
}

pub fn parse_storage(rsp: &[u8]) -> Result<Option<Storage>> {
    ensure!(rsp.len() >= 2, "short storage reply");
    if status(rsp) == NOT_FOUND {
        return Ok(None);
    }
    check_status(rsp)?;
    let mut r = Reader::new(&rsp[2..]);
    let (dbid, usercnt, namesz, _) = (r.u16()?, r.u16()?, r.u16()?, r.u16()?);
    let mut users = Vec::new();
    for _ in 0..usercnt {
        users.push((r.u16()?, r.u16()?));
    }
    let name = r.take(namesz as usize)?.to_vec();
    ensure!(r.is_empty(), "junk at the end of the storage info reply");
    Ok(Some(Storage { dbid, name, users }))
}

pub fn parse_user(rsp: &[u8]) -> Result<User> {
    check_status(rsp)?;
    let mut r = Reader::new(&rsp[2..]);
    let (dbid, fingercnt, _, idsz) = (r.u16()?, r.u16()?, r.u16()?, r.u16()?);
    let mut fingers = Vec::new();
    for _ in 0..fingercnt {
        fingers.push(Finger { dbid: r.u16()?, subtype: r.u16()?, storage: r.u16()?, value_size: r.u16()? });
    }
    let id = r.take(idsz as usize)?;
    ensure!(r.is_empty(), "junk at the end of the user info reply");
    let mut ir = Reader::new(id);
    let t = ir.u32()?;
    if t != 3 {
        bail!("unsupported identity type {t}");
    }
    let l = ir.u32()? as usize;
    let identity = Sid::from_bytes(ir.take(l)?)?;
    Ok(User { dbid, identity, fingers })
}

impl Device {
    pub fn get_user_storage(&mut self, name: &str) -> Result<Option<Storage>> {
        let mut n = name.as_bytes().to_vec();
        if !n.is_empty() {
            n.push(0);
        }
        let cmd = Writer::new().u8(0x4b).u16(0).u16(n.len() as u16).bytes(&n).done();
        parse_storage(&self.cmd(&cmd)?)
    }

    pub fn storage(&mut self) -> Result<Storage> {
        self.get_user_storage(STORAGE_NAME)?.context("StgWindsor storage missing")
    }

    pub fn get_user(&mut self, dbid: u16) -> Result<User> {
        parse_user(&self.cmd(&Writer::new().u8(0x4a).u16(dbid).u16(0).u16(0).done())?)
    }

    pub fn lookup_user(&mut self, sid: &Sid) -> Result<Option<User>> {
        let stg = self.storage()?;
        let data = sid.identity_bytes();
        let cmd = Writer::new().u8(0x4a).u16(0).u16(stg.dbid).u16(data.len() as u16).bytes(&data).done();
        let rsp = self.cmd(&cmd)?;
        ensure!(rsp.len() >= 2, "short user lookup reply");
        if status(&rsp) == NOT_FOUND {
            return Ok(None);
        }
        parse_user(&rsp).map(Some)
    }

    pub fn users(&mut self) -> Result<Vec<User>> {
        let stg = self.storage()?;
        stg.users.iter().map(|&(id, _)| self.get_user(id)).collect()
    }

    pub fn del_record(&mut self, dbid: u16) -> Result<()> {
        check_status(&self.cmd(&Writer::new().u8(0x48).u16(dbid).done())?)
    }

    pub fn db_info(&mut self) -> Result<DbInfo> {
        let rsp = self.cmd(&[0x45])?;
        check_status(&rsp)?;
        let mut r = Reader::new(&rsp[2..]);
        let (_u1, _u0, total, used, free) = (r.u32()?, r.u32()?, r.u32()?, r.u32()?, r.u32()?);
        let (records, nroots) = (r.u16()?, r.u16()?);
        let _roots: Vec<u16> = (0..nroots).map(|_| r.u16()).collect::<Result<_>>()?;
        Ok(DbInfo { total, used, free, records })
    }

    pub fn new_record(&mut self, parent: u16, typ: u16, storage: u16, data: &[u8]) -> Result<u16> {
        let info = self.db_info()?;
        ensure!(info.free as usize > data.len() + 0x100, "template database full ({} bytes free)", info.free);
        let cmd = Writer::new().u8(0x47).u16(parent).u16(typ).u16(storage).u16(data.len() as u16).bytes(data).done();
        self.with_write(|d| {
            let rsp = d.cmd(&cmd)?;
            check_status(&rsp)?;
            ensure!(rsp.len() >= 4, "short new-record reply");
            Ok(u16::from_le_bytes([rsp[2], rsp[3]]))
        })
    }

    pub fn init_db(&mut self) -> Result<()> {
        if self.get_user_storage(STORAGE_NAME)?.is_none() {
            log::info!("creating the StgWindsor user storage");
            self.new_record(1, 4, 3, b"StgWindsor\0")?;
        }
        Ok(())
    }

    pub fn new_user(&mut self, sid: &Sid) -> Result<u16> {
        let stg = self.storage()?;
        self.new_record(stg.dbid, 5, stg.dbid, &sid.identity_bytes())
    }

    pub fn new_finger(&mut self, user: u16, template: &[u8]) -> Result<u16> {
        let stg = self.storage()?;
        // Requested as type 0xb; the write-enable blob turns it into 0x6.
        self.new_record(user, 0xb, stg.dbid, template)
    }
}
