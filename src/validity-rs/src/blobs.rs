//! Opaque command blobs and tables for the 06cb:009a, taken verbatim from
//! python-validity (MIT, (c) 2020 uunicorn), which extracted them from
//! Synaptics' Windows driver.

pub static INIT_HARDCODED: &[u8] = include_bytes!("../blobs/init_hardcoded.bin");
pub static INIT_HARDCODED_CLEAN_SLATE: &[u8] = include_bytes!("../blobs/init_hardcoded_clean_slate.bin");
pub static RESET_BLOB: &[u8] = include_bytes!("../blobs/reset_blob.bin");
pub static DB_WRITE_ENABLE: &[u8] = include_bytes!("../blobs/db_write_enable.bin");
/// Capture program for ROM 6.x, sensor type 0x199 (SensorCaptureProg 6/0x199/0x18/0x19).
pub static CAPTURE_PROG_199: &[u8] = include_bytes!("../blobs/capture_prog_199.bin");
pub static CALIBRATION_BLOB_199: &[u8] = include_bytes!("../blobs/calibration_blob_199.bin");
pub static CRT_HARDCODED: &[u8] = include_bytes!("../blobs/crt_hardcoded.bin");
pub static PARTITION_SIGNATURE: &[u8] = include_bytes!("../blobs/partition_signature.bin");

/// Sensor type 0x199 ("57K0") geometry.
pub struct TypeInfo {
    pub bytes_per_line: usize,
    pub repeat_multiplier: u8,
    pub lines_per_calibration_data: usize,
    pub line_width: usize,
}

pub const TYPE_199: TypeInfo = TypeInfo {
    bytes_per_line: 0x78,
    repeat_multiplier: 2,
    lines_per_calibration_data: 112,
    line_width: 112,
};

pub const SENSOR_MAJOR: u16 = 0x190;

/// (version, mask, name) rows of python-validity's dev_info_table with type 0x199.
pub const DEV_199: &[(u16, u16, &str)] = &[
    (0x4a, 0xff, "57K0 FM-3367-001"),
    (0x57, 0xff, "57K0 FM- 155-002"),
    (0x59, 0xff, "57K0 FM-3367-002"),
    (0x5a, 0xff, "57K0 FM-3367-003"),
    (0x5b, 0xff, "57K0 FM-3367-004"),
    (0x60, 0xff, "57K0 FM-3380-002"),
    (0x61, 0xff, "57K0 FM-3380-003"),
    (0x62, 0xff, "57K0 FM-3380-004"),
    (0x68, 0xff, "57K0 FM-3367-005"),
    (0x69, 0xff, "57K0 FM-3367-006"),
    (0x6a, 0xff, "57K0 FM-3380-001"),
    (0x76, 0xff, "57K0 FM-155-005/006"),
    (0x7e, 0xff, "57K0 FM-155-007"),
    (0x82, 0xff, "57K0 FM-155-102"),
    (0x89, 0xff, "57K0 FM-155-008"),
    (0x8e, 0xff, "57K0 FM-155-103"),
];

/// GD25Q80C, the only flash IC python-validity knows: (jedec0, jedec1, size, sector size, erase cmd).
pub const FLASH_IC: (u16, u16, u32, u32, u8) = (0xc8, 0x40, 1 << 20, 0x1000, 0x20);

/// (id, type, access level, offset, size) — the layout written when pairing.
pub const FLASH_LAYOUT: &[(u8, u8, u16, u32, u32)] = &[
    (1, 4, 7, 0x0000_1000, 0x0000_1000), // cert store
    (2, 1, 2, 0x0000_2000, 0x0003_e000), // xpfwext
    (5, 5, 3, 0x0004_0000, 0x0000_8000), // ???
    (6, 6, 3, 0x0004_8000, 0x0000_8000), // calibration data
    (4, 3, 5, 0x0005_0000, 0x0008_0000), // template database
];

pub const FWEXT_NAME: &str = "6_07f_lenovo_mis_qm.xpfwext";

pub const GLOW_START_SCAN: &str = "3920bf0200ffff0000019900200000000099990000000000000000000000000020000000000000000000000000ffff000000990020000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
pub const GLOW_END_SCAN: &str = "39f4010000f401000001ff002000000000ffff0000000000000000000000000020000000000000000000000000f401000000ff0020000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

pub const FINGER_NAMES: &[(u16, &str)] = &[
    (1, "right-thumb"),
    (2, "right-index-finger"),
    (3, "right-middle-finger"),
    (4, "right-ring-finger"),
    (5, "right-little-finger"),
    (6, "left-thumb"),
    (7, "left-index-finger"),
    (8, "left-middle-finger"),
    (9, "left-ring-finger"),
    (10, "left-little-finger"),
];

pub fn finger_name(subtype: u16) -> &'static str {
    FINGER_NAMES.iter().find(|f| f.0 == subtype).map(|f| f.1).unwrap_or("Unknown")
}

pub fn finger_id(name: &str) -> Option<u16> {
    FINGER_NAMES.iter().find(|f| f.1 == name).map(|f| f.0)
}
