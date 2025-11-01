use camino::{FromPathError, Utf8Path, Utf8PathBuf};

use crate::objects::{Object, Tree, TreeError};
use crate::odb::Odb;
use std::collections::HashMap;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum DiffError {
    #[error(transparent)]
    FromPathError(#[from] FromPathError),
    #[error(transparent)]
    TreeError(#[from] TreeError),
}

#[derive(Default, Debug)]
pub struct Diff {
    pub added: HashMap<Utf8PathBuf, String>,
    pub modified: HashMap<Utf8PathBuf, (String, String)>,
    pub removed: HashMap<Utf8PathBuf, String>,
    pub unchanged: HashMap<Utf8PathBuf, String>,
}

impl Diff {
    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        self.added.extend(other.added);
        self.modified.extend(other.modified);
        self.removed.extend(other.removed);
        self.unchanged.extend(other.unchanged);
        Self {
            added: self.added,
            modified: self.modified,
            removed: self.removed,
            unchanged: self.unchanged,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.modified.is_empty() && self.removed.is_empty()
    }
}

pub fn diff(
    odb: &Odb,
    root: &Utf8Path,
    old: Option<&str>,
    new: Option<&str>,
) -> Result<Diff, DiffError> {
    let mut diff = diff_root(root, old, new);
    let granular_diff = diff_oid(odb, old, new)?;

    for (path, key) in granular_diff.added {
        diff.added.insert(root.join(path), key);
    }
    for (path, (old_key, new_key)) in granular_diff.modified {
        diff.modified.insert(root.join(path), (old_key, new_key));
    }
    for (path, key) in granular_diff.removed {
        diff.removed.insert(root.join(path), key);
    }
    for (path, key) in granular_diff.unchanged {
        diff.unchanged.insert(root.join(path), key);
    }
    Ok(diff)
}

pub fn diff_oid(odb: &Odb, old: Option<&str>, new: Option<&str>) -> Result<Diff, DiffError> {
    let old_obj = match old {
        None => None,
        Some(oid) => Some(odb.load_object(oid)?),
    };
    let new_obj = match new {
        None => None,
        new if new == old => old_obj.clone(),
        Some(oid) => Some(odb.load_object(oid)?),
    };
    Ok(diff_object(old_obj, new_obj))
}

pub fn diff_object(old: Option<Object>, new: Option<Object>) -> Diff {
    match (old, new) {
        (None | Some(Object::HashFile(_)), None | Some(Object::HashFile(_))) => Diff::default(),
        (Some(Object::Tree(t1)), Some(Object::Tree(t2))) => diff_tree(Some(t1), Some(t2)),
        (None | Some(Object::HashFile(_)), Some(Object::Tree(t))) => diff_tree(None, Some(t)),
        (Some(Object::Tree(t)), Some(Object::HashFile(_)) | None) => diff_tree(Some(t), None),
    }
}

pub enum State<'a> {
    Added(&'a str),
    Modified(&'a str, &'a str),
    Removed(&'a str),
    Unchanged(&'a str),
}

pub fn diff_root_oid<'a>(old: Option<&'a str>, new: Option<&'a str>) -> State<'a> {
    match (old, new) {
        (None, Some(n)) => State::Added(n),
        (Some(o), Some(n)) if o != n => State::Modified(o, n),
        (Some(o), None) => State::Removed(o),
        (Some(_), Some(n)) => State::Unchanged(n),
        (None, None) => State::Unchanged(""),
    }
}

pub fn diff_root(root: &Utf8Path, old: Option<&str>, new: Option<&str>) -> Diff {
    let mut diff = Diff::default();

    let old_root = if let Some(old_oid) = old {
        if old_oid.ends_with(".dir") {
            root.join("")
        } else {
            root.to_path_buf()
        }
    } else {
        root.to_path_buf()
    };

    let new_root = if let Some(new_oid) = new {
        if new_oid.ends_with(".dir") {
            root.join("")
        } else {
            root.to_path_buf()
        }
    } else {
        root.to_path_buf()
    };
    match diff_root_oid(old, new) {
        State::Added(n) => {
            diff.added.insert(new_root, n.to_string());
            diff
        }
        State::Modified(o, n) => {
            diff.modified
                .insert(new_root, (o.to_string(), n.to_string()));
            diff
        }
        State::Removed(o) => {
            diff.removed.insert(old_root, o.to_string());
            diff
        }
        State::Unchanged(n) => {
            diff.unchanged.insert(new_root, n.to_string());
            diff
        }
    }
}

pub fn diff_tree(old: Option<Tree>, new: Option<Tree>) -> Diff {
    let old_tree = old.unwrap_or_default();
    let old_hm: HashMap<Utf8PathBuf, String> = old_tree
        .entries
        .into_iter()
        .map(|e| (e.relpath, e.oid))
        .collect();

    let new_tree = new.unwrap_or_default();
    let new_hm: HashMap<Utf8PathBuf, String> = new_tree
        .entries
        .into_iter()
        .map(|e| (e.relpath, e.oid))
        .collect();

    let mut removed: HashMap<Utf8PathBuf, String> = HashMap::new();
    for (key, value) in &old_hm {
        if !new_hm.contains_key(key) {
            removed.insert(key.clone(), value.clone());
        }
    }

    let mut added: HashMap<Utf8PathBuf, String> = HashMap::new();
    let mut modified: HashMap<Utf8PathBuf, (String, String)> = HashMap::new();
    let mut unchanged: HashMap<Utf8PathBuf, String> = HashMap::new();
    for (key, new_value) in new_hm {
        if let Some(old_value) = old_hm.get(&key) {
            if new_value == *old_value {
                unchanged.insert(key, new_value.clone());
            } else {
                modified.insert(key, (old_value.clone(), new_value.clone()));
            }
        } else {
            added.insert(key, new_value);
        }
    }

    Diff {
        added,
        modified,
        removed,
        unchanged,
    }
}
