use std::path::{Path, PathBuf};

use crate::{Object, Tree, objects::TreeError};

#[derive(Debug)]
pub struct Odb {
    pub path: PathBuf,
}

pub fn oid_to_path(root: &Path, oid: &str) -> PathBuf {
    let mut to = root.join(&oid[..2]);
    to.push(&oid[2..]);
    to
}

impl Odb {
    pub fn load_object(&self, oid: &str) -> Result<Object, TreeError> {
        if oid.ends_with(".dir") {
            let path = oid_to_path(&self.path, oid);
            let tree = Tree::load_from(&path)?;
            Ok(Object::Tree(tree))
        } else {
            Ok(Object::HashFile(oid.to_string()))
        }
    }
}
