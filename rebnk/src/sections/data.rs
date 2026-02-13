use std::io;
use byteorder::{LittleEndian, ReadBytesExt};
use crate::BnkError;
use crate::error::BnkResult;
pub fn read_wem<R:io::Read + io::Seek>(size: usize, reader: &mut R) -> BnkResult<Vec<u8>> {
    let mut data = vec![0; size];

    reader.read_exact(&mut data)?;

    // The WEM section must start with "RIFF"
    if !(data[0..=3] == *"RIFF".as_bytes()){
        return Err(BnkError::ParseError("Data section", "WEM SECTION MUST START WITH RIFF".parse().unwrap()))
    }
    Ok(data)
}

pub fn read_data<R:io::Read + io::Seek>(reader: &mut R) -> BnkResult<()> {
    let size = reader.read_u32::<LittleEndian>()?;
    // just seek forward by size
    reader.seek(io::SeekFrom::Current(size as i64))?;
    Ok(())
}