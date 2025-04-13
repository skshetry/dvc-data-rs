use std::ffi::OsString;
use std::path::{Path, PathBuf};

use log::debug;

use crate::config::Config;
use crate::hash::md5;
use crate::odb::Odb;
use crate::state::State;
use crate::timeutils::unix_time;
use std::error::Error;
use std::fs;
use std::{env, io};

#[derive(Debug)]
pub struct Repo {
    pub root: PathBuf,
    pub odb: Odb,
    pub state: State,
    pub config: Config,
}

fn btime(tmp_dir: &Path) -> Result<f64, io::Error> {
    let btime = tmp_dir.join("btime");
    let mut ex_open = fs::OpenOptions::new();
    ex_open.write(true).create_new(true);
    let result = match ex_open.open(&btime) {
        Ok(_) => Ok(()),
        Err(ref e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    };

    match result {
        Ok(()) => match fs::metadata(btime) {
            Ok(meta) => Ok(unix_time(meta.modified()?).unwrap()),
            Err(e) => Err(e),
        },
        Err(e) => Err(e),
    }
}

fn db_dirname(root: &Path, tmp_dir: &Path) -> Result<String, io::Error> {
    let btime = btime(tmp_dir)?;
    let user = whoami::username();
    let dvc_major = 3;
    let salt = 0;

    let mut st: OsString = "('".into();
    st.push(root.as_os_str());
    st.push("', ");
    st.push("None, "); // subdir
    st.push(btime.to_string());
    st.push(", '");
    st.push(user);
    st.push("', ");
    st.push(dvc_major.to_string());
    st.push(", ");
    st.push(salt.to_string());
    st.push(")");

    Ok(md5(&mut st.as_encoded_bytes()))
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

#[cfg(target_os = "windows")]
fn db_dirs() -> PathBuf {
    Path::new(&env::var("CSIDL_COMMON_APPDATA").unwrap_or("C:/ProgramData/iterative/dvc".into()))
        .to_owned()
}

#[cfg(target_os = "macos")]
fn db_dirs() -> PathBuf {
    "/Library/Caches/dvc/repo".into()
}

impl Repo {
    pub fn open(path: Option<PathBuf>) -> Result<Self, Box<dyn Error>> {
        let root = path.unwrap_or(env::current_dir()?);
        let control_dir = root.join(".dvc");

        let config = Config::new(&control_dir)?;
        debug!("{config:?}");

        let tmp_dir = control_dir.join("tmp");
        fs::create_dir_all(&tmp_dir)?;

        let db_dir = match &config.core.site_cache_dir {
            Some(v) => v.clone(),
            None => db_dirs().join(db_dirname(&root, &tmp_dir)?),
        };
        let object_dir = match &config.cache.dir {
            Some(v) => v.clone(),
            None => control_dir.join("cache").join("files").join("md5"),
        };

        let state_path = db_dir.join("hashes/local/cache.db");
        let repo = Self {
            root,
            odb: Odb { path: object_dir },
            state: State::open(&state_path)?.instantiate()?,
            config,
        };
        debug!("{repo:?}");
        Ok(repo)
    }

    pub fn discover(path: Option<PathBuf>) -> Result<Self, Box<dyn Error>> {
        let path = path.unwrap_or(env::current_dir()?);
        let path = fs::canonicalize(path)?;
        for directory in path.ancestors() {
            if directory.join(".dvc").is_dir() {
                return Self::open(Some(directory.to_path_buf()));
            }
        }
        Err("No repository found".into())
    }
}
