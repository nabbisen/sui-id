//! Minimal hand-rolled ustar tar writer and reader (no runtime dep on tar crate).

use anyhow::{Context, Result, bail};
use std::io::Write;

// ---------- minimal POSIX ustar tar writer / reader ----------------------
// The `tar` crate is a perfectly good dependency, but for two files we can
// stay zero-dep and keep the audit surface small. Format reference:
// https://www.gnu.org/software/tar/manual/html_node/Standard.html

const BLOCK: usize = 512;

pub(crate) fn write_tar_entry<W: Write>(out: &mut W, name: &str, bytes: &[u8]) -> Result<()> {
    if name.len() >= 100 {
        bail!("tar entry name too long: {name}");
    }
    let mut header = [0u8; BLOCK];
    // name (offset 0, 100 bytes)
    header[..name.len()].copy_from_slice(name.as_bytes());
    // mode (100, 8 bytes, octal ASCII, NUL-terminated). 0600.
    write_octal(&mut header[100..108], 0o600);
    // uid, gid (108, 8 bytes each). 0.
    write_octal(&mut header[108..116], 0);
    write_octal(&mut header[116..124], 0);
    // size (124, 12 bytes octal)
    write_octal(&mut header[124..136], bytes.len() as u64);
    // mtime (136, 12 bytes octal) — 0 is acceptable for an archive.
    write_octal(&mut header[136..148], 0);
    // chksum (148, 8 bytes) — fill with spaces for the checksum
    // computation, then overwrite with the result.
    for b in &mut header[148..156] {
        *b = b' ';
    }
    // typeflag (156, 1 byte) — '0' = regular file.
    header[156] = b'0';
    // linkname (157, 100 bytes) zero-filled.
    // magic (257, 6) "ustar\0"
    header[257..263].copy_from_slice(b"ustar\0");
    // version (263, 2)
    header[263..265].copy_from_slice(b"00");
    // uname/gname (265, 32 each) — leave empty.
    // devmajor/devminor (329, 8 each) — 0.
    write_octal(&mut header[329..337], 0);
    write_octal(&mut header[337..345], 0);

    let chksum: u32 = header.iter().map(|&b| b as u32).sum();
    // 6 octal digits, NUL, space.
    let s = format!("{chksum:06o}\0 ");
    let bytes_chk = s.as_bytes();
    header[148..148 + bytes_chk.len()].copy_from_slice(bytes_chk);

    out.write_all(&header)?;
    out.write_all(bytes)?;
    let pad = (BLOCK - (bytes.len() % BLOCK)) % BLOCK;
    if pad > 0 {
        out.write_all(&vec![0u8; pad])?;
    }
    Ok(())
}

pub(crate) fn write_tar_terminator<W: Write>(out: &mut W) -> Result<()> {
    out.write_all(&[0u8; BLOCK * 2])?;
    Ok(())
}

pub(crate) fn write_octal(buf: &mut [u8], mut value: u64) {
    let n = buf.len();
    // We write `n-1` octal digits, NUL-terminated.
    for i in (0..n - 1).rev() {
        buf[i] = b'0' + (value & 0o7) as u8;
        value >>= 3;
    }
    buf[n - 1] = 0;
}

pub(crate) fn read_tar(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
    let mut out = Vec::new();
    let mut idx = 0;
    while idx + BLOCK <= bytes.len() {
        let header = &bytes[idx..idx + BLOCK];
        if header.iter().all(|&b| b == 0) {
            break;
        }
        let name_end = header[..100].iter().position(|&b| b == 0).unwrap_or(100);
        let name = std::str::from_utf8(&header[..name_end])
            .context("tar entry name is not UTF-8")?
            .to_owned();
        let size = read_octal(&header[124..136])?;
        idx += BLOCK;
        if idx + (size as usize) > bytes.len() {
            bail!("truncated tar entry for {name}");
        }
        let body = bytes[idx..idx + size as usize].to_vec();
        out.push((name, body));
        // Advance past the rounded-up data area.
        let padded = ((size as usize) + BLOCK - 1) / BLOCK * BLOCK;
        idx += padded;
    }
    if out.is_empty() {
        bail!("tar archive contains no entries");
    }
    Ok(out)
}

pub(crate) fn read_octal(buf: &[u8]) -> Result<u64> {
    let mut v = 0u64;
    for &b in buf {
        if b == 0 || b == b' ' {
            break;
        }
        if !(b'0'..=b'7').contains(&b) {
            bail!("invalid octal digit in tar header");
        }
        v = v * 8 + (b - b'0') as u64;
    }
    Ok(v)
}
