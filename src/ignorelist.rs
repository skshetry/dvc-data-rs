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

    fn as_string(&self, existing: impl BufRead) -> Result<String, std::io::Error> {
        let existing_items = existing.lines().collect::<Result<Vec<_>, _>>()?;
        let mut out = "\n".to_owned();
        for item in &self.ignore {
            if existing_items.contains(item) {
                continue;
            }
            out.push_str(item);
            out.push('\n');
        }
        Ok(out.trim_end().to_string())
    }

    pub fn write(&self, path: &Path) -> Result<(), std::io::Error> {
        let contents = if path.exists() {
            let file = File::open(path)?;
            let reader = BufReader::new(file);
            self.as_string(reader)?
        } else {
            self.as_new()
        };

        let mut file = OpenOptions::new().append(true).create(true).open(path)?;
        file.write_all(contents.as_bytes())?;
        Ok(())
    }
}
