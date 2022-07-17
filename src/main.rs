use clap::{Parser, Subcommand};
use console::style;
use dvc_data::{build, checkout, checkout_obj, create_pool, get_odb, transfer, DvcFile, Object};
use std::error::Error;
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
    },
    Add {
        #[clap(required = true, value_parser)]
        path: PathBuf,
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
        Commands::Build { path, write, jobs } => {
            let threads = create_pool(jobs);
            // println!("{:?}", checksum(&path));
            let (odb, _) = get_odb()?;
            eprintln!("    {} files", style("Staging").green().bold());
            let obj = build(&odb, &path, threads);
            let oid = if write {
                eprintln!("    {} files", style("Transferring").green().bold());
                transfer(&odb, &path, &obj)
            } else {
                match obj {
                    Object::Tree(t) => t.digest().1,
                    Object::HashFile(hf) => hf,
                }
            };
            println!("object {}", oid);
            Ok(())
        }
        Commands::Add { path } => {
            let (odb, _) = get_odb()?;
            let threads = create_pool(None);
            eprintln!("    {} files", style("Staging").green().bold());
            let obj = build(&odb, &path, threads);
            eprintln!("    {} files", style("Transferring").green().bold());
            let oid = transfer(&odb, &path, &obj);
            let filename: String = path.file_name().unwrap().to_str().unwrap().into();
            let dvcfile = path.with_file_name(format!("{}.dvc", filename));
            eprintln!(
                "    {} {}",
                style("Created").green().bold(),
                dvcfile.to_str().unwrap()
            );
            DvcFile::create(&dvcfile, path.as_path(), oid);
            Ok(())
        }
        Commands::CheckoutObject { oid, path } => {
            let (odb, _) = get_odb()?;
            checkout_obj(&odb, &oid, &path);
            Ok(())
        }
        Commands::Checkout { path } => {
            let (odb, _) = get_odb()?;
            checkout(&odb, &path);
            Ok(())
        }
    };
}
