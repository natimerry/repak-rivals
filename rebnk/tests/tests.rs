use std::path::Path;
use rebnk::wwise_reader::WwiseReader;

#[test]
fn test_read_header_from_file() {
    // This is an example path - replace with your actual test file path
    let test_file = Path::new("test_files/example.bnk");
    if !test_file.exists() {
        eprintln!("Note: Skipping file-based header test - test file not found at {:?}", test_file);
        return;
    }

    println!("Reading header from file: {:?}", test_file);
    let mut file = std::fs::File::open(test_file).unwrap();
    // let mut buffer = vec![0u8; 256];
    // let bytes_read = file.read(&mut buffer).unwrap();
    // let buffer = &buffer[..bytes_read];

    let reader = WwiseReader::new(&mut file);
    let header = reader.unwrap().header;
    println!("Successfully parsed header from file:");
    println!("Size: {} bytes", header.size);
    println!("Version: {}", header.version);
    println!("SoundBank ID: {}", header.soundbank_id);
    println!("Language ID: {}", header.language_id);
    assert_eq!(header.size, 40);
    assert_eq!(header.version, 145);
    assert_eq!(header.soundbank_id, 684519430);
    assert_eq!(header.language_id, 16);
}

#[test]
fn test_read_didx_from_file() {
    let test_file = Path::new("test_files/example.bnk");
    if !test_file.exists() {
        eprintln!("Note: Skipping file-based didx test - test file not found at {:?}", test_file);
        return;
    }

    println!("Reading didx from file: {:?}", test_file);
    let mut file = std::fs::File::open(test_file).unwrap();
    
    let reader = WwiseReader::new(&mut file);
    let didx = reader.unwrap().didx;
    println!("Successfully parsed didx from file:");
    println!("Size: {} entries", didx.size);
    println!("{:?}", didx.entries);
}