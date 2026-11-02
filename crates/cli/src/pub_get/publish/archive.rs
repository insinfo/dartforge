//! `createTarGz` of Pub's `io.dart`: a tar written like `package:tar`
//! (`tarWriterWith(format: OutputFormat.gnuLongName)`) compressed with the
//! VM's gzip.
use super::deflate::gzip;
use super::paths as p;
use crate::pub_get::Failure;

const BLOCK: usize = 512;
const DEFAULT_MODE: u32 = 0o644;
const EXECUTABLE_MASK: u32 = 0o111;

/// `setUint` of `package:tar`: octal, right-aligned, padded on the left
/// with spaces and ended by a space; zero is written as `0`.
fn set_uint(block: &mut [u8], value: u64, position: usize, length: usize) {
    block[position + length - 1] = b' ';
    let mut number = value;
    let mut explicit_zero = number == 0;
    for pos in (position..position + length - 1).rev() {
        if number != 0 {
            block[pos] = b'0' + (number & 7) as u8;
            number >>= 3;
        } else if explicit_zero {
            block[pos] = b'0';
            explicit_zero = false;
        } else {
            block[pos] = b' ';
        }
    }
}

fn header(name: &[u8], mode: u32, size: u64, modified: i64, flag: u8, owner: &str) -> [u8; BLOCK] {
    let mut block = [0u8; BLOCK];
    block[..name.len()].copy_from_slice(name);
    set_uint(&mut block, u64::from(mode), 100, 8);
    set_uint(&mut block, 0, 108, 8);
    set_uint(&mut block, 0, 116, 8);
    set_uint(&mut block, size, 124, 12);
    set_uint(&mut block, modified as u64, 136, 12);
    block[156] = flag;
    block[257..263].copy_from_slice(b"ustar\0");
    set_uint(&mut block, 0, 263, 2);
    block[265..265 + owner.len()].copy_from_slice(owner.as_bytes());
    block[297..297 + owner.len()].copy_from_slice(owner.as_bytes());
    block[148..156].fill(b' ');
    let checksum: u64 = block.iter().map(|&b| u64::from(b)).sum();
    set_uint(&mut block, checksum, 148, 8);
    block
}

fn pad(out: &mut Vec<u8>, size: usize) {
    let padding = (BLOCK - size % BLOCK) % BLOCK;
    out.resize(out.len() + padding, 0);
}

/// One entry: its header, with a GNU long-name entry before it when the
/// name does not fit in 99 bytes.
fn write_entry(out: &mut Vec<u8>, name: &str, mode: u32, size: u64, modified: i64, flag: u8) {
    let bytes = name.as_bytes();
    let short = if bytes.len() > 99 {
        out.extend_from_slice(&header(
            b"././@LongLink",
            0,
            bytes.len() as u64,
            0,
            b'L',
            "",
        ));
        out.extend_from_slice(bytes);
        pad(out, bytes.len());
        &bytes[..99]
    } else {
        bytes
    };
    out.extend_from_slice(&header(short, mode, size, modified, flag, "pub"));
}

/// The modes, size and change time of a file as `FileStat` reports them.
#[cfg(unix)]
fn stat(path: &str) -> std::io::Result<(u32, u64, i64)> {
    use std::os::unix::fs::MetadataExt;
    let m = std::fs::metadata(path)?;
    Ok((m.mode(), m.len(), m.ctime()))
}

#[cfg(not(unix))]
fn stat(path: &str) -> std::io::Result<(u32, u64, i64)> {
    let m = std::fs::metadata(path)?;
    // Dart reports 0o666 (0o444 when read-only) for files on Windows and
    // the last write as the change time.
    let mode = if m.permissions().readonly() {
        0o444
    } else {
        0o666
    };
    let changed = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64);
    Ok((mode, m.len(), changed))
}

/// The `.tar.gz` of `contents` (files and directories, as `listFiles`
/// returns them) relative to `base_dir`.
pub(crate) fn create_tar_gz(contents: &[String], base_dir: &str) -> Result<Vec<u8>, Failure> {
    let base = p::normalize(&p::absolute(base_dir));
    let mut tar = Vec::new();
    for entry in contents {
        let entry = p::normalize(&p::absolute(entry));
        if p::equals(&base, &entry) {
            continue;
        }
        if !p::is_within(&base, &entry) {
            return Err(format!("Invalid argument(s): Entry {entry} is not inside {base}.").into());
        }
        let name = p::split(&p::relative(&entry, &base)).join("/");
        let io = |e: std::io::Error| Failure {
            code: crate::pub_get::exit::NO_INPUT,
            message: format!(
                "FileSystemException: Cannot open file, path = '{entry}' ({})",
                super::files::os_error(&e)
            ),
        };
        let metadata = std::fs::metadata(&entry).map_err(io)?;
        if metadata.is_dir() {
            write_entry(&mut tar, &name, DEFAULT_MODE | EXECUTABLE_MASK, 0, 0, b'5');
        } else {
            let (mode, _, changed) = stat(&entry).map_err(io)?;
            let bytes = std::fs::read(&entry).map_err(io)?;
            write_entry(
                &mut tar,
                &name,
                DEFAULT_MODE | (mode & EXECUTABLE_MASK),
                bytes.len() as u64,
                changed,
                b'0',
            );
            tar.extend_from_slice(&bytes);
            pad(&mut tar, bytes.len());
        }
    }
    tar.resize(tar.len() + 2 * BLOCK, 0);
    Ok(gzip(&tar))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn octal_fields_follow_package_tar() {
        let mut block = [0u8; 16];
        set_uint(&mut block, 0o644, 0, 8);
        assert_eq!(&block[..8], b"    644 ");
        set_uint(&mut block, 0, 0, 8);
        assert_eq!(&block[..8], b"      0 ");
        set_uint(&mut block, 0, 8, 2);
        assert_eq!(&block[8..10], b"0 ");
    }

    #[test]
    fn long_names_get_a_gnu_entry() {
        let mut out = Vec::new();
        let name = "a/".repeat(60);
        write_entry(&mut out, &name, 0o644, 0, 0, b'0');
        assert_eq!(&out[..13], b"././@LongLink");
        assert_eq!(out[156], b'L');
        assert_eq!(&out[BLOCK..BLOCK + name.len()], name.as_bytes());
        assert_eq!(out.len(), 3 * BLOCK);
        assert_eq!(&out[2 * BLOCK..2 * BLOCK + 99], &name.as_bytes()[..99]);
    }
}
