pub mod build;
pub mod checkout;
pub mod fsutils;
pub mod hash;
pub mod ignore;
pub mod ignorelist;
pub mod models;
pub mod objects;
pub mod odb;
pub mod repo;
pub mod state;
pub mod transfer;

pub use build::build;
pub use checkout::{checkout, checkout_obj};
pub use models::{DvcFile, Output};
pub use objects::{Object, Tree};

pub use transfer::transfer;

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
