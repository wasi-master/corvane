//! The repository list of an installed GitHub Desktop, for Corvane's
//! "Import Repositories from GitHub Desktop…" (a Corvane extra; flag
//! `206-import-from-github-desktop`).
//!
//! GHD keeps its repositories in the renderer's IndexedDB (Dexie database
//! `Database`, object store `repositories`: `{ id, path, alias, missing,
//! gitHubRepositoryID, … }`), which Chromium stores in LevelDB under
//! `<userData>/IndexedDB/file__0.indexeddb.leveldb`. This module reads that
//! directory without opening it as a database (GHD may be running and holds
//! its lock): the write-ahead logs and the sorted tables are decoded, the
//! newest entry per key wins (deletions included), and every live value that
//! is a repository record - a V8-serialised object with `path` and
//! `gitHubRepositoryID` keys - yields its path and alias.
//!
//! Best effort by design: an unreadable file is skipped, an unknown value
//! layout yields nothing, and nothing is ever written.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A repository in GHD's list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GhdRepository {
    pub path: PathBuf,
    /// GHD's per-repository alias (`Repository.alias`).
    pub alias: Option<String>,
}

/// GHD's user-data directories on this machine (release, then Beta), or
/// `CORVANE_GHD_DATA_DIR` (tests and the parity harness).
pub fn data_dirs() -> Vec<PathBuf> {
    if let Some(dir) = std::env::var_os("CORVANE_GHD_DATA_DIR") {
        return vec![PathBuf::from(dir)];
    }
    let Some(base) = dirs::config_dir() else {
        return Vec::new();
    };
    ["GitHub Desktop", "GitHub Desktop-Beta"]
        .iter()
        .map(|name| base.join(name))
        .filter(|dir| dir.is_dir())
        .collect()
}

/// Every repository GHD lists, across [`data_dirs`], first occurrence of a
/// path kept, in GHD's `id` order.
pub fn repositories() -> Vec<GhdRepository> {
    let mut out: Vec<GhdRepository> = Vec::new();
    for dir in data_dirs() {
        for repo in read_data_dir(&dir) {
            if !out.iter().any(|r| r.path == repo.path) {
                out.push(repo);
            }
        }
    }
    out
}

/// The repositories in one GHD user-data directory.
pub fn read_data_dir(data_dir: &Path) -> Vec<GhdRepository> {
    let Ok(entries) = std::fs::read_dir(data_dir.join("IndexedDB")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".indexeddb.leveldb"))
        {
            out.extend(read_leveldb(&path));
        }
    }
    out
}

/// The repository records among the live values of one LevelDB directory.
pub fn read_leveldb(dir: &Path) -> Vec<GhdRepository> {
    let mut latest: HashMap<Vec<u8>, (u64, Option<Vec<u8>>)> = HashMap::new();
    let mut put = |key: Vec<u8>, seq: u64, value: Option<Vec<u8>>| match latest.get(&key) {
        Some((known, _)) if *known >= seq => {}
        _ => {
            latest.insert(key, (seq, value));
        }
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        match path.extension().and_then(|e| e.to_str()) {
            Some("log") => read_log(&bytes, &mut put),
            Some("ldb") | Some("sst") => read_table(&bytes, &mut put),
            _ => {}
        }
    }
    // records carry their `id` as the key's tail; sort by the IndexedDB key so
    // the list keeps GHD's insertion order
    let mut live: Vec<(Vec<u8>, Vec<u8>)> = latest
        .into_iter()
        .filter_map(|(key, (_, value))| value.map(|v| (key, v)))
        .collect();
    live.sort();
    live.into_iter()
        .filter_map(|(_, value)| repository_record(&value))
        .collect()
}

// ---- LevelDB -------------------------------------------------------------

const LOG_BLOCK: usize = 32 * 1024;

/// A write-ahead log: 32 KiB blocks of fragments (`FULL` / `FIRST` /
/// `MIDDLE` / `LAST`) that reassemble into `WriteBatch`es.
fn read_log(bytes: &[u8], put: &mut impl FnMut(Vec<u8>, u64, Option<Vec<u8>>)) {
    let mut record: Vec<u8> = Vec::new();
    let mut pos = 0;
    while pos + 7 <= bytes.len() {
        let block_left = LOG_BLOCK - pos % LOG_BLOCK;
        if block_left < 7 {
            pos += block_left;
            continue;
        }
        let len = u16::from_le_bytes([bytes[pos + 4], bytes[pos + 5]]) as usize;
        let kind = bytes[pos + 6];
        let start = pos + 7;
        let Some(data) = bytes.get(start..start + len) else {
            break;
        };
        pos = start + len;
        match kind {
            1 => {
                read_batch(data, put);
                record.clear();
            }
            2 => {
                record.clear();
                record.extend_from_slice(data);
            }
            3 => record.extend_from_slice(data),
            4 => {
                record.extend_from_slice(data);
                read_batch(&record, put);
                record.clear();
            }
            // zero padding at the end of a block, or a torn write
            _ => {
                pos += LOG_BLOCK - pos % LOG_BLOCK;
                record.clear();
            }
        }
    }
}

/// `WriteBatch`: sequence (u64), count (u32), then `kTypeValue` (1) key
/// value / `kTypeDeletion` (0) key entries, one sequence number each.
fn read_batch(data: &[u8], put: &mut impl FnMut(Vec<u8>, u64, Option<Vec<u8>>)) {
    if data.len() < 12 {
        return;
    }
    let mut seq = u64::from_le_bytes(data[0..8].try_into().unwrap_or_default());
    let mut r = Reader::new(&data[12..]);
    while let Some(kind) = r.byte() {
        let Some(key) = r.slice() else {
            return;
        };
        let value = match kind {
            1 => match r.slice() {
                Some(v) => Some(v.to_vec()),
                None => return,
            },
            0 => None,
            _ => return,
        };
        put(key.to_vec(), seq, value);
        seq += 1;
    }
}

/// A sorted table: the footer's index block lists the data blocks; each
/// entry's internal key ends in `seq << 8 | kind`.
fn read_table(bytes: &[u8], put: &mut impl FnMut(Vec<u8>, u64, Option<Vec<u8>>)) {
    if bytes.len() < 48 {
        return;
    }
    let footer = &bytes[bytes.len() - 48..];
    let mut r = Reader::new(footer);
    // metaindex handle, then the index handle
    let (Some(_), Some(_), Some(index_offset), Some(index_size)) =
        (r.varint(), r.varint(), r.varint(), r.varint())
    else {
        return;
    };
    let Some(index) = block(bytes, index_offset as usize, index_size as usize) else {
        return;
    };
    for (_, handle) in block_entries(&index) {
        let mut h = Reader::new(&handle);
        let (Some(offset), Some(size)) = (h.varint(), h.varint()) else {
            continue;
        };
        let Some(data) = block(bytes, offset as usize, size as usize) else {
            continue;
        };
        for (internal_key, value) in block_entries(&data) {
            if internal_key.len() < 8 {
                continue;
            }
            let (user_key, trailer) = internal_key.split_at(internal_key.len() - 8);
            let tag = u64::from_le_bytes(trailer.try_into().unwrap_or_default());
            let value = (tag & 0xff == 1).then_some(value);
            put(user_key.to_vec(), tag >> 8, value);
        }
    }
}

/// A block's contents, snappy-decompressed when its trailer says so.
fn block(bytes: &[u8], offset: usize, size: usize) -> Option<Vec<u8>> {
    let raw = bytes.get(offset..offset.checked_add(size)?)?;
    match bytes.get(offset + size)? {
        0 => Some(raw.to_vec()),
        1 => snappy_decompress(raw),
        _ => None,
    }
}

/// A block's key/value entries (prefix-compressed keys, restart array and
/// its count at the end).
fn block_entries(block: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut out = Vec::new();
    if block.len() < 4 {
        return out;
    }
    let restarts = u32::from_le_bytes(block[block.len() - 4..].try_into().unwrap_or_default());
    let Some(end) = (restarts as usize)
        .checked_mul(4)
        .and_then(|n| block.len().checked_sub(4 + n))
    else {
        return out;
    };
    let mut r = Reader::new(&block[..end]);
    let mut key: Vec<u8> = Vec::new();
    while !r.is_empty() {
        let (Some(shared), Some(unshared), Some(value_len)) = (r.varint(), r.varint(), r.varint())
        else {
            break;
        };
        let (Some(suffix), Some(value)) = (r.take(unshared as usize), r.take(value_len as usize))
        else {
            break;
        };
        key.truncate(shared as usize);
        key.extend_from_slice(suffix);
        out.push((key.clone(), value.to_vec()));
    }
    out
}

/// Raw snappy (the block format LevelDB uses).
fn snappy_decompress(input: &[u8]) -> Option<Vec<u8>> {
    let mut r = Reader::new(input);
    let len = r.varint()? as usize;
    let mut out: Vec<u8> = Vec::with_capacity(len.min(1 << 24));
    while let Some(tag) = r.byte() {
        match tag & 3 {
            0 => {
                let mut n = (tag >> 2) as usize;
                if n >= 60 {
                    let bytes = n - 59;
                    let mut v = 0usize;
                    for i in 0..bytes {
                        v |= (r.byte()? as usize) << (8 * i);
                    }
                    n = v;
                }
                out.extend_from_slice(r.take(n + 1)?);
            }
            kind => {
                let (len, offset) = match kind {
                    1 => (
                        4 + ((tag >> 2) & 7) as usize,
                        (((tag >> 5) as usize) << 8) | r.byte()? as usize,
                    ),
                    2 => {
                        let b = r.take(2)?;
                        (
                            (tag >> 2) as usize + 1,
                            u16::from_le_bytes([b[0], b[1]]) as usize,
                        )
                    }
                    _ => {
                        let b = r.take(4)?;
                        (
                            (tag >> 2) as usize + 1,
                            u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize,
                        )
                    }
                };
                if offset == 0 || offset > out.len() {
                    return None;
                }
                let start = out.len() - offset;
                for i in 0..len {
                    let byte = out[start + i];
                    out.push(byte);
                }
            }
        }
    }
    (out.len() == len).then_some(out)
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn is_empty(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn byte(&mut self) -> Option<u8> {
        let b = *self.bytes.get(self.pos)?;
        self.pos += 1;
        Some(b)
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.bytes.get(self.pos..self.pos.checked_add(n)?)?;
        self.pos += n;
        Some(s)
    }

    fn varint(&mut self) -> Option<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.byte()?;
            v |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    }

    /// A varint length, then that many bytes.
    fn slice(&mut self) -> Option<&'a [u8]> {
        let n = self.varint()? as usize;
        self.take(n)
    }
}

// ---- V8 values ------------------------------------------------------------

/// A repository record's path and alias: the value holds the V8 strings
/// `path` and `gitHubRepositoryID` as keys (other object stores with a
/// `path` never have both).
fn repository_record(value: &[u8]) -> Option<GhdRepository> {
    find_key(value, "gitHubRepositoryID")?;
    let path = string_after_key(value, "path")?;
    if path.is_empty() {
        return None;
    }
    let alias = string_after_key(value, "alias").filter(|a| !a.is_empty());
    Some(GhdRepository {
        path: PathBuf::from(path),
        alias,
    })
}

/// Where the one-byte V8 string `key` (`"` tag, varint length, Latin-1)
/// ends.
fn find_key(value: &[u8], key: &str) -> Option<usize> {
    let mut needle = vec![b'"'];
    let mut len = key.len();
    loop {
        let b = (len & 0x7f) as u8;
        len >>= 7;
        needle.push(if len > 0 { b | 0x80 } else { b });
        if len == 0 {
            break;
        }
    }
    needle.extend_from_slice(key.as_bytes());
    value
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|at| at + needle.len())
}

/// The V8 string right after the key `key`: one-byte (`"`, Latin-1),
/// two-byte (`c`, UTF-16LE) or UTF-8 (`S`).
fn string_after_key(value: &[u8], key: &str) -> Option<String> {
    let at = find_key(value, key)?;
    let mut r = Reader::new(&value[at..]);
    // padding (`\0`) may precede a two-byte string to align it
    let mut tag = r.byte()?;
    while tag == 0 {
        tag = r.byte()?;
    }
    let bytes = r.slice()?;
    match tag {
        b'"' => Some(bytes.iter().map(|&b| b as char).collect()),
        b'S' => String::from_utf8(bytes.to_vec()).ok(),
        b'c' => {
            let units: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&c| u16::from_le_bytes(c))
                .collect();
            String::from_utf16(&units).ok()
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v8_str(s: &str) -> Vec<u8> {
        let mut out = vec![b'"', s.len() as u8];
        out.extend_from_slice(s.as_bytes());
        out
    }

    fn v8_utf16(s: &str) -> Vec<u8> {
        let units: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        let mut out = vec![b'c', units.len() as u8];
        out.extend(units);
        out
    }

    /// A record as GHD's Dexie writes it (IDB envelope bytes, then the V8
    /// object).
    fn record(path: &[u8], alias: Option<&str>) -> Vec<u8> {
        let mut v = vec![0xff, 0x14, 0xff, 0x0f, b'o'];
        v.extend(v8_str("path"));
        v.extend(path);
        v.extend(v8_str("gitHubRepositoryID"));
        v.push(b'0');
        v.extend(v8_str("missing"));
        v.push(b'F');
        if let Some(alias) = alias {
            v.extend(v8_str("alias"));
            v.extend(v8_str(alias));
        }
        v.extend(v8_str("id"));
        v.extend([b'I', 2, b'{', 5]);
        v
    }

    fn batch(seq: u64, ops: &[(&[u8], Option<&[u8]>)]) -> Vec<u8> {
        let mut b = seq.to_le_bytes().to_vec();
        b.extend((ops.len() as u32).to_le_bytes());
        for (key, value) in ops {
            b.push(u8::from(value.is_some()));
            b.push(key.len() as u8);
            b.extend_from_slice(key);
            if let Some(value) = value {
                let mut len = value.len();
                loop {
                    let byte = (len & 0x7f) as u8;
                    len >>= 7;
                    b.push(if len > 0 { byte | 0x80 } else { byte });
                    if len == 0 {
                        break;
                    }
                }
                b.extend_from_slice(value);
            }
        }
        b
    }

    fn log(batches: &[Vec<u8>]) -> Vec<u8> {
        let mut out = Vec::new();
        for data in batches {
            out.extend([0, 0, 0, 0]);
            out.extend((data.len() as u16).to_le_bytes());
            out.push(1);
            out.extend(data);
        }
        out
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("corvane-ghd-import-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_live_repository_records_from_the_log() {
        let dir = temp_dir("log");
        let a = record(&v8_str("/Users/me/one"), None);
        let b = record(&v8_utf16("/Users/me/två"), Some("Two"));
        let gone = record(&v8_str("/Users/me/removed"), None);
        let other = [v8_str("path"), v8_str("/not/a/repo")].concat();
        let bytes = log(&[
            batch(1, &[(b"k1", Some(&a)), (b"k3", Some(&gone))]),
            batch(3, &[(b"k2", Some(&b)), (b"k9", Some(&other))]),
            batch(5, &[(b"k3", None)]),
        ]);
        std::fs::write(dir.join("000003.log"), bytes).unwrap();
        let repos = read_leveldb(&dir);
        assert_eq!(
            repos,
            vec![
                GhdRepository {
                    path: "/Users/me/one".into(),
                    alias: None
                },
                GhdRepository {
                    path: "/Users/me/två".into(),
                    alias: Some("Two".into())
                },
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_newer_log_entry_wins_over_the_table() {
        let dir = temp_dir("table");
        let old = record(&v8_str("/Users/me/old"), None);
        let new = record(&v8_str("/Users/me/new"), None);
        // a one-block table holding `k1` at sequence 2 (uncompressed)
        let mut internal = b"k1".to_vec();
        internal.extend(((2u64 << 8) | 1).to_le_bytes());
        let mut data = vec![0, internal.len() as u8, old.len() as u8];
        data.extend(&internal);
        data.extend(&old);
        data.extend(0u32.to_le_bytes());
        data.extend(1u32.to_le_bytes());
        let mut table = data.clone();
        table.extend([0, 0, 0, 0, 0]);
        let index_offset = table.len();
        let handle = [0u8, data.len() as u8];
        let mut index = vec![0, 2, handle.len() as u8];
        index.extend(b"k2");
        index.extend(handle);
        index.extend(0u32.to_le_bytes());
        index.extend(1u32.to_le_bytes());
        let index_len = index.len();
        table.extend(&index);
        table.extend([0, 0, 0, 0, 0]);
        let mut footer = vec![0, 0, index_offset as u8, index_len as u8];
        footer.resize(40, 0);
        footer.extend(0xdb4775248b80fb57u64.to_le_bytes());
        table.extend(footer);
        std::fs::write(dir.join("000005.ldb"), &table).unwrap();
        assert_eq!(read_leveldb(&dir)[0].path, PathBuf::from("/Users/me/old"));
        std::fs::write(
            dir.join("000007.log"),
            log(&[batch(9, &[(b"k1", Some(&new))])]),
        )
        .unwrap();
        assert_eq!(read_leveldb(&dir)[0].path, PathBuf::from("/Users/me/new"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn snappy_literals_and_copies() {
        // "abcabcabcX": literal "abc", copy len 6 offset 3, literal "X"
        let compressed = [10, 0b0000_1000, b'a', b'b', b'c', 0b0000_1001, 3, 0, b'X'];
        assert_eq!(
            snappy_decompress(&compressed).as_deref(),
            Some(&b"abcabcabcX"[..])
        );
        assert_eq!(snappy_decompress(&[5, 0b0000_1000, b'a']), None);
    }
}
