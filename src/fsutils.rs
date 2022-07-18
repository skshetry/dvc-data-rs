use crate::hash::md5;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

pub fn checksum_from_metadata(meta: &fs::Metadata) -> String {
    let m = meta.mtime() as f64 + (meta.mtime_nsec() as f64 / 1_000_000_000f64);
    let st = format!("([{}, {}, {}],)", meta.ino(), m, meta.size());
    let hash = md5(&mut st.as_bytes());
    u128::from_str_radix(&hash, 16).unwrap().to_string()
}

pub fn checksum(path: &PathBuf) -> String {
    // TODO: will be used to read from DVC's state db
    let meta = fs::metadata(path).unwrap();
    checksum_from_metadata(&meta)
}

pub fn size(path: PathBuf) -> u64 {
    let meta = fs::metadata(path).unwrap();
    meta.size()
}

pub fn transfer_file(from: &PathBuf, to: &PathBuf) {
    fs::create_dir_all(to.parent().unwrap()).unwrap();
    reflink::reflink_or_copy(from, to).unwrap();
}
