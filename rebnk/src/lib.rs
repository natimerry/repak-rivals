extern crate core;

pub mod error;
pub mod entry;
pub mod header;
pub mod utils;

pub use error::{BnkError, BnkResult};
pub use entry::BnkEntry;
pub use header::BnkHeader;
pub fn read_bnk_file(path: &str) -> BnkResult<BnkEntry> {
    BnkEntry::from_file(path)
}