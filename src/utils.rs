use std::io::Read;

pub fn decompress_data(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut decoder = flate2::read::ZlibDecoder::new(data);
    let mut extracted = Vec::new();
    decoder.read_to_end(&mut extracted)?;
    Ok(extracted)
}


