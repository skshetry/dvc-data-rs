use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    #[serde(rename = "md5")]
    pub oid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nfiles: Option<usize>,
    pub path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DvcFile {
    pub outs: (Output,),
}

impl DvcFile {
    pub fn create(
        dvcfile: &PathBuf,
        path: &Path,
        oid: String,
        size: Option<u64>,
        nfiles: Option<usize>,
    ) {
        let output = Output {
            oid,
            size,
            nfiles,
            path: path.to_path_buf(),
        };
        let dvcfile_obj = Self { outs: (output,) };
        let contents = serde_yaml::to_string(&dvcfile_obj).unwrap();
        let processed = contents
            .strip_prefix("---")
            .unwrap_or(&contents)
            .trim_start();
        fs::write(dvcfile, processed).unwrap();
    }
}
