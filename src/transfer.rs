use crate::fsutils::{protect_file, transfer_file};
use crate::objects::{Object, Tree, TreeError};
use crate::odb::{Odb, oid_to_path};
use indicatif::{ParallelProgressIterator, ProgressBar};
use rayon::prelude::*;
use std::fs;
use std::path::Path;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum TransferError {
    #[error(transparent)]
    TreeError(#[from] TreeError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub fn transfer_obj(root: &Path, from: &Path, oid: &str) -> std::io::Result<()> {
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
    fs::write(&to, contents)?;
    protect_file(&to);
    Ok(())
}

pub fn transfer_tree(odb: &Odb, wroot: &Path, tree: &Tree) -> Result<String, TransferError> {
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

    let (serialized, oid) = tree.digest()?;
    write_obj(&odb.path, &oid, &serialized)?;
    Ok(oid)
}

pub fn transfer(odb: &Odb, wroot: &Path, obj: &Object) -> Result<String, TransferError> {
    match obj {
        Object::HashFile(hf) => {
            transfer_obj(&odb.path, wroot, hf)?;
            Ok(hf.clone())
        }
        Object::Tree(t) => Ok(transfer_tree(odb, wroot, t)?),
    }
}
