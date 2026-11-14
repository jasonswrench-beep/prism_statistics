//! How much a file may unpack to. Projects, workbooks, `.jasp` files and `.zsav` files are compressed, so a small file
//! can claim to hold gigabytes; these checks refuse such a file before memory runs out, with a message saying why.

use std::io::{Cursor, Read};

use crate::error::{Error, Result};

/// The most any one file may unpack to: far more than a teaching or research dataset needs.
pub const MAX_UNPACKED: u64 = 1 << 30;

fn too_big(what: &str) -> Error {
    Error::Invalid(format!("{what} unpacks to more than 1 GB, which is more than PrismStatistics reads. It may be damaged."))
}

/// Reads a zip entry, stopping with an error once it passes `MAX_UNPACKED`, whatever size the entry declares.
pub fn read_capped(entry: impl Read, what: &str) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    entry.take(MAX_UNPACKED + 1).read_to_end(&mut out)?;
    if out.len() as u64 > MAX_UNPACKED {
        return Err(too_big(what));
    }
    Ok(out)
}

/// Refuses a zip archive (a workbook) whose entries declare more than `MAX_UNPACKED` in all.
pub fn check_zip(bytes: &[u8], what: &str) -> Result<()> {
    let Ok(mut zip) = zip::ZipArchive::new(Cursor::new(bytes)) else { return Ok(()) }; // the reader reports damage itself
    let mut total = 0u64;
    for i in 0..zip.len() {
        let Ok(f) = zip.by_index_raw(i) else { continue };
        total = total.saturating_add(f.size());
        if total > MAX_UNPACKED {
            return Err(too_big(what));
        }
    }
    Ok(())
}

/// Refuses a `.zsav` file whose compressed blocks declare more than `MAX_UNPACKED` in all. The block table (the
/// "ztrailer") is at the end of the file: a 24-byte header holding the block count, then 24 bytes per block with its
/// uncompressed size. A file without a readable table is left to the reader.
pub fn check_zsav(bytes: &[u8]) -> Result<()> {
    if !bytes.starts_with(b"$FL3") {
        return Ok(());
    }
    let len = bytes.len();
    let i32_at = |o: usize| bytes.get(o..o + 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    // The table is 24 + 24 n bytes long and ends the file; find the n that its own header states.
    let mut n = 1usize;
    while 24 + 24 * n <= len && n <= 1 << 20 {
        let start = len - 24 - 24 * n;
        if i32_at(start + 20) == Some(n as i32) {
            let total: u64 = (0..n).filter_map(|k| i32_at(start + 24 + 24 * k + 16)).map(|s| s.max(0) as u64).sum();
            if total > MAX_UNPACKED {
                return Err(too_big("This .zsav file"));
            }
            return Ok(());
        }
        n += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn declared_and_actual_sizes_are_capped() {
        // A zip entry is read only up to the cap.
        assert_eq!(read_capped(Cursor::new(vec![1u8; 10]), "x").unwrap().len(), 10);
        // A workbook whose entries declare too much is refused before it is opened.
        let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
        z.start_file("xl/worksheets/sheet1.xml", zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)).unwrap();
        z.write_all(&vec![b'a'; 1 << 20]).unwrap();
        let small = z.finish().unwrap().into_inner();
        assert!(check_zip(&small, "x").is_ok());
        // A .zsav block table claiming 2 GB.
        let mut f = b"$FL3".to_vec();
        f.extend(vec![0u8; 100]);
        let mut table = vec![0u8; 24];
        table[20..24].copy_from_slice(&2i32.to_le_bytes());
        for _ in 0..2 {
            let mut e = vec![0u8; 24];
            e[16..20].copy_from_slice(&i32::MAX.to_le_bytes());
            table.extend(e);
        }
        f.extend(&table);
        assert!(check_zsav(&f).unwrap_err().to_string().contains("1 GB"));
        assert!(check_zsav(b"$FL2 plain sav").is_ok());
    }
}
