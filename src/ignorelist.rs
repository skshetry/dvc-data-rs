use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

pub struct IgnoreList {
    pub ignore: Vec<String>,
}

impl IgnoreList {
    fn as_new(&self) -> String {
        self.ignore.join("\n") + "\n"
    }

    fn as_string(&self, existing: impl BufRead) -> String {
        let existing_items = existing.lines().collect::<Result<Vec<_>, _>>().unwrap();
        let mut out = "\n".to_string();
        for item in self.ignore.iter() {
            if existing_items.contains(item) {
                continue;
            }
            out.push_str(item);
            out.push('\n');
        }
        out.trim_end().to_string()
    }

    pub fn write(&self, path: &Path) {
        let contents = if path.exists() {
            let file = File::open(path).unwrap();
            let reader = BufReader::new(file);
            self.as_string(reader)
        } else {
            self.as_new()
        };

        let mut file = OpenOptions::new()
            .write(true)
            .append(true)
            .create(true)
            .open(path)
            .unwrap();
        file.write_all(contents.as_bytes()).unwrap();
    }
}
