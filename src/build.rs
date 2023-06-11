use crate::fsutils;
use crate::hash::file_md5;
use crate::objects::{Object, Tree, TreeEntry};
use crate::odb::Odb;
use crate::state::{State, StateHash, StateValue};
use ignore;
use ignore::gitignore::Gitignore;
use jwalk::{Parallelism, WalkDir};
use log::debug;
use rayon::prelude::*;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::Instant;
struct FileInfo {
    checksum: String,
    path: PathBuf,
    size: u64,
}

impl FileInfo {
    fn from_metadata(path: &Path, meta: &fs::Metadata) -> Self {
        Self {
            checksum: fsutils::checksum_from_metadata(meta),
            size: meta.size(),
            path: path.to_path_buf(),
        }
    }

    fn from_state(path: &Path, st: StateValue) -> Self {
        Self {
            checksum: st.checksum,
            size: st.size,
            path: path.to_path_buf(),
        }
    }
}

fn get_hashes(
    file_infos: Vec<FileInfo>,
    state: Option<&State>,
) -> (Vec<FileInfo>, Vec<(FileInfo, String)>) {
    let mut new_files = Vec::new();
    let mut cached_entries = Vec::new();

    let is_empty = match state {
        None => true,
        Some(s) => s.is_empty().unwrap_or(false),
    };
    if is_empty {
        return (file_infos, cached_entries);
    }

    let s = state.unwrap();

    let keys: Vec<String> = file_infos
        .iter()
        .map(|file_info| {
            let file = &file_info.path;
            file.to_str().unwrap().to_string()
        })
        .collect();
    let mut m = s.get_many(keys.iter(), None).unwrap();

    for file_info in file_infos {
        let file = &file_info.path;
        let key = file.to_str().unwrap().to_string();
        match m.remove(&key) {
            Some(v) if v.checksum == file_info.checksum => {
                let oid = v.hash_info.oid.clone();
                let file_info = FileInfo::from_state(&file_info.path, v);
                cached_entries.push((file_info, oid));
            }
            _ => new_files.push(file_info),
        }
    }
    (new_files, cached_entries)
}

fn set_hashes<'a>(
    entries: impl Iterator<Item = &'a (&'a FileInfo, String)>,
    state: Option<&State>,
) {
    if let Some(s) = state {
        let state_hashes = entries.map(|(file_info, oid)| {
            let file = &file_info.path;
            let checksum = &file_info.checksum;
            (
                file.to_str().unwrap().to_string(),
                StateValue {
                    checksum: checksum.to_string(),
                    hash_info: StateHash {
                        oid: oid.to_string(),
                    },
                    size: file_info.size,
                },
            )
        });
        s.set_many(state_hashes).unwrap();
    }
}

fn _build_file(root: &PathBuf, state: Option<&State>) -> (Object, u64) {
    let file_info = FileInfo::from_metadata(root, &fs::metadata(root).unwrap());
    let state_value: Option<StateValue> = match state {
        None => None,
        Some(s) => {
            let value = s.get((*root).to_str().unwrap()).unwrap();
            match value {
                None => None,
                Some(st) => {
                    if st.checksum == file_info.checksum {
                        Some(st)
                    } else {
                        None
                    }
                }
            }
        }
    };
    let oid = match state_value {
        None => {
            let oid = file_md5(root);
            let sv = StateValue {
                checksum: file_info.checksum.to_string(),
                hash_info: StateHash {
                    oid: oid.to_string(),
                },
                size: file_info.size,
            };
            if let Some(s) = state {
                s.set(root.to_str().unwrap(), &sv).unwrap();
            };
            oid
        }
        Some(st) => st.hash_info.oid,
    };
    (Object::HashFile(oid), file_info.size)
}

pub fn build(
    _odb: &Odb,
    root: &Path,
    state: Option<&State>,
    ignore: &Gitignore,
    jobs: usize,
) -> (Object, u64) {
    let root = fs::canonicalize(root).unwrap();
    assert!(
        !ignore
            .matched_path_or_any_parents(&root, root.is_dir())
            .is_ignore(),
        "The path {} is dvcignored",
        root.display(),
    );

    if root.is_file() {
        return _build_file(&root, state);
    }

    let walk_start = Instant::now();
    let files: Vec<FileInfo> = WalkDir::new(&root)
        .follow_links(true)
        .parallelism(Parallelism::RayonNewPool(jobs))
        .into_iter()
        .par_bridge()
        .filter_map(|dir_entry_res| {
            let dentry = dir_entry_res.ok()?;
            if !dentry.file_type().is_file() {
                return None;
            }
            let path = &dentry.path();

            if ignore.matched_path_or_any_parents(path, false).is_ignore() {
                return None;
            }
            Some(FileInfo::from_metadata(path, &dentry.metadata().unwrap()))
        })
        .collect();

    let size: u64 = files.par_iter().map(|fi| fi.size).sum();
    debug!("time to walk {:?}", walk_start.elapsed());

    let check_hashed_start = Instant::now();
    let (new_files, cached_entries) = get_hashes(files, state);
    debug!(
        "time to check if files are already hashed {:?}",
        check_hashed_start.elapsed()
    );

    let hash_start = Instant::now();
    let new_entries: Vec<(&FileInfo, String)> = new_files
        .par_iter()
        .map(|file_info| (file_info, file_md5(&file_info.path)))
        .collect();
    debug!("time to hash {:?}", hash_start.elapsed());

    let save_hashes_start = Instant::now();
    set_hashes(new_entries.iter(), state);
    debug!("time to save hashes {:?}", save_hashes_start.elapsed());

    let build_tree_start = Instant::now();
    let mut tree_entries: Vec<TreeEntry> =
        Vec::with_capacity(cached_entries.len() + new_entries.len());
    for (file_info, oid) in cached_entries {
        let relpath = file_info.path.strip_prefix(&root).unwrap().to_path_buf();
        tree_entries.push(TreeEntry {
            relpath,
            oid: oid.clone(),
        });
    }

    for (file_info, oid) in new_entries {
        let relpath = file_info.path.strip_prefix(&root).unwrap().to_path_buf();
        tree_entries.push(TreeEntry {
            relpath,
            oid: oid.clone(),
        });
    }

    tree_entries.par_sort_unstable(); // sort keys

    debug!("time to build tree {:?}", build_tree_start.elapsed());
    let tree = Object::Tree(Tree {
        entries: tree_entries,
    });
    (tree, size)
}
