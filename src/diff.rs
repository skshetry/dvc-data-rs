use crate::objects::{Object, Tree};
use crate::odb::{oid_to_path, Odb};
use std::{collections::HashMap, path::Path, path::PathBuf};

#[derive(Default, Debug)]
pub struct Diff {
    pub added: HashMap<PathBuf, String>,
    pub modified: HashMap<PathBuf, (String, String)>,
    pub removed: HashMap<PathBuf, String>,
    pub unchanged: HashMap<PathBuf, String>,
}

impl Diff {
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
}

pub fn diff(odb: &Odb, root: &Path, old: Option<&str>, new: Option<&str>) -> Diff {
    let mut diff = diff_root(root, old, new);
    let granular_diff = diff_oid(odb, old, new);

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
    diff
}

pub fn diff_oid(odb: &Odb, old: Option<&str>, new: Option<&str>) -> Diff {
    let old_obj = match old {
        None => None,
        Some(t) if t.ends_with(".dir") => {
            let path = oid_to_path(&odb.path, t);
            Some(Object::Tree(Tree::load_from(&path)))
        }
        Some(o) => Some(Object::HashFile(o.to_string())),
    };
    let new_obj = match new {
        None => None,
        new if new == old => old_obj.clone(),
        Some(t) if t.ends_with(".dir") => {
            let path = oid_to_path(&odb.path, t);
            Some(Object::Tree(Tree::load_from(&path)))
        }
        Some(o) => Some(Object::HashFile(o.to_string())),
    };
    diff_object(old_obj, new_obj)
}

pub fn diff_object(old: Option<Object>, new: Option<Object>) -> Diff {
    match (old, new) {
        (None | Some(Object::HashFile(_)), None | Some(Object::HashFile(_))) => Diff::default(),
        (Some(Object::Tree(t1)), Some(Object::Tree(t2))) => diff_tree(Some(t1), Some(t2)),
        (None | Some(Object::HashFile(_)), Some(Object::Tree(t))) => diff_tree(None, Some(t)),
        (Some(Object::Tree(t)), Some(Object::HashFile(_)) | None) => diff_tree(Some(t), None),
    }
}

pub enum State {
    Added,
    Modified,
    Removed,
    Unchanged,
}

pub fn diff_root_oid(old: Option<&str>, new: Option<&str>) -> State {
    match (old, new) {
        (None, Some(_)) => State::Added,
        (Some(o), Some(n)) if o != n => State::Modified,
        (Some(_), None) => State::Removed,
        _ => State::Unchanged,
    }
}

pub fn diff_root(root: &Path, old: Option<&str>, new: Option<&str>) -> Diff {
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
        State::Added => {
            diff.added.insert(new_root, new.unwrap().to_string());
            diff
        }
        State::Modified => {
            diff.modified.insert(
                new_root,
                (old.unwrap().to_string(), new.unwrap().to_string()),
            );
            diff
        }
        State::Removed => {
            diff.removed.insert(old_root, old.unwrap().to_string());
            diff
        }
        State::Unchanged => {
            diff.unchanged.insert(new_root, new.unwrap().to_string());
            diff
        }
    }
}

pub fn diff_tree(old: Option<Tree>, new: Option<Tree>) -> Diff {
    let old_tree = old.unwrap_or_default();
    let old_hm: HashMap<PathBuf, String> = old_tree.entries.into_iter().collect();

    let new_tree = new.unwrap_or_default();
    let new_hm: HashMap<PathBuf, String> = new_tree.entries.into_iter().collect();

    let mut removed: HashMap<PathBuf, String> = HashMap::new();
    for (key, value) in &old_hm {
        if !new_hm.contains_key(key) {
            removed.insert(key.clone(), value.to_string());
        }
    }

    let mut added: HashMap<PathBuf, String> = HashMap::new();
    let mut modified: HashMap<PathBuf, (String, String)> = HashMap::new();
    let mut unchanged: HashMap<PathBuf, String> = HashMap::new();
    for (key, new_value) in new_hm {
        if let Some(old_value) = old_hm.get(&key) {
            if new_value == *old_value {
                unchanged.insert(key, new_value.to_string());
            } else {
                modified.insert(key, (old_value.to_string(), new_value.to_string()));
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
