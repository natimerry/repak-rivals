use crate::error::BnkResult;
use crate::sections::header::BnkHeader;
use crate::utils::read_magic;
use std::io;
use std::io::{Seek, SeekFrom};
use crate::BnkError;
use crate::sections::data::{read_data, read_wem};
use crate::sections::didx::BnkDataIndex;

#[derive(Debug)]
pub struct WwiseReader {
    pub header: BnkHeader,
    data_offset: usize,
    hirc_offset: usize,
    pub didx: BnkDataIndex,
}

impl WwiseReader {
    pub fn new<R: io::Read + io::Seek>(reader: &mut R) -> BnkResult<WwiseReader> {
        // Read the first magic 4 bytes to determine the type of section
        let file_size = reader.seek(io::SeekFrom::End(0))?;

        let mut current_offset = reader.seek(io::SeekFrom::Start(0))?;
        let mut header: Option<BnkHeader> = None;
        let mut didx: Option<BnkDataIndex> = None;
        let mut datas_offset = 0;
        let mut hirc_offset = 0;

        while current_offset < file_size {
            let magic_number = read_magic(reader).expect("Failed to read magic number");

            match magic_number {
                [0x42, 0x4B, 0x48, 0x44] => {
                    header = Some(BnkHeader::read_header(reader)?);
                }
                [0x44, 0x49, 0x44, 0x58] => {
                    didx = Some(BnkDataIndex::read_didx(reader)?);
                }

                [0x44, 0x41, 0x54, 0x41] => {
                    // We dont load the data section into memory at all.
                    // The user needs to pass in a Reader for us to start parsing the data section
                    datas_offset = current_offset;
                    let _ = read_data(reader); // This just seeks forward to HIRC section
                }
                [0x48, 0x49, 0x52, 0x43] => {
                    hirc_offset = current_offset;
                }
                _ => {
                    // Right now we just continue since DIDX data and HIRC hasnt gotten implemented yet
                    break;
                }
            }
            current_offset = reader.seek(io::SeekFrom::Current(0))?;
        }
        Ok(Self {
            header: header.unwrap(),
            didx: didx.unwrap(),
            data_offset: datas_offset as usize,
            hirc_offset: hirc_offset as usize,
        })
    }

    pub fn get_wem_data<R: io::Read + io::Seek>(&self,id: u32,reader: &mut R) -> BnkResult<Vec<u8>> {
        let wem_entry = self.didx.entries.get(&id);
        if wem_entry.is_none() {
            return Err(BnkError::InvalidId)
        }

        let wem_entry = wem_entry.unwrap();
        let wem_offset = wem_entry.offset as usize; // this is offset from start of data
        // seek to data_offset + wem_offset + data_magic_len + data_section_len
        reader.seek(SeekFrom::Start((self.data_offset + wem_offset + 8) as u64))?;

        read_wem(wem_entry.wem_length as usize, reader)
    }
}
