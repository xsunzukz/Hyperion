use crate::error::{AppError, Result};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};

pub const CHUNK_HEADER_MAGIC: u32 = 0xB1FE3AA2;

#[derive(Debug, Clone)]
pub struct Chunk {
    pub header_version: u32,
    pub header_size: u32,
    pub compressed_size: u32,
    pub guid: [u32; 4],
    pub hash: u64,
    pub stored_as: u8,
    pub sha_hash: [u8; 20],
    pub hash_type: u8,
    pub uncompressed_size: u32,
    pub secret_guid: Option<[u32; 4]>,
    pub encryption_tag: Option<[u8; 16]>,
    pub data: Vec<u8>,
}

impl Chunk {
    pub fn guid_str(&self) -> String {
        format!(
            "{:08x}-{:08x}-{:08x}-{:08x}",
            self.guid[0], self.guid[1], self.guid[2], self.guid[3]
        )
    }

    pub fn is_compressed(&self) -> bool {
        (self.stored_as & 0x01) != 0
    }

    pub fn is_encrypted(&self) -> bool {
        (self.stored_as & 0x02) != 0
    }

    pub fn parse(bytes: &[u8], secrets: Option<&HashMap<String, String>>) -> Result<Self> {
        let mut reader = Cursor::new(bytes);

        let magic = reader
            .read_u32::<LittleEndian>()
            .map_err(|e| AppError::Chunk(format!("Failed to read chunk magic: {}", e)))?;
        if magic != CHUNK_HEADER_MAGIC {
            return Err(AppError::Chunk(format!(
                "Invalid chunk header magic: 0x{:08X}",
                magic
            )));
        }

        let header_version = reader.read_u32::<LittleEndian>()?;
        let header_size = reader.read_u32::<LittleEndian>()?;
        let compressed_size = reader.read_u32::<LittleEndian>()?;

        let mut guid = [0u32; 4];
        for g in &mut guid {
            *g = reader.read_u32::<LittleEndian>()?;
        }

        let hash = reader.read_u64::<LittleEndian>()?;
        let stored_as = reader.read_u8()?;

        let mut sha_hash = [0u8; 20];
        let mut hash_type = 0u8;
        if header_version >= 2 {
            reader.read_exact(&mut sha_hash)?;
            hash_type = reader.read_u8()?;
        }

        let mut uncompressed_size = 1024 * 1024;
        if header_version >= 3 {
            uncompressed_size = reader.read_u32::<LittleEndian>()?;
        }

        let mut secret_guid = None;
        let mut encryption_tag = None;
        if header_version >= 4 {
            let mut sg = [0u32; 4];
            for g in &mut sg {
                *g = reader.read_u32::<LittleEndian>()?;
            }
            secret_guid = Some(sg);

            let mut tag = [0u8; 16];
            reader.read_exact(&mut tag)?;
            encryption_tag = Some(tag);
        }

        // Seek past header
        reader.seek(SeekFrom::Start(header_size as u64))?;

        let mut raw_data = Vec::with_capacity(compressed_size as usize);
        reader.read_to_end(&mut raw_data)?;

        // Decrypt if necessary
        let decrypted_data = if (stored_as & 0x02) != 0 {
            if let (Some(sg), Some(tag), Some(sec)) = (secret_guid, encryption_tag, secrets) {
                let secret_key_str =
                    format!("{:08X}{:08X}{:08X}{:08X}", sg[0], sg[1], sg[2], sg[3]);
                let hex_key = sec.get(&secret_key_str).ok_or_else(|| {
                    AppError::Chunk(format!(
                        "Missing decryption key for chunk secret GUID: {}",
                        secret_key_str
                    ))
                })?;
                let key_bytes = hex::decode(hex_key)
                    .map_err(|e| AppError::Chunk(format!("Invalid hex key: {}", e)))?;

                let cipher = Aes256Gcm::new_from_slice(&key_bytes)
                    .map_err(|e| AppError::Chunk(format!("AES cipher error: {}", e)))?;
                let nonce = Nonce::from_slice(&sha_hash[0..12]);

                let mut payload = raw_data;
                payload.extend_from_slice(&tag);

                cipher
                    .decrypt(nonce, payload.as_ref())
                    .map_err(|e| AppError::Chunk(format!("Decryption failed: {}", e)))?
            } else {
                return Err(AppError::Chunk(
                    "Chunk is encrypted but no decryption parameters provided".to_string(),
                ));
            }
        } else {
            raw_data
        };

        // Decompress if compressed
        let decompressed = if (stored_as & 0x01) != 0 {
            let mut decoder = ZlibDecoder::new(&decrypted_data[..]);
            let mut out = Vec::with_capacity(uncompressed_size as usize);
            decoder
                .read_to_end(&mut out)
                .map_err(|e| AppError::Chunk(format!("Failed to decompress chunk: {}", e)))?;
            out
        } else {
            decrypted_data
        };

        Ok(Self {
            header_version,
            header_size,
            compressed_size,
            guid,
            hash,
            stored_as,
            sha_hash,
            hash_type,
            uncompressed_size,
            secret_guid,
            encryption_tag,
            data: decompressed,
        })
    }

    /// Creates and serializes a new Chunk from uncompressed payload (used for cloud saves)
    pub fn create_from_data(guid: [u32; 4], data: &[u8]) -> Result<Vec<u8>> {
        let uncompressed_size = data.len() as u32;

        let mut hasher = Sha1::new();
        hasher.update(data);
        let sha_hash: [u8; 20] = hasher.finalize().into();

        // Calculate rolling hash
        let hash = compute_rolling_hash(data);

        // Compress data
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data)?;
        let compressed_data = encoder.finish()?;
        let compressed_size = compressed_data.len() as u32;

        let header_version = 3u32;
        let header_size = 66u32;

        let mut out = Vec::with_capacity(header_size as usize + compressed_data.len());
        out.write_u32::<LittleEndian>(CHUNK_HEADER_MAGIC)?;
        out.write_u32::<LittleEndian>(header_version)?;
        out.write_u32::<LittleEndian>(header_size)?;
        out.write_u32::<LittleEndian>(compressed_size)?;
        for g in guid {
            out.write_u32::<LittleEndian>(g)?;
        }
        out.write_u64::<LittleEndian>(hash)?;
        out.write_u8(0x01)?; // zlib compressed
        out.write_all(&sha_hash)?;
        out.write_u8(0x03)?; // rolling + sha hash
        out.write_u32::<LittleEndian>(uncompressed_size)?;
        out.write_all(&compressed_data)?;

        Ok(out)
    }
}

pub fn compute_rolling_hash(data: &[u8]) -> u64 {
    // Epic rolling hash approximation or Adler-64/Murmur variant
    let mut a: u64 = 0;
    let mut b: u64 = 0;
    for &byte in data {
        a = (a + byte as u64) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}
