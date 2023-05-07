use clap::{Parser, Subcommand};
use console::style;
use dvc_data::ignore::get_ignore;
use dvc_data::ignorelist;
use dvc_data::repo::Repo;
use dvc_data::{build, checkout, checkout_obj, create_pool, transfer, DvcFile, Object};
use env_logger::Env;
use log::debug;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "dvc-data")]
#[command(about = "dvc-data in rust", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Build {
        path: PathBuf,
        #[arg(short, long)]
        write: bool,
        #[arg(short, long)]
        jobs: Option<usize>,
        #[arg(long)]
        no_state: bool,
    },
    Add {
        path: PathBuf,
        #[arg(long)]
        no_state: bool,
    },
    CheckoutObject {
        oid: String,
        path: PathBuf,
    },
    Checkout {
        path: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();
    let level = if args.verbose { "debug" } else { "warn" };
    env_logger::Builder::from_env(Env::default().default_filter_or(level)).init();

    return match args.command {
        Commands::Build {
            path,
            write,
            jobs,
            no_state,
        } => {
            let repo = Repo::discover(None)?;
            let threads = create_pool(jobs.or(repo.config.core.checksum_jobs));
            let state = if no_state { None } else { Some(&repo.state) };
            eprintln!("    {} files", style("Staging").green().bold());

            let abspath = fs::canonicalize(path.clone())?;
            let ignore = get_ignore(&repo.root, abspath.parent().unwrap());
            let (obj, size) = build(&repo.odb, &path, state, &ignore, threads);

            match &obj {
                Object::Tree(t) => debug!("size: {}, nfiles: {}", size, t.entries.len()),
                Object::HashFile(_) => debug!("size: {}", size),
            }

            let oid = if write {
                eprintln!("    {} files", style("Transferring").green().bold());
                transfer(&repo.odb, &path, &obj)
            } else {
                match obj {
                    Object::Tree(t) => t.digest().1,
                    Object::HashFile(hf) => hf,
                }
            };
            println!("object {}", oid);

            Ok(())
        }
        Commands::Add { path, no_state } => {
            let repo = Repo::discover(None)?;
            let state = if no_state { None } else { Some(&repo.state) };
            let threads = create_pool(repo.config.core.checksum_jobs);
            eprintln!("    {} files", style("Staging").green().bold());

            let abspath = fs::canonicalize(path.clone())?;
            let ignore = get_ignore(&repo.root, abspath.parent().unwrap());
            let (obj, size) = build(&repo.odb, &path, state, &ignore, threads);
            eprintln!("    {} files", style("Transferring").green().bold());

            let oid = transfer(&repo.odb, &path, &obj);
            let filename: String = path.file_name().unwrap().to_str().unwrap().into();
            let dvcfile = path.with_file_name(format!("{}.dvc", filename));
            eprintln!(
                "    {} {}",
                style("Created").green().bold(),
                dvcfile.to_str().unwrap()
            );
            let nfiles = match obj {
                Object::Tree(t) => Some(t.entries.len()),
                _ => None,
            };
            DvcFile::create(&dvcfile, path.as_path(), oid, Some(size), nfiles);

            if !repo.config.core.no_scm {
                let mut ignorelst = ignorelist::IgnoreList { ignore: Vec::new() };
                ignorelst
                    .ignore
                    .push(format!("/{}", path.file_name().unwrap().to_str().unwrap()));
                let gitignore = path.with_file_name(".gitignore");
                ignorelst.write(&gitignore);
            }
            Ok(())
        }
        Commands::CheckoutObject { oid, path } => {
            let repo = Repo::discover(None)?;
            if repo.config.cache.typ.is_some() {
                eprintln!("link type other than 'reflink,copy' is unsupported.");
            }
            checkout_obj(&repo.odb, &oid, &path);
            Ok(())
        }
        Commands::Checkout { path } => {
            let repo = Repo::discover(None)?;
            if repo.config.cache.typ.is_some() {
                eprintln!("link type other than 'reflink,copy' is unsupported.");
            }
            checkout(&repo.odb, &path);
            Ok(())
        }
    };
}
