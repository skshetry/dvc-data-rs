use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    #[serde(rename = "md5")]
    pub oid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nfiles: Option<usize>,
    pub hash: String,
    pub path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DvcFile {
    pub outs: (Output,),
}

#[derive(Error, Debug)]
pub enum DvcFileCreateError {
    #[error("failed to validate contents")]
    InvalidFormat(#[from] serde_yaml::Error),
    #[error("failed to write")]
    FailedToWrite(#[from] std::io::Error),
}

impl DvcFile {
    pub fn create(
        dvcfile: &PathBuf,
        path: &Path,
        oid: String,
        size: Option<u64>,
        nfiles: Option<usize>,
    ) -> Result<(), DvcFileCreateError> {
        let output = Output {
            oid,
            size,
            nfiles,
            hash: "md5".to_string(),
            path: path.to_path_buf(),
        };
        let dvcfile_obj = Self { outs: (output,) };
        let contents = serde_yaml::to_string(&dvcfile_obj)?;
        let processed = contents
            .strip_prefix("---")
            .unwrap_or(&contents)
            .trim_start();
        Ok(fs::write(dvcfile, processed)?)
    }
}
