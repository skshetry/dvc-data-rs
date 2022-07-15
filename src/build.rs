use crate::hash::file_md5;
use crate::objects::{Object, Tree};
use crate::odb::Odb;
use jwalk::{Parallelism, WalkDir};
use rayon::prelude::*;
use std::path::{Path, PathBuf};

pub fn build(_odb: &Odb, root: &Path, jobs: usize) -> Object {
    if root.is_file() {
        return Object::HashFile(file_md5(root.to_path_buf()));
    }

    let mut result: Vec<(PathBuf, String)> = WalkDir::new(root)
        .parallelism(Parallelism::RayonNewPool(jobs))
        .into_iter()
        .par_bridge()
        .filter_map(|dir_entry_res| {
            let dentry = dir_entry_res.ok()?;
            if !dentry.file_type().is_file() {
                return None;
            }
            Some(dentry.path())
        })
        .map(|file| {
            let relpath = file.strip_prefix(root).unwrap().to_path_buf();
            let oid = file_md5(file);
            (relpath, oid)
        })
        .collect();

    result.sort_unstable(); // sort keys
    Object::Tree(Tree { entries: result })
}
