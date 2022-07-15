use std::path::{Path, PathBuf};

pub struct Odb {
    #[allow(dead_code)]
    pub path: PathBuf,
}

pub fn oid_to_path(root: &Path, oid: &str) -> PathBuf {
    let mut to = root.join(&oid[..2]);
    to.push(&oid[2..]);
    to
}
