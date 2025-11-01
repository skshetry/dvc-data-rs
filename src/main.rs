use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use console::style;
use dvc_data::diff::Diff;
use dvc_data::ignore::get_ignore;
use dvc_data::models::{default_dvcfile_path, path_relative_to_dvcfile};
use dvc_data::repo::Repo;
use dvc_data::status::{status, status_git};
use dvc_data::{DvcFile, Object, build, checkout, checkout_obj, create_pool, transfer};
use dvc_data::{diff, ignorelist};
use env_logger::Env;
use git2::Repository;
use log::debug;
use std::env::set_current_dir;
use std::error::Error;
use std::path::PathBuf;
use std::str;

const ROOT: &str = "root";
#[derive(Debug, Parser)]
#[command(name = "dvc-data")]
#[command(about = "dvc-data in rust", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, global = true)]
    verbose: bool,

    #[arg(long, global = true)]
    cd: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Build {
        path: Utf8PathBuf,
        #[arg(short, long)]
        write: bool,
        #[arg(short, long)]
        jobs: Option<usize>,
        #[arg(long)]
        no_state: bool,
    },
    Add {
        path: Utf8PathBuf,
        #[arg(long)]
        no_state: bool,
    },
    CheckoutObject {
        oid: String,
        path: Utf8PathBuf,
    },
    Checkout {
        path: Utf8PathBuf,
    },
    Diff {
        old: String,
        new: Option<String>,
    },
    Status {
        path: Utf8PathBuf,
    },
}

#[allow(clippy::too_many_lines)]
fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();
    let level = if args.verbose { "debug" } else { "warn" };
    env_logger::Builder::from_env(Env::default().default_filter_or(level)).init();

    if let Some(dir) = args.cd {
        set_current_dir(dir)?;
    }
    match args.command {
        Commands::Build {
            path,
            write,
            jobs,
            no_state,
        } => {
            let repo = Repo::discover(None)?;
            let threads = create_pool(jobs.or(repo.config.core.checksum_jobs))?;
            let state = if no_state { None } else { Some(&repo.state) };
            eprintln!("    {} files", style("Staging").green().bold());

            let abspath = camino::absolute_utf8(&path)?;
            let ignore = get_ignore(
                &repo.root,
                abspath
                    .parent()
                    .expect("failed to determine parent directory")
                    .as_std_path(),
            )?;
            let (obj, size) = build(&repo.odb, &abspath, state, &ignore, threads)?;

            match &obj {
                Object::Tree(t) => debug!("size: {}, nfiles: {}", size, t.entries.len()),
                Object::HashFile(_) => debug!("size: {size}"),
            }

            let oid = if write {
                eprintln!("    {} files", style("Transferring").green().bold());
                transfer(&repo.odb, abspath.as_std_path(), &obj)?
            } else {
                match obj {
                    Object::Tree(t) => t.digest()?.1,
                    Object::HashFile(hf) => hf,
                }
            };
            println!("object {oid}");

            Ok(())
        }
        Commands::Add { path, no_state } => {
            let repo = Repo::discover(None)?;
            let state = if no_state { None } else { Some(&repo.state) };
            let threads = create_pool(repo.config.core.checksum_jobs)?;
            eprintln!("    {} files", style("Staging").green().bold());

            let abspath = camino::absolute_utf8(&path)?;
            let ignore = get_ignore(
                &repo.root,
                abspath
                    .parent()
                    .expect("failed to determine parent directory")
                    .as_std_path(),
            )?;
            let (obj, size) = build(&repo.odb, &abspath, state, &ignore, threads)?;
            eprintln!("    {} files", style("Transferring").green().bold());

            let oid = transfer(&repo.odb, abspath.as_std_path(), &obj)?;
            let nfiles = match obj {
                Object::Tree(t) => Some(t.entries.len()),
                Object::HashFile(_) => None,
            };
            let dvcfile = default_dvcfile_path(&abspath);
            let out_path = path_relative_to_dvcfile(&dvcfile, &abspath)?;
            DvcFile::create(dvcfile.as_std_path(), &out_path, oid, Some(size), nfiles)?;
            eprintln!(
                "    {} {}",
                style("Created").green().bold(),
                dvcfile.as_str()
            );

            if !repo.config.core.no_scm {
                let mut ignorelst = ignorelist::IgnoreList { ignore: Vec::new() };
                ignorelst.ignore.push(format!(
                    "/{}",
                    abspath.file_name().expect("expected to have filename")
                ));
                let gitignore = path.with_file_name(".gitignore");
                ignorelst.write(&gitignore.into_std_path_buf())?;
            }
            Ok(())
        }
        Commands::CheckoutObject { oid, path } => {
            let repo = Repo::discover(None)?;
            checkout_obj(&repo.odb, &oid, &path, &repo.config.cache.typ)?;
            Ok(())
        }
        Commands::Checkout { path } => {
            let repo = Repo::discover(None)?;
            checkout(&repo.odb, &path, &repo.config.cache.typ)?;
            Ok(())
        }
        Commands::Diff { old, new } => {
            let repo = Repo::discover(None)?;
            let d = diff::diff_oid(&repo.odb, Some(&old), new.as_deref())?;

            for (path, key) in d.added {
                println!("added: {} ({})", path.as_str(), key);
            }
            for (path, key) in d.removed {
                println!("removed: {} ({})", path.as_str(), key);
            }
            for (path, (new, old)) in d.modified {
                println!(
                    "modified: {} ({}) -> {} ({})",
                    path.as_str(),
                    old,
                    path.as_str(),
                    new
                );
            }

            match diff::diff_root_oid(Some(&old), new.as_deref()) {
                diff::State::Added(n) => println!("added: {ROOT} ({n})"),
                diff::State::Modified(o, n) => {
                    println!("modified: {ROOT} ({o}) -> {ROOT} ({n})");
                }
                diff::State::Removed(o) => println!("removed: {ROOT} ({o})"),
                diff::State::Unchanged(_) => (),
            }
            Ok(())
        }
        Commands::Status { path } => {
            let repo = Repo::discover(None)?;
            let threads = create_pool(repo.config.core.checksum_jobs)?;
            let state = Some(&repo.state);

            let abspath = camino::absolute_utf8(&path)?;
            let ignore = get_ignore(
                &repo.root,
                abspath
                    .parent()
                    .expect("failed to determine parent directory")
                    .as_std_path(),
            )?;

            let diff = match Repository::discover(repo.root) {
                Ok(git_repo) => status_git(&git_repo, &repo.odb, &path)?,
                Err(e) => {
                    debug!("{e}");
                    Diff::default()
                }
            };
            let commit_diff = !diff.is_empty() && {
                println!("DVC committed changes:");
                for added in diff.added.keys() {
                    let line = format!("{}: {}", "added", added.as_str());
                    println!("\t{}", style(line).green());
                }
                for modified in diff.modified.keys() {
                    let line = format!("{}: {}", "modified", modified.as_str());
                    println!("\t{}", style(line).green());
                }
                for removed in diff.removed.keys() {
                    let line = format!("{}: {}", "deleted", removed.as_str());
                    println!("\t{}", style(line).green());
                }
                true
            };

            let diff = status(&repo.odb, state, &ignore, threads, &path)?;
            if !diff.is_empty() {
                if commit_diff {
                    println!();
                }
                println!("DVC uncommitted changes:");
                for added in diff.added.keys() {
                    let line = format!("{}: {}", "added", added.as_str());
                    println!("\t{}", style(line).yellow());
                }
                for modified in diff.modified.keys() {
                    let line = format!("{}: {}", "modified", modified.as_str());
                    println!("\t{}", style(line).yellow());
                }
                for removed in diff.removed.keys() {
                    let line = format!("{}: {}", "deleted", removed.as_str());
                    println!("\t{}", style(line).yellow());
                }
            }
            Ok(())
        }
    }
}
