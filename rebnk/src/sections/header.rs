use std::io;
use crate::error::BnkResult;
use byteorder::{LittleEndian, ReadBytesExt};

#[derive(Debug)]
pub struct BnkHeader {
    pub size: u32,
    pub version: u32,
    pub soundbank_id: u32,
    pub language_id: u32,
    unknown_data: Vec<u8>,
}

impl BnkHeader {
    pub fn read_header<R: io::Read + io::Seek>(cursor: &mut R) -> BnkResult<Self> {
        let size = cursor.read_u32::<LittleEndian>()?;
        let version = cursor.read_u32::<LittleEndian>()?;
        if version >= 72 {
            cursor.read_u32::<LittleEndian>()?; // Skip project ID for versions 72 and above
        }
        let soundbank_id = cursor.read_u32::<LittleEndian>()?;
        let language_id = cursor.read_u32::<LittleEndian>()?;

        let bytes_read = 16;
        let mut final_bytes = vec![0u8; size as usize - bytes_read];
        cursor.read_exact(&mut final_bytes)?;
        Ok(Self {
            size,
            version,
            soundbank_id,
            language_id,
            unknown_data: final_bytes,
        })
    }
}