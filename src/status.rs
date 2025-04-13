use crate::build::build;
use crate::diff::{Diff, diff_object, diff_root};
use crate::models::{DvcFile, Output};
use crate::odb::{Odb, oid_to_path};
use crate::state::State;
use crate::{Object, Tree};
use core::str;
use git2;
use ignore::gitignore::Gitignore;
use std::path::Path;
use std::path::PathBuf;
use std::{env, fs};

pub fn diff_obj(root: &Path, old: Option<Object>, new: Option<Object>) -> Diff {
    let mut diff = Diff::default();
    let granular_diff = diff_object(old, new);

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

pub fn get_tree_obj(git_repo: &git2::Repository) -> Option<git2::Tree> {
    git_repo.head().map_or(None, |r| {
        let tree_id = r.peel_to_commit().expect("commit").tree_id();
        Some(git_repo.find_tree(tree_id).expect("expected tree"))
    })
}

pub fn get_oid_for_path(tree: &git2::Tree, path: &Path) -> git2::Oid {
    tree.get_path(path).expect("tree entry for file").id()
}

pub fn status_git(git_repo: &git2::Repository, odb: &Odb, dvcfile_path: &PathBuf) -> Diff {
    let tree = get_tree_obj(git_repo);
    let dvcfile_relpath = env::current_dir()
        .unwrap()
        .strip_prefix(git_repo.workdir().unwrap())
        .unwrap()
        .join(dvcfile_path);
    let oid = match tree {
        Some(t) => get_oid_for_path(&t, &dvcfile_relpath),
        None => {
            return Diff::default();
        }
    };

    let Ok(git_odb) = git_repo.odb() else {
        return Diff::default();
    };
    let git_obj = git_odb.read(oid).expect("object with oid");
    let data = git_obj.data();

    let contents = str::from_utf8(data).expect("Invalid utf8 sequence");

    let dvcfile_obj: DvcFile = serde_yaml::from_str(contents).unwrap();
    let old_out = dvcfile_obj.outs.0;

    let contents = &fs::read_to_string(dvcfile_path).unwrap();
    let dvcfile_obj: DvcFile = serde_yaml::from_str(contents).unwrap();
    let new_out = dvcfile_obj.outs.0;

    let new_oid = new_out.oid;
    let old_oid = old_out.oid;

    assert!(new_out.path == old_out.path);

    let old_obj = if old_oid.ends_with(".dir") {
        let obj_path = oid_to_path(&odb.path, &old_oid);
        let tree = Tree::load_from(&obj_path).unwrap();
        Object::Tree(tree)
    } else {
        Object::HashFile(old_oid.clone())
    };

    let new_obj = if new_oid.ends_with(".dir") {
        let obj_path = oid_to_path(&odb.path, &new_oid);
        let tree = Tree::load_from(&obj_path).unwrap();
        Object::Tree(tree)
    } else {
        Object::HashFile(new_oid.clone())
    };

    let path = dvcfile_path.parent().unwrap().join(new_out.path);
    let diff = diff_obj(&path, Some(old_obj), Some(new_obj));
    diff.merge(diff_root(&path, Some(&old_oid), Some(&new_oid)))
}

pub fn status(
    odb: &Odb,
    state: Option<&State>,
    ignore: &Gitignore,
    jobs: usize,
    dvcfile_path: &PathBuf,
) -> Diff {
    let contents = &fs::read_to_string(dvcfile_path).unwrap();
    let dvcfile_obj: DvcFile = serde_yaml::from_str(contents).unwrap();
    let Output { oid, path, .. } = dvcfile_obj.outs.0;

    let (obj, _) = build(odb, &path, state, ignore, jobs);

    let old_obj = if oid.ends_with(".dir") {
        let obj_path = oid_to_path(&odb.path, &oid);
        let tree = Tree::load_from(&obj_path).unwrap();
        Object::Tree(tree)
    } else {
        Object::HashFile(oid.clone())
    };

    let obj_oid = match obj {
        Object::Tree(ref t) => t.digest().unwrap().1,
        Object::HashFile(ref o) => o.to_string(),
    };
    let diff = diff_obj(&path, Some(old_obj), Some(obj));
    diff.merge(diff_root(&path, Some(&oid), Some(&obj_oid)))
}
