use crate::error::BnkResult;
pub fn read_data(data: &[u8]) -> BnkResult<Vec<u8>> {
    // DATA section is just raw binary data
    Ok(data.to_vec())
}