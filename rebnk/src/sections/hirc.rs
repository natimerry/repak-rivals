use std::io;
use std::io::Read;
use crate::{BnkError, BnkResult};

pub fn copy_hirc<R:io::Read + io::Seek>(size: usize, reader: &mut R) -> BnkResult<Vec<u8>> {
    let mut data = vec![0; size];
    reader.read_exact(&mut data)?;
    Ok(data)
}

enum HIRCSection {
    Settings = 0x1,
    Sound = 0x2,
    EventAction = 0x3,
    Event = 0x4,
    RandomContainer = 0x5,
    SwitchContainer = 0x6,
    ActorMixer = 0x7,
    AudioBus = 0x8,
    BlendContainer = 0x9,
    MusicSegment = 0xa,
    MusicTrack = 0xb,
    MusicSwitchContainer = 0xc,
    MusicPlaylist = 0xd,
}
/// Returns a list of known sections in HIRC format.
pub fn parse_hirc_sections(data: &[u8]) -> BnkResult<()> {
    todo!()
}