use crate::error::BnkResult;
use crate::sections::header::BnkHeader;
use crate::utils::read_magic;
use std::io;
use std::io::Seek;

#[derive(Debug)]
pub struct WwiseReader {
    pub header: BnkHeader,
    index_offset: usize,
    data_offset: usize,
    hirc_offset: usize,
}

impl WwiseReader {
    pub fn new<R: io::Read + io::Seek>(reader: &mut R) -> BnkResult<WwiseReader> {
        // Read the first magic 4 bytes to determine the type of section
        let file_size = reader.seek(io::SeekFrom::End(0))?;

        let mut current_offset = reader.seek(io::SeekFrom::Start(0))?;
        let mut header: Option<BnkHeader> = None;

        let mut index_offset = 0;
        let mut datas_offset = 0;
        let mut hirc_offset = 0;

        while current_offset < file_size {
            let magic_number = read_magic(reader).expect("Failed to read magic number");
            
            match magic_number {
                [0x42, 0x4B, 0x48, 0x44] => {
                    header = Some(BnkHeader::read_header(reader)?);
                }
                [0x44, 0x49, 0x44, 0x58] => {
                    index_offset = current_offset;
                }

                [0x44, 0x41, 0x54, 0x41] => {
                    datas_offset = current_offset;
                }
                [0x48, 0x49, 0x52, 0x43] => {
                    hirc_offset = current_offset;
                }
                _ => {
                    // Right now we just continue since DIDX data and HIRC hasnt gotten implemented yet
                    break;
                }
            }
            current_offset = reader.seek(io::SeekFrom::Current(0)).unwrap();
        }
        Ok(Self {
            header: header.unwrap(),
            index_offset: index_offset as usize,
            data_offset: datas_offset as usize,
            hirc_offset: hirc_offset as usize,
        })
    }
    
}
