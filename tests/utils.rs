use std::fs::File;
use std::io::Write;
use std::path::Path;

#[macro_export]
macro_rules! t {
    ($e:expr) => {
        match $e {
            Ok(n) => n,
            Err(e) => panic!("error: {}", e),
        }
    };
}

pub fn write_to_temp_file<P: AsRef<Path>>(dir: &Path, file_path: P, content: &str) -> File {
    let file_path = dir.join(file_path);
    let mut file = t!(File::create(file_path));
    t!(writeln!(file, "{}", content));
    file
}
