use crate::error::{AppError, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use flate2::read::{GzDecoder, ZlibDecoder};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};

pub const MANIFEST_HEADER_MAGIC: u32 = 0x44BEC00C;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub header_size: u32,
    pub size_uncompressed: u32,
    pub size_compressed: u32,
    pub sha_hash: [u8; 20],
    pub stored_as: u8,
    pub version: u32,
    pub meta: ManifestMeta,
    pub chunk_data_list: Vec<ChunkInfo>,
    pub file_manifest_list: Vec<FileManifest>,
    pub custom_fields: HashMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManifestMeta {
    pub meta_size: u32,
    pub data_version: u8,
    pub feature_level: u32,
    pub is_file_data: bool,
    pub app_id: u32,
    pub app_name: String,
    pub build_version: String,
    pub launch_exe: String,
    pub launch_command: String,
    pub prereq_ids: Vec<String>,
    pub prereq_name: String,
    pub prereq_path: String,
    pub prereq_args: String,
    pub build_id: String,
    pub uninstall_action_path: String,
    pub uninstall_action_args: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkInfo {
    pub guid: [u32; 4],
    pub hash: u64,
    pub sha_hash: [u8; 20],
    pub group_num: u8,
    pub window_size: u32,
    pub file_size: i64,
    pub secret_guid: Option<[u32; 4]>,
    pub window_size_compressed: Option<u32>,
    pub encryption_tag: Option<[u8; 16]>,
}

impl ChunkInfo {
    pub fn guid_str(&self) -> String {
        format!(
            "{:08x}-{:08x}-{:08x}-{:08x}",
            self.guid[0], self.guid[1], self.guid[2], self.guid[3]
        )
    }

    pub fn guid_hex(&self) -> String {
        format!(
            "{:08X}{:08X}{:08X}{:08X}",
            self.guid[0], self.guid[1], self.guid[2], self.guid[3]
        )
    }

    pub fn path(&self, manifest_version: u32) -> String {
        let chunk_dir = if manifest_version >= 22 {
            "ChunksV5"
        } else if manifest_version >= 15 {
            "ChunksV4"
        } else if manifest_version >= 6 {
            "ChunksV3"
        } else if manifest_version >= 3 {
            "ChunksV2"
        } else {
            "Chunks"
        };

        if manifest_version >= 22 {
            use base64::Engine;
            let secret_part = if let Some(sg) = self.secret_guid {
                if sg == [0, 0, 0, 0] {
                    "plain".to_string()
                } else {
                    let mut b = [0u8; 16];
                    let mut cur = Cursor::new(&mut b[..]);
                    for g in sg {
                        let _ = cur.write_u32::<LittleEndian>(g);
                    }
                    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
                }
            } else {
                "plain".to_string()
            };

            let mut hash_b = [0u8; 8];
            Cursor::new(&mut hash_b[..])
                .write_u64::<LittleEndian>(self.hash)
                .unwrap();
            let hash_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash_b);

            let mut guid_b = [0u8; 16];
            let mut cur = Cursor::new(&mut guid_b[..]);
            for g in self.guid {
                cur.write_u32::<LittleEndian>(g).unwrap();
            }
            let guid_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(guid_b);

            format!(
                "{}/{}/{:02}/{}_{}.chunk",
                chunk_dir, secret_part, self.group_num, hash_b64, guid_b64
            )
        } else {
            format!(
                "{}/{:02}/{:016X}_{}.chunk",
                chunk_dir,
                self.group_num,
                self.hash,
                self.guid_hex()
            )
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkPart {
    pub guid: [u32; 4],
    pub offset: u32,
    pub size: u32,
    pub file_offset: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileManifest {
    pub filename: String,
    pub symlink_target: String,
    pub hash: [u8; 20],
    pub flags: u8,
    pub install_tags: Vec<String>,
    pub chunk_parts: Vec<ChunkPart>,
    pub file_size: u64,
    pub mime_type: Option<String>,
    pub hash_md5: Option<[u8; 16]>,
    pub hash_sha256: Option<[u8; 32]>,
}

impl FileManifest {
    pub fn is_executable(&self) -> bool {
        (self.flags & 0x04) != 0
    }

    pub fn is_read_only(&self) -> bool {
        (self.flags & 0x01) != 0
    }
}

pub fn read_fstring<R: Read>(reader: &mut R) -> Result<String> {
    let length = reader
        .read_i32::<LittleEndian>()
        .map_err(|e| AppError::Manifest(format!("Failed to read fstring length: {}", e)))?;

    if length < 0 {
        let char_count = (-length) as usize;
        let byte_count = char_count * 2;
        let mut buf = vec![0u8; byte_count];
        reader
            .read_exact(&mut buf)
            .map_err(|e| AppError::Manifest(format!("Failed to read UTF-16 fstring: {}", e)))?;

        // Exclude null terminator
        let u16_chars: Vec<u16> = buf
            .chunks_exact(2)
            .take(char_count.saturating_sub(1))
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();

        String::from_utf16(&u16_chars)
            .map_err(|e| AppError::Manifest(format!("Invalid UTF-16 in fstring: {}", e)))
    } else if length > 0 {
        let byte_count = length as usize;
        let mut buf = vec![0u8; byte_count];
        reader
            .read_exact(&mut buf)
            .map_err(|e| AppError::Manifest(format!("Failed to read ASCII fstring: {}", e)))?;

        // Exclude trailing null byte
        if let Some(pos) = buf.iter().position(|&b| b == 0) {
            buf.truncate(pos);
        }
        String::from_utf8(buf)
            .map_err(|e| AppError::Manifest(format!("Invalid UTF-8 in fstring: {}", e)))
    } else {
        Ok(String::new())
    }
}

pub fn write_fstring<W: Write>(writer: &mut W, text: &str) -> Result<()> {
    if text.is_empty() {
        writer
            .write_i32::<LittleEndian>(0)
            .map_err(|e| AppError::Manifest(e.to_string()))?;
        return Ok(());
    }

    let is_ascii = text.bytes().all(|b| b < 128);
    if is_ascii {
        let len = (text.len() + 1) as i32;
        writer
            .write_i32::<LittleEndian>(len)
            .map_err(|e| AppError::Manifest(e.to_string()))?;
        writer
            .write_all(text.as_bytes())
            .map_err(|e| AppError::Manifest(e.to_string()))?;
        writer
            .write_all(&[0u8])
            .map_err(|e| AppError::Manifest(e.to_string()))?;
    } else {
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let len = -((utf16.len() + 1) as i32);
        writer
            .write_i32::<LittleEndian>(len)
            .map_err(|e| AppError::Manifest(e.to_string()))?;
        for c in utf16 {
            writer
                .write_u16::<LittleEndian>(c)
                .map_err(|e| AppError::Manifest(e.to_string()))?;
        }
        writer
            .write_all(&[0u8, 0u8])
            .map_err(|e| AppError::Manifest(e.to_string()))?;
    }
    Ok(())
}

/// Helper: Converts GUID representations (hex string, hyphenated UUID, or 4-element u32 array) to [u32; 4]
fn parse_guid_from_value(val: &serde_json::Value) -> [u32; 4] {
    if let Some(s) = val.as_str() {
        let clean: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        if clean.len() >= 32 {
            let g0 = u32::from_str_radix(&clean[0..8], 16).unwrap_or(0);
            let g1 = u32::from_str_radix(&clean[8..16], 16).unwrap_or(0);
            let g2 = u32::from_str_radix(&clean[16..24], 16).unwrap_or(0);
            let g3 = u32::from_str_radix(&clean[24..32], 16).unwrap_or(0);
            return [g0, g1, g2, g3];
        }
    } else if let Some(arr) = val.as_array() {
        if arr.len() == 4 {
            let g0 = arr[0].as_u64().unwrap_or(0) as u32;
            let g1 = arr[1].as_u64().unwrap_or(0) as u32;
            let g2 = arr[2].as_u64().unwrap_or(0) as u32;
            let g3 = arr[3].as_u64().unwrap_or(0) as u32;
            return [g0, g1, g2, g3];
        }
    }
    [0, 0, 0, 0]
}

/// Helper: Parses Epic %03d decimal triplet strings, standard integers, or string numbers into u64
fn blob_to_u64(val: &serde_json::Value) -> u64 {
    match val {
        serde_json::Value::Number(n) => n.as_u64().unwrap_or(0),
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                return 0;
            }
            if s.len() % 3 == 0 && s.len() >= 3 && s.chars().all(|c| c.is_ascii_digit()) {
                let mut num: u64 = 0;
                let mut shift = 0;
                let chars: Vec<char> = s.chars().collect();
                for chunk in chars.chunks(3) {
                    if shift >= 64 {
                        break;
                    }
                    let triplet: String = chunk.iter().collect();
                    if let Ok(b) = triplet.parse::<u64>() {
                        num |= (b & 0xFF) << shift;
                    }
                    shift += 8;
                }
                num
            } else {
                s.parse::<u64>().unwrap_or(0)
            }
        }
        _ => 0,
    }
}

/// Helper: Parses hash values from hex strings, %03d decimal triplet blobs, or byte arrays
fn parse_hash_from_value(val: &serde_json::Value, expected_len: usize) -> Vec<u8> {
    match val {
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s.len() == expected_len * 2 && s.chars().all(|c| c.is_ascii_hexdigit()) {
                if let Ok(b) = hex::decode(s) {
                    return b;
                }
            }
            if s.len() == expected_len * 3 && s.chars().all(|c| c.is_ascii_digit()) {
                let mut bytes = Vec::with_capacity(expected_len);
                let chars: Vec<char> = s.chars().collect();
                for chunk in chars.chunks(3) {
                    let triplet: String = chunk.iter().collect();
                    if let Ok(b) = triplet.parse::<u8>() {
                        bytes.push(b);
                    }
                }
                if bytes.len() == expected_len {
                    return bytes;
                }
            }
            if let Ok(b) = hex::decode(s) {
                let mut res = b;
                res.resize(expected_len, 0);
                return res;
            }
            vec![0u8; expected_len]
        }
        serde_json::Value::Array(arr) => {
            let mut bytes = Vec::with_capacity(expected_len);
            for v in arr {
                bytes.push(v.as_u64().unwrap_or(0) as u8);
            }
            bytes.resize(expected_len, 0);
            bytes
        }
        _ => vec![0u8; expected_len],
    }
}

/// Helper: Case-insensitive map lookup
fn get_by_key_ci<'a>(
    map: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<&'a serde_json::Value> {
    if let Some(v) = map.get(key) {
        return Some(v);
    }
    let key_lower = key.to_lowercase();
    map.iter().find_map(|(k, v)| {
        if k.to_lowercase() == key_lower {
            Some(v)
        } else {
            None
        }
    })
}

impl Manifest {
    /// Auto-detects manifest format (Binary 0x44BEC00C, JSON text / 0x614D227B, Gzip, or Zlib)
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            return Err(AppError::Manifest("Manifest byte buffer is empty".to_string()));
        }

        // 1. Check for gzip compression (magic 0x1F, 0x8B)
        if bytes.starts_with(&[0x1F, 0x8B]) {
            let mut decoder = GzDecoder::new(bytes);
            let mut decompressed = Vec::new();
            if decoder.read_to_end(&mut decompressed).is_ok() && !decompressed.is_empty() {
                return Self::parse(&decompressed);
            }
        }

        // 2. Check for zlib compression (magic 0x78)
        if bytes.len() >= 2 && bytes[0] == 0x78 && (bytes[1] == 0x01 || bytes[1] == 0x9C || bytes[1] == 0xDA || bytes[1] == 0x5E) {
            let mut decoder = ZlibDecoder::new(bytes);
            let mut decompressed = Vec::new();
            if decoder.read_to_end(&mut decompressed).is_ok() && !decompressed.is_empty() {
                return Self::parse(&decompressed);
            }
        }

        // 3. Check for binary manifest magic: 0x44BEC00C
        if bytes.len() >= 4 {
            let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            if magic == MANIFEST_HEADER_MAGIC {
                return Self::parse_binary(bytes);
            }
        }

        // 4. Check for JSON manifest (starts with '{' or BOM + '{' or ASCII text)
        let mut slice = bytes;
        if slice.starts_with(b"\xEF\xBB\xBF") {
            slice = &slice[3..];
        }
        let trimmed = slice
            .iter()
            .position(|&b| !b.is_ascii_whitespace())
            .map(|idx| &slice[idx..])
            .unwrap_or(slice);

        if trimmed.starts_with(b"{") {
            return Self::parse_json(trimmed);
        }

        let magic = if bytes.len() >= 4 {
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        } else {
            0
        };
        Err(AppError::Manifest(format!(
            "Invalid manifest magic: 0x{:08X}",
            magic
        )))
    }

    /// Parses binary manifest (0x44BEC00C)
    pub fn parse_binary(bytes: &[u8]) -> Result<Self> {
        let mut reader = Cursor::new(bytes);

        let magic = reader
            .read_u32::<LittleEndian>()
            .map_err(|e| AppError::Manifest(format!("Failed to read magic: {}", e)))?;
        if magic != MANIFEST_HEADER_MAGIC {
            return Err(AppError::Manifest(format!(
                "Invalid manifest magic: 0x{:08X}",
                magic
            )));
        }

        let header_size = reader.read_u32::<LittleEndian>()?;
        let size_uncompressed = reader.read_u32::<LittleEndian>()?;
        let size_compressed = reader.read_u32::<LittleEndian>()?;

        let mut sha_hash = [0u8; 20];
        reader.read_exact(&mut sha_hash)?;

        let stored_as = reader.read_u8()?;
        let version = reader.read_u32::<LittleEndian>()?;

        let mut _secret_guid = [0u32; 4];
        let mut _encryption_tag = [0u8; 16];
        if version >= 22 {
            for g in &mut _secret_guid {
                *g = reader.read_u32::<LittleEndian>()?;
            }
            reader.read_exact(&mut _encryption_tag)?;
        }

        // Seek to end of header if header size specifies
        reader.seek(SeekFrom::Start(header_size as u64))?;

        let mut body_bytes = Vec::new();
        reader.read_to_end(&mut body_bytes)?;

        let decompressed = if (stored_as & 0x01) != 0 {
            let mut decoder = ZlibDecoder::new(&body_bytes[..]);
            let mut out = Vec::with_capacity(size_uncompressed as usize);
            decoder.read_to_end(&mut out).map_err(|e| {
                AppError::Manifest(format!("Failed to zlib decompress manifest body: {}", e))
            })?;
            out
        } else {
            body_bytes
        };

        // Verify SHA1 hash
        let mut hasher = Sha1::new();
        hasher.update(&decompressed);
        let calculated_hash = hasher.finalize();
        if calculated_hash.as_slice() != sha_hash {
            return Err(AppError::Manifest(
                "Manifest SHA1 hash mismatch".to_string(),
            ));
        }

        let mut body_cursor = Cursor::new(&decompressed);

        // 1. Parse ManifestMeta
        let meta_start = body_cursor.position();
        let meta_size = body_cursor.read_u32::<LittleEndian>()?;
        let data_version = body_cursor.read_u8()?;
        let feature_level = body_cursor.read_u32::<LittleEndian>()?;
        let is_file_data = body_cursor.read_u8()? != 0;
        let app_id = body_cursor.read_u32::<LittleEndian>()?;
        let app_name = read_fstring(&mut body_cursor)?;
        let build_version = read_fstring(&mut body_cursor)?;
        let launch_exe = read_fstring(&mut body_cursor)?;
        let launch_command = read_fstring(&mut body_cursor)?;

        let prereq_count = body_cursor.read_u32::<LittleEndian>()?;
        let mut prereq_ids = Vec::with_capacity(prereq_count as usize);
        for _ in 0..prereq_count {
            prereq_ids.push(read_fstring(&mut body_cursor)?);
        }

        let prereq_name = read_fstring(&mut body_cursor)?;
        let prereq_path = read_fstring(&mut body_cursor)?;
        let prereq_args = read_fstring(&mut body_cursor)?;

        let mut build_id = String::new();
        if data_version >= 1 {
            build_id = read_fstring(&mut body_cursor)?;
        }

        let mut uninstall_action_path = String::new();
        let mut uninstall_action_args = String::new();
        if data_version >= 2 {
            uninstall_action_path = read_fstring(&mut body_cursor)?;
            uninstall_action_args = read_fstring(&mut body_cursor)?;
        }

        body_cursor.seek(SeekFrom::Start(meta_start + meta_size as u64))?;

        let meta = ManifestMeta {
            meta_size,
            data_version,
            feature_level,
            is_file_data,
            app_id,
            app_name,
            build_version,
            launch_exe,
            launch_command,
            prereq_ids,
            prereq_name,
            prereq_path,
            prereq_args,
            build_id,
            uninstall_action_path,
            uninstall_action_args,
        };

        // 2. Parse CDL (Chunk Data List)
        let cdl_start = body_cursor.position();
        let cdl_size = body_cursor.read_u32::<LittleEndian>()?;
        let _cdl_version = body_cursor.read_u8()?;
        let chunk_count = body_cursor.read_u32::<LittleEndian>()? as usize;

        let mut chunks = vec![ChunkInfo::default(); chunk_count];

        for c in &mut chunks {
            for g in &mut c.guid {
                *g = body_cursor.read_u32::<LittleEndian>()?;
            }
        }
        for c in &mut chunks {
            c.hash = body_cursor.read_u64::<LittleEndian>()?;
        }
        for c in &mut chunks {
            body_cursor.read_exact(&mut c.sha_hash)?;
        }
        for c in &mut chunks {
            c.group_num = body_cursor.read_u8()?;
        }
        for c in &mut chunks {
            c.window_size = body_cursor.read_u32::<LittleEndian>()?;
        }
        for c in &mut chunks {
            c.file_size = body_cursor.read_i64::<LittleEndian>()?;
        }

        if feature_level >= 22 {
            for c in &mut chunks {
                let mut sg = [0u32; 4];
                for g in &mut sg {
                    *g = body_cursor.read_u32::<LittleEndian>()?;
                }
                c.secret_guid = Some(sg);
            }
            for c in &mut chunks {
                c.window_size_compressed = Some(body_cursor.read_u32::<LittleEndian>()?);
            }
            for c in &mut chunks {
                let mut tag = [0u8; 16];
                body_cursor.read_exact(&mut tag)?;
                c.encryption_tag = Some(tag);
            }
        }

        body_cursor.seek(SeekFrom::Start(cdl_start + cdl_size as u64))?;

        // 3. Parse FML (File Manifest List)
        let fml_start = body_cursor.position();
        let fml_size = body_cursor.read_u32::<LittleEndian>()?;
        let fml_version = body_cursor.read_u8()?;
        let file_count = body_cursor.read_u32::<LittleEndian>()? as usize;

        let mut files = vec![FileManifest::default(); file_count];

        for f in &mut files {
            f.filename = read_fstring(&mut body_cursor)?;
        }
        for f in &mut files {
            f.symlink_target = read_fstring(&mut body_cursor)?;
        }
        for f in &mut files {
            body_cursor.read_exact(&mut f.hash)?;
        }
        for f in &mut files {
            f.flags = body_cursor.read_u8()?;
        }
        for f in &mut files {
            let tag_count = body_cursor.read_u32::<LittleEndian>()?;
            let mut tags = Vec::with_capacity(tag_count as usize);
            for _ in 0..tag_count {
                tags.push(read_fstring(&mut body_cursor)?);
            }
            f.install_tags = tags;
        }

        // Chunk parts for each file
        for f in &mut files {
            let part_count = body_cursor.read_u32::<LittleEndian>()? as usize;
            let mut parts = Vec::with_capacity(part_count);
            let mut current_file_offset = 0u64;

            for _ in 0..part_count {
                let part_start = body_cursor.position();
                let declared_size = body_cursor.read_u32::<LittleEndian>()? as u64;

                let mut guid = [0u32; 4];
                for g in &mut guid {
                    *g = body_cursor.read_u32::<LittleEndian>()?;
                }
                let offset = body_cursor.read_u32::<LittleEndian>()?;
                let size = body_cursor.read_u32::<LittleEndian>()?;

                parts.push(ChunkPart {
                    guid,
                    offset,
                    size,
                    file_offset: current_file_offset,
                });
                current_file_offset += size as u64;

                let bytes_read = body_cursor.position() - part_start;
                if bytes_read < declared_size {
                    body_cursor.seek(SeekFrom::Current((declared_size - bytes_read) as i64))?;
                }
            }
            f.chunk_parts = parts;
            f.file_size = current_file_offset;
        }

        if fml_version >= 1 {
            for f in &mut files {
                let has_md5 = body_cursor.read_u32::<LittleEndian>()?;
                if has_md5 != 0 {
                    let mut md5 = [0u8; 16];
                    body_cursor.read_exact(&mut md5)?;
                    f.hash_md5 = Some(md5);
                }
            }
            for f in &mut files {
                let mime = read_fstring(&mut body_cursor)?;
                if !mime.is_empty() {
                    f.mime_type = Some(mime);
                }
            }
        }

        if fml_version >= 2 {
            for f in &mut files {
                let mut sha256 = [0u8; 32];
                body_cursor.read_exact(&mut sha256)?;
                f.hash_sha256 = Some(sha256);
            }
        }

        body_cursor.seek(SeekFrom::Start(fml_start + fml_size as u64))?;

        // 4. Parse CustomFields
        let mut custom_fields = HashMap::new();
        if body_cursor.position() < decompressed.len() as u64 {
            let cf_start = body_cursor.position();
            if let Ok(cf_size) = body_cursor.read_u32::<LittleEndian>() {
                let _cf_ver = body_cursor.read_u8().unwrap_or(0);
                let cf_count = body_cursor.read_u32::<LittleEndian>().unwrap_or(0) as usize;

                let mut keys = Vec::with_capacity(cf_count);
                for _ in 0..cf_count {
                    if let Ok(k) = read_fstring(&mut body_cursor) {
                        keys.push(k);
                    }
                }
                let mut values = Vec::with_capacity(cf_count);
                for _ in 0..cf_count {
                    if let Ok(v) = read_fstring(&mut body_cursor) {
                        values.push(v);
                    }
                }

                for (k, v) in keys.into_iter().zip(values) {
                    custom_fields.insert(k, v);
                }
                let _ = body_cursor.seek(SeekFrom::Start(cf_start + cf_size as u64));
            }
        }

        Ok(Self {
            header_size,
            size_uncompressed,
            size_compressed,
            sha_hash,
            stored_as,
            version,
            meta,
            chunk_data_list: chunks,
            file_manifest_list: files,
            custom_fields,
        })
    }

    /// Parses JSON manifest (Epic Games format or serialized Manifest format)
    pub fn parse_json(bytes: &[u8]) -> Result<Self> {
        // Fast path: try direct deserialization if it was serialized from Manifest
        if let Ok(m) = serde_json::from_slice::<Manifest>(bytes) {
            if !m.file_manifest_list.is_empty() {
                return Ok(m);
            }
        }

        let json: serde_json::Value = serde_json::from_slice(bytes)
            .map_err(|e| AppError::Manifest(format!("Failed to parse JSON manifest: {}", e)))?;

        let obj = json.as_object().ok_or_else(|| {
            AppError::Manifest("JSON manifest root is not an object".to_string())
        })?;

        // ManifestFileVersion / Version
        let manifest_version = obj
            .get("ManifestFileVersion")
            .or_else(|| obj.get("manifest_version"))
            .or_else(|| obj.get("version"))
            .map(|v| blob_to_u64(v) as u32)
            .unwrap_or(18);

        // Metadata extraction (check root or nested "meta")
        let meta_obj = obj.get("meta").and_then(|v| v.as_object()).unwrap_or(obj);

        let is_file_data = meta_obj
            .get("bIsFileData")
            .or_else(|| meta_obj.get("is_file_data"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let app_id = meta_obj
            .get("AppID")
            .or_else(|| meta_obj.get("app_id"))
            .map(|v| blob_to_u64(v) as u32)
            .unwrap_or(0);

        let app_name = meta_obj
            .get("AppNameString")
            .or_else(|| meta_obj.get("app_name"))
            .or_else(|| meta_obj.get("appName"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let build_version = meta_obj
            .get("BuildVersionString")
            .or_else(|| meta_obj.get("build_version"))
            .or_else(|| meta_obj.get("buildVersion"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let launch_exe = meta_obj
            .get("LaunchExeString")
            .or_else(|| meta_obj.get("launch_exe"))
            .or_else(|| meta_obj.get("launchExe"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let launch_command = meta_obj
            .get("LaunchCommand")
            .or_else(|| meta_obj.get("launch_command"))
            .or_else(|| meta_obj.get("launchCommand"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let prereq_ids = meta_obj
            .get("PrereqIds")
            .or_else(|| meta_obj.get("prereq_ids"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let prereq_name = meta_obj
            .get("PrereqName")
            .or_else(|| meta_obj.get("prereq_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let prereq_path = meta_obj
            .get("PrereqPath")
            .or_else(|| meta_obj.get("prereq_path"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let prereq_args = meta_obj
            .get("PrereqArgs")
            .or_else(|| meta_obj.get("prereq_args"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let build_id = meta_obj
            .get("BuildId")
            .or_else(|| meta_obj.get("build_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let uninstall_action_path = meta_obj
            .get("UninstallActionPath")
            .or_else(|| meta_obj.get("uninstall_action_path"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let uninstall_action_args = meta_obj
            .get("UninstallActionArgs")
            .or_else(|| meta_obj.get("uninstall_action_args"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let data_version = if !uninstall_action_path.is_empty() {
            2
        } else if !build_id.is_empty() {
            1
        } else {
            0
        };

        let meta = ManifestMeta {
            meta_size: 0,
            data_version,
            feature_level: manifest_version,
            is_file_data,
            app_id,
            app_name,
            build_version,
            launch_exe,
            launch_command,
            prereq_ids,
            prereq_name,
            prereq_path,
            prereq_args,
            build_id,
            uninstall_action_path,
            uninstall_action_args,
        };

        // Chunk Data List (CDL)
        let empty_map = serde_json::Map::new();
        let cfl = obj
            .get("ChunkFilesizeList")
            .and_then(|v| v.as_object())
            .unwrap_or(&empty_map);
        let chl = obj
            .get("ChunkHashList")
            .and_then(|v| v.as_object())
            .unwrap_or(&empty_map);
        let csl = obj
            .get("ChunkShaList")
            .and_then(|v| v.as_object())
            .unwrap_or(&empty_map);
        let dgl = obj
            .get("DataGroupList")
            .and_then(|v| v.as_object())
            .unwrap_or(&empty_map);

        let mut chunks = Vec::with_capacity(cfl.len());
        for (guid_str, size_val) in cfl {
            let guid = parse_guid_from_value(&serde_json::Value::String(guid_str.clone()));
            if guid == [0, 0, 0, 0] {
                continue;
            }

            let file_size = blob_to_u64(size_val) as i64;
            let hash = get_by_key_ci(chl, guid_str).map(blob_to_u64).unwrap_or(0);

            let mut sha_hash = [0u8; 20];
            if let Some(sha_val) = get_by_key_ci(csl, guid_str) {
                let parsed = parse_hash_from_value(sha_val, 20);
                sha_hash.copy_from_slice(&parsed[..20]);
            }

            let group_num = if let Some(grp_val) = get_by_key_ci(dgl, guid_str) {
                blob_to_u64(grp_val) as u8
            } else {
                let mut hasher = crc32fast::Hasher::new();
                for g in guid {
                    hasher.update(&g.to_le_bytes());
                }
                (hasher.finalize() % 100) as u8
            };

            chunks.push(ChunkInfo {
                guid,
                hash,
                sha_hash,
                group_num,
                window_size: 1024 * 1024,
                file_size,
                secret_guid: None,
                window_size_compressed: None,
                encryption_tag: None,
            });
        }

        // Also check if chunks were passed as an array under chunk_data_list / ChunkDataList
        let chunk_arr_val = obj
            .get("ChunkDataList")
            .or_else(|| obj.get("chunk_data_list"));
        if let Some(chunk_arr) = chunk_arr_val.and_then(|v| v.as_array()) {
            for c_val in chunk_arr {
                if let Some(c_obj) = c_val.as_object() {
                    let guid = c_obj
                        .get("guid")
                        .or_else(|| c_obj.get("Guid"))
                        .map(parse_guid_from_value)
                        .unwrap_or([0, 0, 0, 0]);
                    if guid == [0, 0, 0, 0] {
                        continue;
                    }
                    let hash = c_obj
                        .get("hash")
                        .or_else(|| c_obj.get("Hash"))
                        .map(blob_to_u64)
                        .unwrap_or(0);
                    let mut sha_hash = [0u8; 20];
                    if let Some(sha_val) = c_obj.get("sha_hash").or_else(|| c_obj.get("ShaHash")) {
                        let parsed = parse_hash_from_value(sha_val, 20);
                        sha_hash.copy_from_slice(&parsed[..20]);
                    }
                    let group_num = c_obj
                        .get("group_num")
                        .or_else(|| c_obj.get("GroupNum"))
                        .map(blob_to_u64)
                        .map(|v| v as u8)
                        .unwrap_or_else(|| {
                            let mut hasher = crc32fast::Hasher::new();
                            for g in guid {
                                hasher.update(&g.to_le_bytes());
                            }
                            (hasher.finalize() % 100) as u8
                        });
                    let window_size = c_obj
                        .get("window_size")
                        .or_else(|| c_obj.get("WindowSize"))
                        .map(blob_to_u64)
                        .unwrap_or(1024 * 1024) as u32;
                    let file_size = c_obj
                        .get("file_size")
                        .or_else(|| c_obj.get("FileSize"))
                        .map(blob_to_u64)
                        .unwrap_or(0) as i64;

                    chunks.push(ChunkInfo {
                        guid,
                        hash,
                        sha_hash,
                        group_num,
                        window_size,
                        file_size,
                        secret_guid: None,
                        window_size_compressed: None,
                        encryption_tag: None,
                    });
                }
            }
        }

        // File Manifest List (FML)
        let files_val = obj
            .get("FileManifestList")
            .or_else(|| obj.get("file_manifest_list"))
            .or_else(|| obj.get("fileManifestList"));

        let mut file_manifest_list = Vec::new();
        if let Some(files) = files_val.and_then(|v| v.as_array()) {
            for fmj in files {
                let filename = fmj
                    .get("Filename")
                    .or_else(|| fmj.get("filename"))
                    .or_else(|| fmj.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let mut hash = [0u8; 20];
                if let Some(hash_val) = fmj.get("FileHash").or_else(|| fmj.get("file_hash")).or_else(|| fmj.get("hash")) {
                    let parsed = parse_hash_from_value(hash_val, 20);
                    hash.copy_from_slice(&parsed[..20]);
                }

                let mut flags = 0u8;
                if fmj.get("bIsReadOnly").or_else(|| fmj.get("is_read_only")).and_then(|v| v.as_bool()).unwrap_or(false) {
                    flags |= 0x01;
                }
                if fmj.get("bIsCompressed").or_else(|| fmj.get("is_compressed")).and_then(|v| v.as_bool()).unwrap_or(false) {
                    flags |= 0x02;
                }
                if fmj.get("bIsUnixExecutable").or_else(|| fmj.get("is_unix_executable")).and_then(|v| v.as_bool()).unwrap_or(false) {
                    flags |= 0x04;
                }

                let install_tags = fmj
                    .get("InstallTags")
                    .or_else(|| fmj.get("install_tags"))
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                let mut chunk_parts = Vec::new();
                let mut total_file_size: u64 = 0;
                let mut cur_file_offset: u64 = 0;

                let parts_val = fmj
                    .get("FileChunkParts")
                    .or_else(|| fmj.get("file_chunk_parts"))
                    .or_else(|| fmj.get("chunk_parts"));

                if let Some(parts) = parts_val.and_then(|v| v.as_array()) {
                    for cpj in parts {
                        let guid = cpj
                            .get("Guid")
                            .or_else(|| cpj.get("guid"))
                            .map(parse_guid_from_value)
                            .unwrap_or([0, 0, 0, 0]);

                        let offset = cpj
                            .get("Offset")
                            .or_else(|| cpj.get("offset"))
                            .map(blob_to_u64)
                            .unwrap_or(0) as u32;

                        let size = cpj
                            .get("Size")
                            .or_else(|| cpj.get("size"))
                            .map(blob_to_u64)
                            .unwrap_or(0) as u32;

                        chunk_parts.push(ChunkPart {
                            guid,
                            offset,
                            size,
                            file_offset: cur_file_offset,
                        });

                        cur_file_offset += size as u64;
                        total_file_size += size as u64;
                    }
                }

                if total_file_size == 0 {
                    if let Some(fs) = fmj.get("FileSize").or_else(|| fmj.get("file_size")) {
                        total_file_size = blob_to_u64(fs);
                    }
                }

                file_manifest_list.push(FileManifest {
                    filename,
                    symlink_target: String::new(),
                    hash,
                    flags,
                    install_tags,
                    chunk_parts,
                    file_size: total_file_size,
                    mime_type: None,
                    hash_md5: None,
                    hash_sha256: None,
                });
            }
        }

        // Ensure all chunks referenced by chunk_parts exist in chunks list
        let mut existing_guids: std::collections::HashSet<[u32; 4]> =
            chunks.iter().map(|c| c.guid).collect();

        for f in &file_manifest_list {
            for part in &f.chunk_parts {
                if part.guid != [0, 0, 0, 0] && !existing_guids.contains(&part.guid) {
                    let mut hasher = crc32fast::Hasher::new();
                    for g in part.guid {
                        hasher.update(&g.to_le_bytes());
                    }
                    let group_num = (hasher.finalize() % 100) as u8;
                    chunks.push(ChunkInfo {
                        guid: part.guid,
                        hash: 0,
                        sha_hash: [0u8; 20],
                        group_num,
                        window_size: 1024 * 1024,
                        file_size: part.size as i64,
                        secret_guid: None,
                        window_size_compressed: None,
                        encryption_tag: None,
                    });
                    existing_guids.insert(part.guid);
                }
            }
        }

        // Custom Fields
        let mut custom_fields = HashMap::new();
        let cf_val = obj.get("CustomFields").or_else(|| obj.get("custom_fields"));
        if let Some(cf) = cf_val.and_then(|v| v.as_object()) {
            for (k, v) in cf {
                if let Some(s) = v.as_str() {
                    custom_fields.insert(k.clone(), s.to_string());
                } else {
                    custom_fields.insert(k.clone(), v.to_string());
                }
            }
        }

        Ok(Manifest {
            header_size: 0,
            size_uncompressed: bytes.len() as u32,
            size_compressed: 0,
            sha_hash: [0u8; 20],
            stored_as: 0,
            version: manifest_version,
            meta,
            chunk_data_list: chunks,
            file_manifest_list,
            custom_fields,
        })
    }

    /// Serializes manifest into binary format
    pub fn write_to_vec(&self) -> Result<Vec<u8>> {
        let mut body = Vec::new();

        // 1. Write ManifestMeta
        let mut meta_buf = Vec::new();
        meta_buf.write_u32::<LittleEndian>(0)?; // placeholder for meta_size
        meta_buf.write_u8(self.meta.data_version)?;
        meta_buf.write_u32::<LittleEndian>(self.meta.feature_level)?;
        meta_buf.write_u8(if self.meta.is_file_data { 1 } else { 0 })?;
        meta_buf.write_u32::<LittleEndian>(self.meta.app_id)?;
        write_fstring(&mut meta_buf, &self.meta.app_name)?;
        write_fstring(&mut meta_buf, &self.meta.build_version)?;
        write_fstring(&mut meta_buf, &self.meta.launch_exe)?;
        write_fstring(&mut meta_buf, &self.meta.launch_command)?;

        meta_buf.write_u32::<LittleEndian>(self.meta.prereq_ids.len() as u32)?;
        for pid in &self.meta.prereq_ids {
            write_fstring(&mut meta_buf, pid)?;
        }
        write_fstring(&mut meta_buf, &self.meta.prereq_name)?;
        write_fstring(&mut meta_buf, &self.meta.prereq_path)?;
        write_fstring(&mut meta_buf, &self.meta.prereq_args)?;

        if self.meta.data_version >= 1 {
            write_fstring(&mut meta_buf, &self.meta.build_id)?;
        }
        if self.meta.data_version >= 2 {
            write_fstring(&mut meta_buf, &self.meta.uninstall_action_path)?;
            write_fstring(&mut meta_buf, &self.meta.uninstall_action_args)?;
        }
        let meta_len = meta_buf.len() as u32;
        (&mut meta_buf[0..4]).write_u32::<LittleEndian>(meta_len)?;
        body.write_all(&meta_buf)?;

        // 2. Write CDL
        let mut cdl_buf = Vec::new();
        cdl_buf.write_u32::<LittleEndian>(0)?; // placeholder
        cdl_buf.write_u8(0)?; // version
        cdl_buf.write_u32::<LittleEndian>(self.chunk_data_list.len() as u32)?;

        for c in &self.chunk_data_list {
            for g in c.guid {
                cdl_buf.write_u32::<LittleEndian>(g)?;
            }
        }
        for c in &self.chunk_data_list {
            cdl_buf.write_u64::<LittleEndian>(c.hash)?;
        }
        for c in &self.chunk_data_list {
            cdl_buf.write_all(&c.sha_hash)?;
        }
        for c in &self.chunk_data_list {
            cdl_buf.write_u8(c.group_num)?;
        }
        for c in &self.chunk_data_list {
            cdl_buf.write_u32::<LittleEndian>(c.window_size)?;
        }
        for c in &self.chunk_data_list {
            cdl_buf.write_i64::<LittleEndian>(c.file_size)?;
        }
        let cdl_len = cdl_buf.len() as u32;
        (&mut cdl_buf[0..4]).write_u32::<LittleEndian>(cdl_len)?;
        body.write_all(&cdl_buf)?;

        // 3. Write FML
        let mut fml_buf = Vec::new();
        fml_buf.write_u32::<LittleEndian>(0)?; // placeholder
        fml_buf.write_u8(0)?; // version 0
        fml_buf.write_u32::<LittleEndian>(self.file_manifest_list.len() as u32)?;

        for f in &self.file_manifest_list {
            write_fstring(&mut fml_buf, &f.filename)?;
        }
        for f in &self.file_manifest_list {
            write_fstring(&mut fml_buf, &f.symlink_target)?;
        }
        for f in &self.file_manifest_list {
            fml_buf.write_all(&f.hash)?;
        }
        for f in &self.file_manifest_list {
            fml_buf.write_u8(f.flags)?;
        }
        for f in &self.file_manifest_list {
            fml_buf.write_u32::<LittleEndian>(f.install_tags.len() as u32)?;
            for tag in &f.install_tags {
                write_fstring(&mut fml_buf, tag)?;
            }
        }
        for f in &self.file_manifest_list {
            fml_buf.write_u32::<LittleEndian>(f.chunk_parts.len() as u32)?;
            for p in &f.chunk_parts {
                fml_buf.write_u32::<LittleEndian>(28)?;
                for g in p.guid {
                    fml_buf.write_u32::<LittleEndian>(g)?;
                }
                fml_buf.write_u32::<LittleEndian>(p.offset)?;
                fml_buf.write_u32::<LittleEndian>(p.size)?;
            }
        }
        let fml_len = fml_buf.len() as u32;
        (&mut fml_buf[0..4]).write_u32::<LittleEndian>(fml_len)?;
        body.write_all(&fml_buf)?;

        // 4. Write CustomFields
        let mut cf_buf = Vec::new();
        cf_buf.write_u32::<LittleEndian>(0)?;
        cf_buf.write_u8(0)?;
        cf_buf.write_u32::<LittleEndian>(self.custom_fields.len() as u32)?;
        for k in self.custom_fields.keys() {
            write_fstring(&mut cf_buf, k)?;
        }
        for v in self.custom_fields.values() {
            write_fstring(&mut cf_buf, v)?;
        }
        let cf_len = cf_buf.len() as u32;
        (&mut cf_buf[0..4]).write_u32::<LittleEndian>(cf_len)?;
        body.write_all(&cf_buf)?;

        // Compress body with Zlib
        let size_uncompressed = body.len() as u32;
        let mut hasher = Sha1::new();
        hasher.update(&body);
        let sha_hash: [u8; 20] = hasher.finalize().into();

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&body)?;
        let compressed_body = encoder.finish()?;
        let size_compressed = compressed_body.len() as u32;

        let header_size = 41u32;
        let mut out = Vec::with_capacity(header_size as usize + compressed_body.len());

        out.write_u32::<LittleEndian>(MANIFEST_HEADER_MAGIC)?;
        out.write_u32::<LittleEndian>(header_size)?;
        out.write_u32::<LittleEndian>(size_uncompressed)?;
        out.write_u32::<LittleEndian>(size_compressed)?;
        out.write_all(&sha_hash)?;
        out.write_u8(0x01)?; // zlib compressed
        out.write_u32::<LittleEndian>(self.version.max(18))?;
        out.write_all(&compressed_body)?;

        Ok(out)
    }
}
