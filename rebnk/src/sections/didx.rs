use crate::error::BnkResult;
use crate::BnkError;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Read, Seek};

#[derive(Debug)]
pub struct BnkDataIndex {
    pub entries: Vec<WemEntry>,
    pub size: u32,
}

#[derive(Debug)]
pub struct WemEntry {
    pub id: u32,
    pub offset: u32,
    pub wem_length: u32,
}

impl BnkDataIndex {
    pub fn read_didx<R: Read + Seek>(cursor: &mut R) -> BnkResult<Self> {
        let section_length = cursor.read_u32::<LittleEndian>()?;

        if section_length % 12 != 0 {
            return Err(BnkError::InvalidSectionSize("DIDX"))
        }
        
        let entry_count = section_length / 12;
        let mut entries = Vec::with_capacity(entry_count as usize);

        for _ in 0..entry_count {
            let id = cursor.read_u32::<LittleEndian>()?;
            let offset = cursor.read_u32::<LittleEndian>()?;
            let wem_length = cursor.read_u32::<LittleEndian>()?;

            entries.push(WemEntry {
                id,
                offset,
                wem_length,
            });
        }
        Ok(BnkDataIndex {
            entries,
            size: section_length,
        })
    }
}
