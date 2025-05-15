use std::io;
use crate::{
    error::BnkResult,
    header::BnkHeader,
};

#[derive(Debug)]
pub struct BnkEntry {
    pub header: BnkHeader,
}

impl BnkEntry {
    
    pub fn parse<R: io::Read + io::Seek>(reader: &mut R) -> BnkResult<Self> {
        let header = BnkHeader::parse(reader)?;
        Ok(BnkEntry {
            header
        })
    }
}