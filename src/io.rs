//! File I/O: encoding-aware reading and atomic saving (spec F80–F83, §8.4).

use std::fs;
use std::io::Write;
use std::path::Path;

use crate::buffer::{DiskStamp, Encoding};

/// Everything needed to open a file into a [`crate::buffer::Buffer`].
pub struct ReadOutcome {
    pub text: String,
    pub encoding: Encoding,
    pub read_only: bool,
    pub stamp: DiskStamp,
}

pub enum ReadError {
    NotFound,
    Io(std::io::Error),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::NotFound => write!(f, "file does not exist"),
            ReadError::Io(e) => write!(f, "{}", e),
        }
    }
}

/// Read bytes, decode with BOM/UTF detection, warn-free fallback to lossy.
pub fn read_file(path: &Path) -> Result<ReadOutcome, ReadError> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(ReadError::NotFound),
        Err(e) => return Err(ReadError::Io(e)),
    };
    let meta = fs::metadata(path).map_err(ReadError::Io)?;
    let stamp = DiskStamp::from_meta(&meta);
    let read_only = probe_read_only(path, &meta);

    let (text, encoding) = decode_bytes(&bytes);
    Ok(ReadOutcome { text, encoding, read_only, stamp })
}

fn probe_read_only(path: &Path, meta: &fs::Metadata) -> bool {
    // On Windows the read-only attribute is the truth; on Unix we test write
    // permission bits. Opening for append is the most reliable cross-platform
    // probe but has side effects on network mounts, so we check attributes first.
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_READONLY: u32 = 0x1;
        if meta.file_attributes() & FILE_ATTRIBUTE_READONLY != 0 {
            return true;
        }
        // Fall through to the probe for ACL-denied cases.
    }
    match fs::OpenOptions::new().write(true).open(path) {
        Ok(_) => false,
        Err(_) => true,
    }
}

/// Decode file bytes into internal UTF-8 text with encoding provenance.
fn decode_bytes(bytes: &[u8]) -> (String, Encoding) {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let body = &bytes[3..];
        return match std::str::from_utf8(body) {
            Ok(s) => (s.to_owned(), Encoding::Utf8Bom),
            Err(_) => (String::from_utf8_lossy(body).into_owned(), Encoding::Lossy),
        };
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return decode_utf16(&bytes[2..], true);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return decode_utf16(&bytes[2..], false);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => (s.to_owned(), Encoding::Utf8),
        Err(_) => (String::from_utf8_lossy(bytes).into_owned(), Encoding::Lossy),
    }
}

fn decode_utf16(body: &[u8], little: bool) -> (String, Encoding) {
    let mut units = Vec::with_capacity(body.len() / 2);
    for chunk in body.chunks_exact(2) {
        let u = if little {
            u16::from_le_bytes([chunk[0], chunk[1]])
        } else {
            u16::from_be_bytes([chunk[0], chunk[1]])
        };
        units.push(u);
    }
    let odd = !body.len().is_multiple_of(2); // stable rust 1.95 supports is_multiple_of
    let encoding = if odd { Encoding::Lossy } else if little { Encoding::Utf16Le } else { Encoding::Utf16Be };
    (String::from_utf16_lossy(&units), encoding)
}

#[derive(Debug)]
pub enum SaveError {
    ReadOnly,
    Io(std::io::Error),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::ReadOnly => write!(f, "file is read-only"),
            SaveError::Io(e) => write!(f, "{}", e),
        }
    }
}

/// Atomic save: write to a temp file in the same directory, then rename over
/// the target (spec F80). Never leaves a partial target behind.
pub fn atomic_save(path: &Path, bytes: &[u8]) -> Result<(), SaveError> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "document".to_owned());

    // Respect an existing read-only file (F85): surface instead of clobbering.
    if path.exists() {
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            const FILE_ATTRIBUTE_READONLY: u32 = 0x1;
            if let Ok(meta) = fs::metadata(path) {
                if meta.file_attributes() & FILE_ATTRIBUTE_READONLY != 0 {
                    return Err(SaveError::ReadOnly);
                }
            }
        }
        #[cfg(not(windows))]
        {
            if fs::OpenOptions::new().write(true).open(path).is_err() {
                return Err(SaveError::ReadOnly);
            }
        }
    }

    let tmp_name = format!(".{}.rdt-{}", file_name, std::process::id());
    let tmp_path = dir.join(tmp_name);
    let result = (|| -> std::io::Result<()> {
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp_path, path)?;
        Ok(())
    })();
    match result {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp_path);
            Err(SaveError::Io(e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_plain_utf8() {
        let (t, e) = decode_bytes("héllo".as_bytes());
        assert_eq!(t, "héllo");
        assert_eq!(e, Encoding::Utf8);
    }

    #[test]
    fn decode_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("hi".as_bytes());
        let (t, e) = decode_bytes(&bytes);
        assert_eq!(t, "hi");
        assert_eq!(e, Encoding::Utf8Bom);
    }

    #[test]
    fn decode_utf16le() {
        let mut bytes = vec![0xFF, 0xFE];
        for u in "héllo".encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        let (t, e) = decode_bytes(&bytes);
        assert_eq!(t, "héllo");
        assert_eq!(e, Encoding::Utf16Le);
    }

    #[test]
    fn lossy_fallback() {
        let (_, e) = decode_bytes(&[0xC3, 0x28]); // invalid UTF-8
        assert_eq!(e, Encoding::Lossy);
    }

    #[test]
    fn atomic_save_roundtrip() {
        let dir = std::env::temp_dir().join(format!("rdt-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("doc.md");
        atomic_save(&file, b"hello\n").unwrap();
        atomic_save(&file, b"world\n").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "world\n");
        fs::remove_dir_all(&dir).unwrap();
    }
}
