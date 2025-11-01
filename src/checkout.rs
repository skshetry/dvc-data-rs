use crate::fsutils::transfer_file;
use crate::models::{DvcFile, Output};
use crate::objects::{Tree, TreeError};
use crate::odb::{Odb, oid_to_path};
use camino::Utf8PathBuf;
use indicatif::{ParallelProgressIterator, ProgressBar};
use rayon::prelude::*;
use std::fs;
use std::io::Error as IOError;
use std::path::Path;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum CheckoutError {
    #[error(transparent)]
    TreeError(#[from] TreeError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    SerdeYaml(#[from] serde_yaml::Error),
}

fn checkout_file(from: &Path, to: &Path, cache_types: Option<&Vec<String>>) -> std::io::Result<()> {
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
        &vec!["copy".to_owned()]
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
    Err(IOError::other("No cache type worked"))
}

pub fn checkout_obj(
    odb: &Odb,
    oid: &str,
    to: &Utf8PathBuf,
    cache_types: &Option<Vec<String>>,
) -> Result<(), CheckoutError> {
    let from = oid_to_path(&odb.path, oid);
    if oid.ends_with(".dir") {
        let tree = Tree::load_from(&from)?;
        let pb = ProgressBar::new(tree.entries.len() as u64);

        fs::create_dir_all(to)?;
        tree.entries
            .par_iter()
            .progress_with(pb)
            .try_for_each(|entry| {
                let src = oid_to_path(&odb.path, &entry.oid);
                let dst = to.join(&entry.relpath);
                checkout_file(&src, &dst.into_std_path_buf(), cache_types.as_ref())
            })?;
        return Ok(());
    }
    Ok(checkout_file(
        &from,
        &to.clone().into_std_path_buf(),
        cache_types.as_ref(),
    )?)
}

pub fn checkout(
    odb: &Odb,
    dvcfile_path: &Utf8PathBuf,
    cache_types: &Option<Vec<String>>,
) -> Result<(), CheckoutError> {
    let contents = &fs::read_to_string(dvcfile_path)?;
    let dvcfile_obj: DvcFile = serde_yaml::from_str(contents)?;
    let Output { oid, path, .. } = dvcfile_obj.outs.0;
    checkout_obj(odb, &oid, &path, cache_types)
}
