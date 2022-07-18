use clap::{Parser, Subcommand};
use console::style;
use dvc_data::ignore::get_ignore;
use dvc_data::ignorelist;
use dvc_data::repo::Repo;
use dvc_data::{build, checkout, checkout_obj, create_pool, transfer, DvcFile, Object};
use std::error::Error;
use std::fs;
use std::path::PathBuf;

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
        #[clap(long, takes_value = false)]
        no_state: bool,
    },
    Add {
        #[clap(required = true, value_parser)]
        path: PathBuf,
        #[clap(long, takes_value = false)]
        no_state: bool,
    },
    CheckoutObject {
        #[clap(required = true)]
        oid: String,
        #[clap(required = true, value_parser)]
        path: PathBuf,
    },
    Checkout {
        #[clap(required = true, value_parser)]
        path: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();

    return match args.command {
        Commands::Build {
            path,
            write,
            jobs,
            no_state,
        } => {
            let threads = create_pool(jobs);
            let repo = Repo::discover(None)?;
            let state = if no_state { None } else { Some(&repo.state) };
            eprintln!("    {} files", style("Staging").green().bold());

            let abspath = fs::canonicalize(path.clone())?;
            let ignore = get_ignore(&repo.root, abspath.parent().unwrap());
            let obj = build(&repo.odb, &path, state, &ignore, threads);

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
            let threads = create_pool(None);
            eprintln!("    {} files", style("Staging").green().bold());

            let abspath = fs::canonicalize(path.clone())?;
            let ignore = get_ignore(&repo.root, abspath.parent().unwrap());
            let obj = build(&repo.odb, &path, state, &ignore, threads);
            eprintln!("    {} files", style("Transferring").green().bold());

            let oid = transfer(&repo.odb, &path, &obj);
            let filename: String = path.file_name().unwrap().to_str().unwrap().into();
            let dvcfile = path.with_file_name(format!("{}.dvc", filename));
            eprintln!(
                "    {} {}",
                style("Created").green().bold(),
                dvcfile.to_str().unwrap()
            );
            DvcFile::create(&dvcfile, path.as_path(), oid);

            let mut ignorelst = ignorelist::IgnoreList { ignore: Vec::new() };
            ignorelst
                .ignore
                .push(format!("/{}", path.file_name().unwrap().to_str().unwrap()));
            let gitignore = path.with_file_name(".gitignore");
            ignorelst.write(&gitignore);

            Ok(())
        }
        Commands::CheckoutObject { oid, path } => {
            let repo = Repo::discover(None)?;
            checkout_obj(&repo.odb, &oid, &path);
            Ok(())
        }
        Commands::Checkout { path } => {
            let repo = Repo::discover(None)?;
            checkout(&repo.odb, &path);
            Ok(())
        }
    };
}
