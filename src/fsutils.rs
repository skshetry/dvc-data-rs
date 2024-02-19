use crate::hash::md5;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;

pub fn checksum_from_metadata(meta: &fs::Metadata) -> String {
    #[allow(clippy::cast_precision_loss)]
    let m = meta.mtime() as f64 + (meta.mtime_nsec() as f64 / 1_000_000_000f64);
    let st = "([".to_owned()
        + &meta.ino().to_string()
        + ", "
        + &m.to_string()
        + ", "
        + &meta.size().to_string()
        + "],)";
    let hash = md5(&mut st.as_bytes());
    u128::from_str_radix(&hash, 16).unwrap().to_string()
}

pub fn checksum(path: &PathBuf) -> String {
    let meta = fs::metadata(path).unwrap();
    checksum_from_metadata(&meta)
}

pub fn size(path: PathBuf) -> u64 {
    let meta = fs::metadata(path).unwrap();
    meta.size()
}

pub fn transfer_file(from: &PathBuf, to: &PathBuf) {
    fs::create_dir_all(to.parent().unwrap()).unwrap();
    reflink_copy::reflink_or_copy(from, to)
        .unwrap_or_else(|_| panic!("transfer failed: {from:?} {to:?}"));
}

pub fn protect_file(path: &PathBuf) {
    let permission = fs::Permissions::from_mode(0o444);
    fs::set_permissions(path, permission).unwrap_or_default();
}
