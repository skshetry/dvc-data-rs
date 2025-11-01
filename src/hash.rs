use md5::{Digest, Md5};
use std::path::Path;
use std::{fs, io};

pub fn md5<R>(reader: &mut R) -> String
where
    R: std::io::Read,
{
    let mut hasher = Md5::new();
    let _: Result<_, _> = io::copy(reader, &mut hasher);
    let hash = hasher.finalize();
    base16ct::lower::encode_string(&hash)
}

pub fn file_md5<P: AsRef<Path>>(path: &P) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    Ok(md5(&mut file))
}
