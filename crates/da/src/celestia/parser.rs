use base64::{Engine as _, engine::general_purpose};
use celestia_types::Blob;
use eyre::Result;

use crate::types::BlockData;

pub struct CelestiaBlob {
    pub twine_block_hash: String,
    pub commitment_hash: String,
    pub height: u64,
    pub namespace: String,
}

pub fn parse_blob(height: u64, blob: &Blob) -> Result<Vec<CelestiaBlob>> {
    let commitment_hash: String = general_purpose::STANDARD.encode(blob.commitment.hash());
    let data = blob.data.as_ref();

    let blocks = serde_json::from_slice::<Vec<BlockData>>(data)?;

    let mut blobs: Vec<CelestiaBlob> = Vec::new();

    for block in blocks {
        let _ = block.number;

        //TODO: Find block hash from the number

        let blob = CelestiaBlob {
            commitment_hash: commitment_hash.clone(),
            twine_block_hash: commitment_hash.clone(),
            height,
            namespace: String::from_utf8_lossy(blob.namespace.as_ref()).to_string(),
        };

        blobs.push(blob);
    }
    Ok(blobs)
}
