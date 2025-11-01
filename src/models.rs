use camino::{FromPathError, Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub hash: String,
    #[serde(rename = "md5")]
    pub oid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nfiles: Option<usize>,
    pub path: Utf8PathBuf,
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
    #[error(transparent)]
    FromPathError(#[from] FromPathError),
}

const DVCFILE_EXT: &str = "dvc";

pub fn default_dvcfile_path(path: &Utf8Path) -> Utf8PathBuf {
    match path.extension() {
        Some(ext) => path.with_extension(ext.to_owned() + "." + DVCFILE_EXT),
        None => path.with_extension(DVCFILE_EXT),
    }
}

pub fn path_relative_to_dvcfile(
    dvcfile: &Utf8Path,
    path: &Utf8Path,
) -> Result<Utf8PathBuf, std::path::StripPrefixError> {
    let wdir = dvcfile
        .parent()
        .expect("expected dvcfile to have a parent directory");
    path.strip_prefix(wdir).map(Utf8Path::to_path_buf)
}

pub fn absolute_output_path(dvcfile: &Utf8Path, path: &Utf8Path) -> Utf8PathBuf {
    dvcfile
        .parent()
        .expect("expected dvcfile to have a parent directory")
        .join(path)
}

impl DvcFile {
    pub fn create(
        dvcfile: &Path,
        path: &Utf8Path,
        oid: String,
        size: Option<u64>,
        nfiles: Option<usize>,
    ) -> Result<(), DvcFileCreateError> {
        let output = Output {
            oid,
            size,
            nfiles,
            hash: "md5".to_owned(),
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
