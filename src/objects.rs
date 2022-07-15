use crate::hash::md5;
use json::{object, parse, stringify, JsonValue};
use std::fs;
use std::path::PathBuf;

pub enum Object {
    Tree(Tree),
    HashFile(HashFile),
}

pub type HashFile = String;

pub struct Tree {
    pub entries: Vec<(PathBuf, String)>,
}

impl Tree {
    pub fn serialize(&self) -> String {
        let mut data = JsonValue::new_array();
        for (path, oid) in self.entries.iter() {
            data.push(object! {md5: oid.as_str(), relpath: path.to_str()})
                .unwrap();
        }
        // make it compatible with `json.dumps()` separator
        stringify(data).replace(',', ", ").replace(':', ": ")
    }

    pub fn digest(&self) -> (String, String) {
        let serialized = self.serialize();
        let reader = serialized.as_bytes().to_owned();
        return (serialized, md5(&mut reader.as_slice()) + ".dir");
    }

    pub fn load_from(path: &PathBuf) -> Self {
        let obj = parse(&fs::read_to_string(path).unwrap()).unwrap();
        let mut entries: Vec<(PathBuf, String)> = Vec::new();
        for entry in obj.members() {
            let relpath = PathBuf::from(entry["relpath"].as_str().unwrap());
            let oid = entry["md5"].as_str().unwrap().to_string();
            entries.push((relpath, oid));
        }
        Self { entries }
    }
}
