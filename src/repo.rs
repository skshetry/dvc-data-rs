use std::path::PathBuf;

use crate::odb::Odb;
use crate::state::State;
use std::env;
use std::error::Error;
use std::fs;

#[derive(Debug)]
pub struct Repo {
    pub root: PathBuf,
    pub odb: Odb,
    pub state: State,
}

impl Repo {
    pub fn open(path: Option<PathBuf>) -> Result<Self, Box<dyn Error>> {
        let root = path.unwrap_or(env::current_dir()?);
        let control_dir = root.join(".dvc");
        let tmp_path = control_dir.join("tmp");
        let state_path = tmp_path.join("hashes/local/cache.db");
        let odb_path = control_dir.join("cache");
        let repo = Self {
            root,
            odb: Odb { path: odb_path },
            state: State::open(&state_path)?.instantiate()?,
        };
        Ok(repo)
    }

    pub fn discover(path: Option<PathBuf>) -> Result<Self, Box<dyn Error>> {
        let path = path.unwrap_or(env::current_dir()?);
        let path = fs::canonicalize(path).unwrap();
        for directory in path.ancestors() {
            if directory.join(".dvc").is_dir() {
                return Self::open(Some(directory.to_path_buf()));
            }
        }
        Err("No repository found".into())
    }

    pub fn control_dir(&self) -> PathBuf {
        self.root.join(".dvc")
    }

    pub fn tmp_dir(&self) -> PathBuf {
        self.control_dir().join("tmp")
    }

    pub fn object_dir(&self) -> PathBuf {
        self.control_dir().join("cache")
    }
}
