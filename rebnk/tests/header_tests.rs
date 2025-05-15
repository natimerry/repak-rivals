use std::io::Read;
use rebnk::{BnkHeader};
use std::path::Path;
#[test]
fn test_read_header_from_file() {
    // This is an example path - replace with your actual test file path
    let test_file = Path::new("test_files/example.bnk");
    if !test_file.exists() {
        eprintln!("Note: Skipping file-based header test - test file not found at {:?}", test_file);
        return;
    }
    let mut file = std::fs::File::open(test_file).unwrap();
    let mut buffer = vec![0u8; 256];
    let bytes_read = file.read(&mut buffer).unwrap();
    let buffer = &buffer[..bytes_read];

    match BnkHeader::parse(&mut std::io::Cursor::new(buffer)) {
        Ok(header) => {
            println!("Successfully parsed header from file:");
            println!("Magic: {:02X?}, {}", header.magic, String::from_utf8_lossy(&header.magic));
            println!("Size: {} bytes", header.size);
            println!("Version: {}", header.version);
            println!("SoundBank ID: {}", header.soundbank_id);
            println!("Language ID: {}", header.language_id);
        },
        Err(e) => panic!("Failed to parse header from file: {}", e),
    }
}