use std::ffi::OsString;
use std::path::{Path, PathBuf};

use log::debug;

use crate::config::Config;
use crate::hash::md5;
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
    pub config: Config,
}

#[cfg(unix)]
fn db_dirname(root: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let user = users::get_current_username().unwrap();
    let mut st: OsString = "('".into();
    st.push(root.as_os_str());
    st.push("', '");
    st.push(user);
    st.push("')");

    md5(&mut st.as_bytes())
}

#[cfg(not(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "android",
    target_os = "ios"
)))]
fn db_dirs() -> PathBuf {
    "/var/tmp/dvc/repo".into()
}

#[cfg(any(target_os = "macos"))]
fn db_dirs() -> PathBuf {
    "/Library/Caches/dvc/repo".into()
}

impl Repo {
    pub fn open(path: Option<PathBuf>) -> Result<Self, Box<dyn Error>> {
        let root = path.unwrap_or(env::current_dir()?);
        let control_dir = root.join(".dvc");

        let config = Config::new(&control_dir)?;
        debug!("{:?}", config);

        let db_dir = match &config.core.site_cache_dir {
            Some(v) => v.clone(),
            None => Repo::db_dir(&root),
        };
        let object_dir = match &config.cache.dir {
            Some(v) => v.clone(),
            None => control_dir.join("cache"),
        };

        let state_path = db_dir.join("hashes/local/cache.db");
        let repo = Self {
            root,
            odb: Odb { path: object_dir },
            state: State::open(&state_path)?.instantiate()?,
            config,
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

    pub fn db_dir(root: &Path) -> PathBuf {
        db_dirs().join(db_dirname(root))
    }
}
