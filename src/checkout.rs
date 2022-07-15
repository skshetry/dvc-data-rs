use crate::fsutils::transfer_file;
use crate::models::{DvcFile, Output};
use crate::objects::Tree;
use crate::odb::{oid_to_path, Odb};
use indicatif::{ParallelProgressIterator, ProgressBar};
use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;

pub fn checkout_obj(odb: &Odb, oid: &str, to: &PathBuf) {
    let from = oid_to_path(&odb.path, oid);
    if oid.ends_with(".dir") {
        let tree = Tree::load_from(&from);
        let pb = ProgressBar::new(tree.entries.len() as u64);

        fs::create_dir_all(to).unwrap();
        return tree
            .entries
            .par_iter()
            .progress_with(pb)
            .for_each(|(path, oid)| {
                let src = oid_to_path(&odb.path, oid);
                let dst = to.join(path);
                transfer_file(&src, &dst);
            });
    }
    transfer_file(&from, to)
}

pub fn checkout(odb: &Odb, dvcfile_path: &PathBuf) {
    let contents = &fs::read_to_string(dvcfile_path).unwrap();
    let dvcfile_obj: DvcFile = serde_yaml::from_str(contents).unwrap();
    let Output { oid, path } = dvcfile_obj.outs.0;
    checkout_obj(odb, &oid, &path)
}
