use std::path::PathBuf;

pub struct Repo {
    pub root: PathBuf,
}

impl Repo {
    pub fn control_dir(&self) -> PathBuf {
        self.root.join(".dvc")
    }

    #[allow(dead_code)]
    pub fn tmp_dir(&self) -> PathBuf {
        self.control_dir().join("tmp")
    }

    pub fn object_dir(&self) -> PathBuf {
        self.control_dir().join("cache")
    }
}
