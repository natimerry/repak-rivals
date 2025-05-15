use crate::{
    error::BnkResult,
    header::BnkHeader,
};
use std::io::Cursor;

#[derive(Debug)]
pub struct BnkEntry {
    pub header: BnkHeader,
}

impl BnkEntry {
    pub fn from_file(path: &str) -> BnkResult<Self> {
        let data = std::fs::read(path)?;
        Self::parse(&data)
    }

    pub fn parse(data: &[u8]) -> BnkResult<Self> {
        let mut cursor = Cursor::new(data);
        let header = BnkHeader::parse(&mut cursor)?;
        Ok(BnkEntry {
            header
        })
    }
}