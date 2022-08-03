use crate::fsutils::{protect_file, transfer_file};
use crate::objects::{Object, Tree};
use crate::odb::{oid_to_path, Odb};
use indicatif::{ParallelProgressIterator, ProgressBar};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

pub fn transfer_obj(root: &Path, from: &PathBuf, oid: &str) {
    let to = oid_to_path(root, oid);
    if to.exists() {
        return;
    }
    transfer_file(from, &to);
    protect_file(&to);
}

pub fn write_obj(root: &Path, oid: &str, contents: &String) {
    let to = oid_to_path(root, oid);
    if to.exists() {
        return;
    }
    fs::create_dir_all(to.parent().unwrap()).unwrap();
    fs::write(to.clone(), contents).unwrap();
    protect_file(&to);
}

pub fn transfer_tree(odb: &Odb, wroot: &Path, tree: &Tree) -> String {
    let pb = ProgressBar::new(tree.entries.len() as u64);
    fs::create_dir_all(&odb.path).unwrap();
    tree.entries
        .par_iter()
        .progress_with(pb)
        .for_each(|(path, oid)| {
            let file = wroot.join(path);
            transfer_obj(&odb.path, &file, oid);
        });

    let (serialized, oid) = tree.digest();
    write_obj(&odb.path, &oid, &serialized);
    oid
}

pub fn transfer(odb: &Odb, wroot: &PathBuf, obj: &Object) -> String {
    match obj {
        Object::HashFile(hf) => {
            transfer_obj(&odb.path, wroot, hf);
            hf.to_string()
        }
        Object::Tree(t) => transfer_tree(odb, wroot, t),
    }
}
