use crate::fsutils;
use crate::hash::file_md5;
use crate::objects::{Object, Tree};
use crate::odb::Odb;
use crate::state::{State, StateHash, StateValue};
use jwalk::{Parallelism, WalkDir};
use rayon::prelude::*;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

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

fn get_hashes<'a>(
    file_infos: impl Iterator<Item = &'a FileInfo>,
    state: &State,
) -> (Vec<&'a FileInfo>, Vec<(FileInfo, String)>) {
    let mut new_files = Vec::new();
    let mut cached_entries = Vec::new();
    for file_info in file_infos {
        let file = &file_info.path;
        let value = state.get((*file).to_str().unwrap().to_string()).unwrap();
        match value {
            None => new_files.push(file_info),
            Some(st) => {
                if st.checksum != file_info.checksum {
                    new_files.push(file_info)
                } else {
                    let oid = st.hash_info.oid.to_owned();
                    let file_info = FileInfo::from_state(&file_info.path, st);
                    cached_entries.push((file_info, oid))
                }
            }
        }
    }
    (new_files, cached_entries)
}

fn set_hashes<'a>(entries: impl Iterator<Item = &'a (&'a &'a FileInfo, String)>, state: &State) {
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
    state.set_many(state_hashes).unwrap();
}

pub fn build(_odb: &Odb, root: &Path, jobs: usize) -> Object {
    if root.is_file() {
        return Object::HashFile(file_md5(root.to_path_buf()));
    }

    let files: Vec<FileInfo> = WalkDir::new(root)
        .parallelism(Parallelism::RayonNewPool(jobs))
        .into_iter()
        .par_bridge()
        .filter_map(|dir_entry_res| {
            let dentry = dir_entry_res.ok()?;
            if !dentry.file_type().is_file() {
                return None;
            }
            Some(FileInfo::from_metadata(
                &dentry.path(),
                &dentry.metadata().unwrap(),
            ))
        })
        .collect();

    let state = State::open(&PathBuf::from(".dvc/tmp/hashes/local/cache.db"))
        .unwrap()
        .instantiate()
        .unwrap();
    let (new_files, cached_entries) = get_hashes(files.iter(), &state);

    let new_entries: Vec<(&&FileInfo, String)> = new_files
        .par_iter()
        .map(|file_info| (file_info, file_md5(file_info.path.to_path_buf())))
        .collect();

    set_hashes(new_entries.iter(), &state);

    let mut tree_entries: Vec<(PathBuf, String)> = Vec::new();
    for (file_info, oid) in cached_entries {
        let relpath = file_info.path.strip_prefix(root).unwrap().to_path_buf();
        tree_entries.push((relpath, oid.to_owned()))
    }

    for (file_info, oid) in new_entries {
        let relpath = file_info.path.strip_prefix(root).unwrap().to_path_buf();
        tree_entries.push((relpath, oid.to_owned()))
    }

    tree_entries.sort_unstable(); // sort keys
    Object::Tree(Tree {
        entries: tree_entries,
    })
}
