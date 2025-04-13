use crate::fsutils::{protect_file, transfer_file};
use crate::objects::{Object, Tree};
use crate::odb::{Odb, oid_to_path};
use indicatif::{ParallelProgressIterator, ProgressBar};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

pub fn transfer_obj(root: &Path, from: &PathBuf, oid: &str) -> std::io::Result<()> {
    let to = oid_to_path(root, oid);
    if to.exists() {
        return Ok(());
    }
    transfer_file(from, &to)?;
    protect_file(&to);
    Ok(())
}

pub fn write_obj(root: &Path, oid: &str, contents: &str) -> std::io::Result<()> {
    let to = oid_to_path(root, oid);
    if to.exists() {
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(to.clone(), contents)?;
    protect_file(&to);
    Ok(())
}

pub fn transfer_tree(odb: &Odb, wroot: &Path, tree: &Tree) -> std::io::Result<String> {
    let pb = ProgressBar::new(tree.entries.len() as u64);
    fs::create_dir_all(&odb.path)?;
    tree.entries
        .par_iter()
        .progress_with(pb)
        .try_for_each(|entry| {
            let file = wroot.join(&entry.relpath);
            transfer_obj(&odb.path, &file, &entry.oid)?;
            std::io::Result::Ok(())
        })?;

    let (serialized, oid) = tree.digest().unwrap();
    write_obj(&odb.path, &oid, &serialized)?;
    Ok(oid)
}

pub fn transfer(odb: &Odb, wroot: &PathBuf, obj: &Object) -> std::io::Result<String> {
    match obj {
        Object::HashFile(hf) => {
            transfer_obj(&odb.path, wroot, hf)?;
            Ok(hf.to_string())
        }
        Object::Tree(t) => Ok(transfer_tree(odb, wroot, t)?),
    }
}
