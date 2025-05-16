use std::io::{Read, Seek};
use crate::error::BnkResult;
use byteorder::{LittleEndian, ReadBytesExt};
use crate::BnkError;

#[derive(Debug)]
pub struct BnkDataIndex {
    pub entries: Vec<SoundEntry>,
    pub size: u32,
}

#[derive(Debug)]
pub struct SoundEntry {
    pub id: u32,
    pub offset: u32,
}

impl BnkDataIndex {
    pub fn read_didx<R: Read + Seek>(cursor: &mut R) -> BnkResult<Self> {
        let size = cursor.read_u32::<LittleEndian>()?;
        if size % 12 != 0 {
            return Err(BnkError::InvalidSectionSize("DIDX"))
        }

        let entry_count = size / 12;
        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            let id = cursor.read_u32::<LittleEndian>()?;
            let offset = cursor.read_u32::<LittleEndian>()?;
            entries.push(SoundEntry {
                id,
                offset,
            });
        }
        Ok(BnkDataIndex { entries,size })
    }
}