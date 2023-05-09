#![warn(clippy::perf, clippy::pedantic)]
#![allow(
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::return_self_not_must_use
)]

pub mod build;
pub mod checkout;
pub mod config;
pub mod diff;
pub mod fsutils;
pub mod hash;
pub mod ignore;
pub mod ignorelist;
pub mod models;
pub mod objects;
pub mod odb;
pub mod repo;
pub mod state;
pub mod status;
pub mod transfer;

pub use build::build;
pub use checkout::{checkout, checkout_obj};
pub use models::{DvcFile, Output};
pub use objects::{Object, Tree};

pub use transfer::transfer;

pub fn create_pool(num: Option<usize>) -> usize {
    let threads = num.unwrap_or_else(num_cpus::get_physical);
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .unwrap();
    threads
}
