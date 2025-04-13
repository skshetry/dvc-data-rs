use crate::fsutils::transfer_file;
use crate::models::{DvcFile, Output};
use crate::objects::Tree;
use crate::odb::{Odb, oid_to_path};
use indicatif::{ParallelProgressIterator, ProgressBar};
use rayon::prelude::*;
use std::fs;
use std::io::Error as IOError;
use std::path::PathBuf;

fn checkout_file(
    from: &PathBuf,
    to: &PathBuf,
    cache_types: Option<&Vec<String>>,
) -> std::io::Result<()> {
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    #[cfg(windows)]
    use std::os::windows::fs::symlink_file as symlink;

    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }

    let _ = fs::remove_file(to);
    let link_types = if let Some(vec) = cache_types {
        vec
    } else {
        &vec!["copy".to_string()]
    };

    for link_type in link_types {
        match link_type.as_str() {
            "copy" | "reflink" => {
                if transfer_file(from, to).is_ok() {
                    return Ok(());
                }
            }
            "hardlink" => {
                if fs::hard_link(from, to).is_ok() {
                    return Ok(());
                }
            }
            "symlink" => {
                if symlink(from, to).is_ok() {
                    return Ok(());
                }
            }
            _ => panic!("Unknown cache type: {link_type:?}"),
        }
    }
    Err(IOError::new(
        std::io::ErrorKind::Other,
        "No cache type worked",
    ))
}

pub fn checkout_obj(
    odb: &Odb,
    oid: &str,
    to: &PathBuf,
    cache_types: &Option<Vec<String>>,
) -> std::io::Result<()> {
    let from = oid_to_path(&odb.path, oid);
    if oid.ends_with(".dir") {
        let tree = Tree::load_from(&from).unwrap();
        let pb = ProgressBar::new(tree.entries.len() as u64);

        fs::create_dir_all(to)?;
        tree.entries
            .par_iter()
            .progress_with(pb)
            .try_for_each(|entry| {
                let src = oid_to_path(&odb.path, &entry.oid);
                let dst = to.join(&entry.relpath);
                checkout_file(&src, &dst, cache_types.as_ref())?;
                std::io::Result::Ok(())
            })?;
        return Ok(());
    }
    checkout_file(&from, to, cache_types.as_ref())
}

pub fn checkout(
    odb: &Odb,
    dvcfile_path: &PathBuf,
    cache_types: &Option<Vec<String>>,
) -> std::io::Result<()> {
    let contents = &fs::read_to_string(dvcfile_path)?;
    let dvcfile_obj: DvcFile = serde_yaml::from_str(contents).unwrap();
    let Output { oid, path, .. } = dvcfile_obj.outs.0;
    checkout_obj(odb, &oid, &path, cache_types)
}
