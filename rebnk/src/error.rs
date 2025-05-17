use thiserror::Error;
use std::io;

#[derive(Error, Debug)]
pub enum BnkError {
    #[error("Invalid file magic number")]
    InvalidMagic,

    #[error("Unsupported version: {0}")]
    UnsupportedVersion(u32),

    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    #[error("Section {0} not found")]
    SectionNotFound(&'static str),

    #[error("Invalid section size for {0}")]
    InvalidSectionSize(&'static str),

    #[error("Parse error in {0}: {1}")]
    ParseError(&'static str, String),

    #[error("Checksum mismatch")]
    ChecksumMismatch,
    
    #[error("Invalid ID requested")]
    InvalidId,
}

pub type BnkResult<T> = Result<T, BnkError>;