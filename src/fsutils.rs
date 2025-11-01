use crate::hash::md5;
use std::fs;
use std::path::Path;

pub fn compute_checksum(ut: f64, ino: u128, size: u64) -> String {
    let st = "([".to_owned()
        + &ino.to_string()
        + ", "
        + &ut.to_string()
        + ", "
        + &size.to_string()
        + "],)";
    let hash = md5(&mut st.as_bytes());
    u128::from_str_radix(&hash, 16)
        .expect("expected a valid u128 from md5 hash")
        .to_string()
}

#[cfg(unix)]
pub fn size_from_meta(meta: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.size()
}

#[cfg(windows)]
pub fn size_from_meta(meta: &fs::Metadata) -> u64 {
    use std::os::windows::fs::MetadataExt;
    meta.file_size()
}

pub fn transfer_file(from: &Path, to: &Path) -> std::io::Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    reflink_copy::reflink_or_copy(from, to).map(|_| ())?;
    Ok(())
}

pub fn protect_file(path: &Path) {
    if let Ok(m) = path.metadata() {
        m.permissions().set_readonly(true);
    }
}
