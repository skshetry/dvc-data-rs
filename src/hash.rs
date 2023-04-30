use content_inspector::inspect;
use md5::{Digest, Md5};
use newline_converter::dos2unix;
use std::io::Read;
use std::path::PathBuf;
use std::{fs, io};

pub fn md5<R>(reader: &mut R) -> String
where
    R: std::io::Read,
{
    let mut hasher = Md5::new();
    let _ = io::copy(reader, &mut hasher);
    let hash = hasher.finalize();
    base16ct::lower::encode_string(&hash)
}

pub fn is_text_file(path: &PathBuf) -> bool {
    let mut file = fs::File::open(path).unwrap();
    let mut block = [0; 512];
    if file.read(&mut block).is_ok() {
        return block.is_empty() || inspect(&block).is_text();
    }
    false
}

pub fn file_md5(path: PathBuf) -> String {
    let mut file = fs::File::open(&path).unwrap();
    let md5 = if is_text_file(&path) {
        println!("dos2unix converting text file: {:?}", &path);
        let mut text = String::new();
        file.read_to_string(&mut text).unwrap();
        let new_text = dos2unix(&text);
        let mut bytes = new_text.as_bytes();
        md5(&mut bytes)
    } else {
        md5(&mut file)
    };
    md5
}
