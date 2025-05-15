use std::io;
use crate::{error::BnkResult, utils::*};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::error::BnkError;

#[derive(Debug)]
pub struct BnkHeader {
    pub magic: [u8; 4],
    pub size: u32,
    pub version: u32,
    pub soundbank_id: u32,
    pub language_id: u32,
}

impl BnkHeader {
    pub fn parse<R: io::Read + io::Seek>(cursor: &mut R) -> BnkResult<Self> {
        let magic = read_fourcc(cursor)?;
        if &magic != b"BKHD" {
            return Err(BnkError::InvalidMagic);
        }
        
        let size = cursor.read_u32::<LittleEndian>()?;
        let version = cursor.read_u32::<LittleEndian>()?;
        if version >= 72 {
            cursor.read_u32::<LittleEndian>()?; // Skip project ID for versions 72 and above
        }
        let soundbank_id = cursor.read_u32::<LittleEndian>()?;
        let language_id = cursor.read_u32::<LittleEndian>()?;
        
        let bytes_read = 4 + 4 + 4 +4 + if version >= 112 { 1 } else { 0 };
        for _ in bytes_read..size {
            cursor.read_u8()?;
        }
        Ok(Self {
            magic,
            size,
            version,
            soundbank_id,
            language_id,
        })
    }
}