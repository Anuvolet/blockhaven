//! Region file: 32x32 chunk slots. Header of 1024 (u32 offset, u32 length) entries followed by
//! the chunk blobs. Files are rewritten atomically on save (write temp + rename).

use std::collections::HashMap;
use std::path::Path;

pub const SLOTS: usize = 1024;
const HEADER: usize = SLOTS * 8;
const MAGIC: &[u8; 4] = b"BHR1";

#[derive(Default)]
pub struct Region {
    chunks: HashMap<u32, Vec<u8>>,
}

impl Region {
    pub fn load(path: &Path) -> Option<Region> {
        let bytes = std::fs::read(path).ok()?;
        if bytes.len() < 4 + HEADER || &bytes[0..4] != MAGIC {
            return None;
        }
        let mut chunks = HashMap::new();
        for i in 0..SLOTS {
            let o = 4 + i * 8;
            let off = u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]) as usize;
            let len = u32::from_le_bytes([bytes[o + 4], bytes[o + 5], bytes[o + 6], bytes[o + 7]]) as usize;
            if len == 0 {
                continue;
            }
            if off + len > bytes.len() {
                return None;
            }
            chunks.insert(i as u32, bytes[off..off + len].to_vec());
        }
        Some(Region { chunks })
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let mut out = Vec::with_capacity(4 + HEADER + self.chunks.values().map(|v| v.len()).sum::<usize>());
        out.extend_from_slice(MAGIC);
        out.resize(4 + HEADER, 0);
        let mut keys: Vec<u32> = self.chunks.keys().copied().collect();
        keys.sort();
        for k in keys {
            let data = &self.chunks[&k];
            let off = out.len() as u32;
            let len = data.len() as u32;
            let o = 4 + k as usize * 8;
            out[o..o + 4].copy_from_slice(&off.to_le_bytes());
            out[o + 4..o + 8].copy_from_slice(&len.to_le_bytes());
            out.extend_from_slice(data);
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, &out)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn get(&self, idx: u32) -> Option<&[u8]> {
        self.chunks.get(&idx).map(|v| v.as_slice())
    }

    pub fn set(&mut self, idx: u32, data: Vec<u8>) {
        self.chunks.insert(idx, data);
    }

}
