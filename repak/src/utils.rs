#[derive(Debug, Clone)]
pub struct AesKey(pub aes::Aes256);
impl std::str::FromStr for AesKey {
    type Err = crate::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use aes::cipher::KeyInit;
        use base64::{engine::general_purpose, Engine as _};
        let try_parse = |mut bytes: Vec<_>| {
            bytes.chunks_mut(4).for_each(|c| c.reverse());
            aes::Aes256::new_from_slice(&bytes).ok().map(AesKey)
        };
        hex::decode(s.strip_prefix("0x").unwrap_or(s))
            .ok()
            .and_then(try_parse)
            .or_else(|| {
                general_purpose::STANDARD_NO_PAD
                    .decode(s.trim_end_matches('='))
                    .ok()
                    .and_then(try_parse)
            })
            .ok_or(crate::Error::Aes)
    }
}
