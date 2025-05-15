use std::io;
use crate::error::{BnkResult};
use byteorder::{ReadBytesExt};
use std::io::{Cursor};

pub fn read_fourcc<R: io::Read + io::Seek>(cursor: &mut R) -> BnkResult<[u8; 4]> {
    let mut fourcc = [0u8; 4];
    cursor.read_exact(&mut fourcc)?;
    Ok(fourcc)
}

pub fn read_null_terminated_string(cursor: &mut Cursor<&[u8]>) -> BnkResult<String> {
    let mut bytes = Vec::new();
    loop {
        let byte = cursor.read_u8()?;
        if byte == 0 {
            break;
        }
        bytes.push(byte);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub fn align_cursor(cursor: &mut Cursor<&[u8]>, alignment: u64) -> BnkResult<()> {
    let pos = cursor.position();
    let rem = pos % alignment;
    if rem != 0 {
        cursor.set_position(pos + (alignment - rem));
    }
    Ok(())
}