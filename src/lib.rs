use std::env;
use std::error::Error;
pub mod build;
pub mod checkout;
pub mod fsutils;
pub mod hash;
pub mod models;
pub mod objects;
pub mod odb;
pub mod repo;
pub mod transfer;
pub mod state;
use odb::Odb;

use repo::Repo;

pub use build::build;
pub use checkout::{checkout, checkout_obj};
pub use models::{DvcFile, Output};
pub use objects::{Object, Tree};
use state::State;
pub use transfer::transfer;

pub fn get_odb() -> Result<(Odb, State), Box<dyn Error>> {
    let repo = Repo {
        root: env::current_dir()?,
    };
    let state_path = repo.tmp_dir().join("hashes/local/cache.db");
    Ok((Odb { path: repo.object_dir()}, State::open(&state_path)?.instantiate()?))
}

pub fn create_pool(num: Option<usize>) -> usize {
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
