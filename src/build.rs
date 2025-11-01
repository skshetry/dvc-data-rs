use crate::fsutils::{compute_checksum, size_from_meta};
use crate::hash::file_md5;
use crate::objects::{Object, Oid, Tree, TreeEntry};
use crate::odb::Odb;
use crate::state::{State, StateError, StateHash, StateValue};
use crate::timeutils::unix_time;
use camino::{FromPathBufError, FromPathError, Utf8Path, Utf8PathBuf};
use ignore;
use ignore::gitignore::Gitignore;
use jwalk::{Parallelism, WalkDir};
use log::debug;
use rayon::prelude::*;
use std::fs;
use std::path::StripPrefixError;
use std::time::Instant;
struct FileInfo {
    checksum: String,
    path: Utf8PathBuf,
    size: u64,
}

use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum BuildError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    WalkError(#[from] jwalk::Error),
    #[error(transparent)]
    StateError(#[from] StateError),
    #[error(transparent)]
    FromPathBufError(#[from] FromPathBufError),
    #[error(transparent)]
    FromPathError(#[from] FromPathError),
    #[error(transparent)]
    StripPrefixError(#[from] StripPrefixError),
}

#[inline]
fn log_durations<F, R>(label: &str, f: F) -> R
where
    F: FnOnce() -> R,
{
    let start = Instant::now();
    let result = f();
    let duration = start.elapsed();
    debug!("{duration:?} in {label}");
    result
}

impl FileInfo {
    fn from_metadata(path: &Utf8Path, meta: &fs::Metadata) -> Result<Self, std::io::Error> {
        let ut = unix_time(meta.modified()?);
        let ino = {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                u128::from(meta.ino())
            }

            #[cfg(windows)]
            {
                use file_id::{FileId, get_file_id};
                match get_file_id(path)? {
                    FileId::LowRes {
                        volume_serial_number: _,
                        file_index,
                    } => u128::from(file_index),
                    FileId::HighRes {
                        volume_serial_number: _,
                        file_id: id,
                    } => id,
                    FileId::Inode {
                        device_id: _,
                        inode_number,
                    } => u128::from(inode_number),
                }
            }
        };
        let size = size_from_meta(meta);
        let checksum = compute_checksum(ut, ino, size);
        Ok(Self {
            checksum,
            size,
            path: path.to_path_buf(),
        })
    }
}

fn collect_files(
    root: &Utf8Path,
    ignore: &Gitignore,
    jobs: usize,
) -> Result<Vec<FileInfo>, BuildError> {
    WalkDir::new(root)
        .follow_links(true)
        .skip_hidden(false)
        .parallelism(Parallelism::RayonNewPool(jobs))
        .process_read_dir(|_, _, (), children| {
            for dir_entry in children.iter_mut().flatten() {
                if dir_entry.file_name() == ".dvc"
                    || dir_entry.file_name() == ".git"
                    || dir_entry.file_name() == ".hg"
                {
                    dir_entry.read_children_path = None;
                }
            }
        })
        .into_iter()
        .par_bridge()
        .map(|dir_entry_res| {
            let dentry = dir_entry_res?;
            if !dentry.file_type().is_file() {
                return Ok(None);
            }
            let path = &Utf8PathBuf::try_from(dentry.path())?;
            if ignore.matched_path_or_any_parents(path, false).is_ignore() {
                return Ok(None);
            }
            match dentry.metadata() {
                Err(e) => Err(BuildError::WalkError(e)),
                Ok(meta) => FileInfo::from_metadata(path, &meta)
                    .map(Some)
                    .map_err(BuildError::Io),
            }
        })
        .filter_map(std::result::Result::transpose)
        .collect()
}

#[derive(Default)]
struct HashResults {
    new: Vec<FileInfo>,
    cached: Vec<(FileInfo, Oid)>,
}

fn get_hashes(file_infos: Vec<FileInfo>, state: Option<&State>) -> Result<HashResults, BuildError> {
    let mut new = Vec::new();
    let mut cached = Vec::new();
    match state {
        Some(s) if !s.is_empty()? => {
            let keys: Vec<String> = file_infos
                .iter()
                .map(|file_info| file_info.path.as_str().to_string())
                .collect();
            let mut m = s.get_many(keys.iter(), None)?;

            for (file_info, key) in file_infos.into_iter().zip(keys.into_iter()) {
                match m.remove(&key) {
                    Some(v) if v.checksum == file_info.checksum => {
                        cached.push((file_info, v.hash_info.oid));
                    }
                    _ => new.push(file_info),
                }
            }
            Ok(HashResults { new, cached })
        }
        _ => Ok(HashResults {
            new: file_infos,
            cached,
        }),
    }
}

fn set_hashes<'a>(
    entries: impl Iterator<Item = &'a (FileInfo, Oid)>,
    state: Option<&State>,
) -> Result<(), StateError> {
    if let Some(s) = state {
        let state_hashes = entries.map(|(file_info, oid)| {
            let file = &file_info.path;
            let checksum = &file_info.checksum;
            (
                file.as_str().to_string(),
                StateValue {
                    checksum: checksum.clone(),
                    hash_info: StateHash { oid: oid.clone() },
                    size: file_info.size,
                },
            )
        });
        s.set_many(state_hashes)?;
    }
    Ok(())
}

fn hash_files(file_infos: Vec<FileInfo>) -> std::io::Result<Vec<(FileInfo, Oid)>> {
    if file_infos.is_empty() {
        Ok(Vec::new())
    } else {
        file_infos
            .into_par_iter()
            .map(|file_info| {
                let oid = file_md5(&file_info.path)?;
                Ok((file_info, oid))
            })
            .collect()
    }
}

fn get_or_hash_files(
    files: Vec<FileInfo>,
    state: Option<&State>,
) -> Result<Vec<(FileInfo, Oid)>, BuildError> {
    let HashResults { new, mut cached } = log_durations("checking cache for hashed files", || {
        get_hashes(files, state)
    })?;
    let new_entries = log_durations("hashing files", || hash_files(new))?;
    log_durations("saving hashes", || set_hashes(new_entries.iter(), state))?;
    cached.extend(new_entries);
    Ok(cached)
}

fn build_file(root: &Utf8Path, state: Option<&State>) -> Result<(Object, u64), BuildError> {
    let file_info = FileInfo::from_metadata(root, &fs::metadata(root)?)?;
    let key = root.as_str();
    let state_value: Option<StateValue> = match state {
        Some(s) => s.get(key)?.filter(|st| st.checksum == file_info.checksum),
        _ => None,
    };
    let oid = if let Some(st) = state_value {
        st.hash_info.oid
    } else {
        let oid = file_md5(&root)?;
        if let Some(s) = state {
            let sv = StateValue {
                checksum: file_info.checksum.clone(),
                hash_info: StateHash { oid: oid.clone() },
                size: file_info.size,
            };
            s.set(key, &sv)?;
        }
        oid
    };
    Ok((Object::HashFile(oid), file_info.size))
}

fn build_tree_from_entries(
    root: &Utf8Path,
    file_infos_with_oids: impl Iterator<Item = (FileInfo, Oid)>,
) -> Result<Tree, BuildError> {
    let mut entries = file_infos_with_oids
        .map(|(file_info, oid)| {
            let relpath = file_info.path.strip_prefix(root)?.to_path_buf();
            Ok(TreeEntry { relpath, oid })
        })
        .collect::<Result<Vec<_>, BuildError>>()?;
    entries.par_sort_unstable(); // sort keys
    Ok(Tree { entries })
}

fn build_tree(
    root: &Utf8Path,
    state: Option<&State>,
    ignore: &Gitignore,
    jobs: usize,
) -> Result<(Object, u64), BuildError> {
    let result = log_durations("collecting files", || collect_files(root, ignore, jobs));
    match result {
        Ok(files) => {
            let size = files.iter().map(|fi| fi.size).sum();
            let all_entries = get_or_hash_files(files, state)?;
            let tree = log_durations("building tree", || {
                build_tree_from_entries(root, all_entries.into_iter())
            })?;
            Ok((Object::Tree(tree), size))
        }
        Err(e) => Err(e),
    }
}

pub fn build(
    _odb: &Odb,
    root: &Utf8Path,
    state: Option<&State>,
    ignore: &Gitignore,
    jobs: usize,
) -> Result<(Object, u64), BuildError> {
    let root = camino::absolute_utf8(root)?;
    assert!(
        !ignore
            .matched_path_or_any_parents(&root, root.is_dir())
            .is_ignore(),
        "The path {root} is dvcignored",
    );

    if root.is_file() {
        build_file(&root, state)
    } else {
        build_tree(&root, state, ignore, jobs)
    }
}
