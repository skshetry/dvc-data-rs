use crate::hash::md5;
use crate::json_format;
use serde::{Deserialize, Serialize};
use std::convert::From;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

#[derive(Clone)]
pub enum Object {
    Tree(Tree),
    HashFile(HashFile),
}

pub type HashFile = String;

#[derive(Deserialize, Clone, PartialEq, Debug, PartialOrd, Ord, Eq)]
pub struct TreeEntry {
    pub relpath: PathBuf,
    #[serde(rename = "md5")]
    pub oid: String,
}

/// private helper for ordering `TreeEntry` on serialization, keep `md5` before relpath
#[derive(Serialize, Debug)]
struct TreeEntrySerializer {
    #[serde(rename = "md5")]
    pub oid: String,
    pub relpath: PathBuf,
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

impl Tree {
    pub fn serialize(&self) -> serde_json::Result<String> {
        // make it compatible with `json.dumps()` separator
        json_format::to_string(self)
    }

    pub fn digest(&self) -> serde_json::Result<(String, String)> {
        let serialized = self.serialize()?;
        let reader = serialized.as_bytes().to_owned();
        Ok((serialized, md5(&mut reader.as_slice()) + ".dir"))
    }

    pub fn load_from(path: &PathBuf) -> Result<Self, Box<dyn Error>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Ok(serde_json::from_reader(reader)?)
    }
}
