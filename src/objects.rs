use crate::hash::md5;
use crate::json_format;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize, Serializer};
use std::convert::From;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum TreeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub enum Object {
    Tree(Tree),
    HashFile(HashFile),
}

pub type HashFile = String;
pub type Oid = String;

#[derive(Deserialize, Clone, PartialEq, Debug, PartialOrd, Ord, Eq)]
pub struct TreeEntry {
    pub relpath: Utf8PathBuf,
    #[serde(rename = "md5")]
    pub oid: Oid,
}

/// private helper for ordering `TreeEntry` on serialization, keep `md5` before relpath
#[derive(Serialize, Debug)]
struct TreeEntrySerializer {
    #[serde(rename = "md5")]
    pub oid: Oid,
    #[serde(serialize_with = "posixify_path")]
    pub relpath: Utf8PathBuf,
}

impl From<&TreeEntry> for TreeEntrySerializer {
    fn from(value: &TreeEntry) -> Self {
        Self {
            oid: value.oid.clone(),
            relpath: value.relpath.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(transparent)]
pub struct Tree {
    pub entries: Vec<TreeEntry>,
}

impl Serialize for TreeEntry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        TreeEntrySerializer::from(self).serialize(serializer)
    }
}

fn posixify_path<S>(x: &Utf8Path, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let parts = x.iter().collect::<Vec<_>>();
    s.serialize_str(&parts.join("/"))
}

impl Tree {
    pub fn serialize(&self) -> Result<String, TreeError> {
        // make it compatible with `json.dumps()` separator
        Ok(json_format::to_string(self)?)
    }

    pub fn digest(&self) -> Result<(String, String), TreeError> {
        let serialized = self.serialize()?;
        let reader = serialized.as_bytes().to_owned();
        Ok((serialized, md5(&mut reader.as_slice()) + ".dir"))
    }

    pub fn load_from(path: &PathBuf) -> Result<Self, TreeError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Ok(serde_json::from_reader(reader)?)
    }
}
