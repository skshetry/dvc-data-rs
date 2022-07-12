use clap::{Parser, Subcommand};
use console::style;
use indicatif::{ParallelProgressIterator, ProgressBar};
use json::{object, stringify, JsonValue};
use jwalk::{Parallelism, WalkDir};
use md5::{Digest, Md5};
use rayon::prelude::*;
use std::error::Error;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

#[derive(Debug, Parser)]
#[clap(name = "dvc-data")]
#[clap(about = "dvc-data in rust", long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Build {
        #[clap(required = true, value_parser)]
        path: PathBuf,
        #[clap(short, long)]
        write: bool,
        #[clap(short, long)]
        jobs: Option<usize>,
    },
}

struct Repo {
    root: PathBuf,
}

impl Repo {
    fn control_dir(&self) -> PathBuf {
        self.root.join(".dvc")
    }

    #[allow(dead_code)]
    fn tmp_dir(&self) -> PathBuf {
        self.control_dir().join("tmp")
    }

    fn object_dir(&self) -> PathBuf {
        self.control_dir().join("cache")
    }
}

struct Odb {
    #[allow(dead_code)]
    path: PathBuf,
}

enum Object {
    Tree(Tree),
    HashFile(HashFile),
}

type HashFile = String;

struct Tree {
    entries: Vec<(PathBuf, String)>,
}

impl Tree {
    fn serialize(&self) -> String {
        let mut data = JsonValue::new_array();
        for (path, hash_value) in self.entries.iter() {
            data.push(object! {md5: hash_value.as_str(), relpath: path.to_str()})
                .unwrap();
        }
        // make it compatible with `json.dumps()` separator
        stringify(data).replace(',', ", ").replace(':', ": ")
    }

    fn digest(&self) -> (String, String) {
        let serialized = self.serialize();
        let reader = serialized.as_bytes().to_owned();
        return (serialized, md5(&mut reader.as_slice()) + ".dir");
    }
}

fn md5<R>(reader: &mut R) -> String
where
    R: io::Read,
{
    let mut hasher = Md5::new();
    let _ = io::copy(reader, &mut hasher);
    let hash = hasher.finalize();
    base16ct::lower::encode_string(&hash)
}

fn file_md5(path: PathBuf) -> String {
    let mut file = fs::File::open(&path).unwrap();
    md5(&mut file)
}

fn stage(_odb: &Odb, root: &Path, jobs: usize) -> Object {
    eprintln!("    {} files", style("Staging").green().bold());
    if root.is_file() {
        return Object::HashFile(file_md5(root.to_path_buf()));
    }

    let mut result: Vec<(PathBuf, String)> = WalkDir::new(root)
        .parallelism(Parallelism::RayonNewPool(jobs))
        .into_iter()
        .par_bridge()
        .filter_map(|dir_entry_res| {
            let dentry = dir_entry_res.ok()?;
            if !dentry.file_type().is_file() {
                return None;
            }
            Some(dentry.path())
        })
        .map(|file| {
            let relpath = file.strip_prefix(root).unwrap().to_path_buf();
            let hash_value = file_md5(file);
            (relpath, hash_value)
        })
        .collect();

    result.sort_unstable(); // sort keys
    Object::Tree(Tree { entries: result })
}

fn oid_to_path(root: PathBuf, oid: String) -> PathBuf {
    let mut to = root.join(&oid[..2]);
    to.push(&oid[2..]);
    to
}

fn transfer_file(root: &Path, from: &PathBuf, oid: &String) {
    let to = oid_to_path(root.to_path_buf(), oid.to_string());
    if to.exists() {
        return;
    }
    fs::create_dir_all(to.parent().unwrap()).unwrap();
    reflink::reflink_or_copy(from, to).unwrap();
}

fn write_file(root: &Path, oid: &String, contents: &String) {
    let to = oid_to_path(root.to_path_buf(), oid.to_string());
    if to.exists() {
        return;
    }
    fs::create_dir_all(to.parent().unwrap()).unwrap();
    fs::write(to, contents).unwrap();
}

fn transfer_tree(odb: &Odb, wroot: &Path, tree: &Tree) -> String {
    let pb = ProgressBar::new(tree.entries.len() as u64);
    eprintln!("    {} files", style("Transferring").green().bold());

    fs::create_dir_all(&odb.path).unwrap();
    tree.entries
        .par_iter()
        .progress_with(pb)
        .for_each(|(path, oid)| {
            let file = wroot.join(path);
            transfer_file(&odb.path, &file, oid);
        });

    let (serialized, hash_value) = tree.digest();
    write_file(&odb.path, &hash_value, &serialized);
    hash_value
}

fn transfer(odb: &Odb, wroot: &PathBuf, obj: &Object) -> String {
    match obj {
        Object::HashFile(hf) => {
            transfer_file(&odb.path, wroot, hf);
            hf.to_string()
        }
        Object::Tree(t) => transfer_tree(odb, wroot, t),
    }
}

#[allow(dead_code)]
fn checksum(path: &PathBuf) -> u128 {
    // TODO: will be used to read from DVC's state db
    let meta = fs::metadata(path).unwrap();
    let m = meta.mtime() as f64 + (meta.mtime_nsec() as f64 / 1_000_000_000f64);
    let st = format!("([{}, {}, {}],)", meta.ino(), m, meta.size());
    let hash = md5(&mut st.as_bytes());
    u128::from_str_radix(&hash, 16).unwrap()
}

fn create_pool(num: Option<usize>) -> usize {
    let threads = match num {
        None => num_cpus::get_physical(),
        Some(n) => n,
    };
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .unwrap();
    threads
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();

    return match args.command {
        Commands::Build { path, write, jobs } => {
            let repo = Repo {
                root: env::current_dir()?,
            };
            let odb = Odb {
                path: repo.object_dir(),
            };

            let threads = create_pool(jobs);
            // println!("{:?}", checksum(&path));
            let obj = stage(&odb, &path, threads);
            let hash = if write {
                transfer(&odb, &path, &obj)
            } else {
                match obj {
                    Object::Tree(t) => t.digest().1,
                    Object::HashFile(hf) => hf,
                }
            };
            println!("object {}", hash);
            Ok(())
        }
    };
}
