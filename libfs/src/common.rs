/*
 * Copyright © 2018, Steve Smith <tarkasteve@gmail.com>
 *
 * This program is free software: you can redistribute it and/or
 * modify it under the terms of the GNU General Public License version
 * 3 as published by the Free Software Foundation.
 *
 * This program is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */


use log::{debug, warn};
use rustix::fs::{fsync, ftruncate};
use rustix::io::{pread, pwrite};
use std::cmp;
use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use xattr::FileExt;

use crate::errors::{Result, Error};
use crate::{Extent, XATTR_SUPPORTED, copy_sparse, probably_sparse, copy_file_bytes};

fn copy_xattr(infd: &File, outfd: &File) -> Result<()> {
    // FIXME: Flag for xattr.
    if XATTR_SUPPORTED {
        debug!("Starting xattr copy...");
        for attr in infd.list_xattr()? {
            if let Some(val) = infd.get_xattr(&attr)? {
                debug!("Copy xattr {:?}", attr);
                outfd.set_xattr(attr, val.as_slice())?;
            }
        }
    }
    Ok(())
}

/// Copy file permissions. Will also copy
/// [xattr](https://man7.org/linux/man-pages/man7/xattr.7.html)'s if
/// possible.
pub fn copy_permissions(infd: &File, outfd: &File) -> Result<()> {
    let xr = copy_xattr(infd, outfd);
    if let Err(e) = xr {
        // FIXME: We don't have a way of detecting if the
        // target FS supports XAttr, so assume any error is
        // "Unsupported" for now.
        warn!("Failed to copy xattrs from {:?}: {}", infd, e);
    }

    // FIXME: ACLs, selinux, etc.

    debug!("Performing permissions copy");
    outfd.set_permissions(infd.metadata()?.permissions())?;

    debug!("Permissions copy done");
    Ok(())
}

pub(crate) fn read_bytes(fd: &File, buf: &mut [u8], off: usize) -> Result<usize> {
    Ok(pread(fd, buf, off as u64)?)
}

pub(crate) fn write_bytes(fd: &File, buf: &mut [u8], off: usize) -> Result<usize> {
    Ok(pwrite(fd, buf, off as u64)?)
}

/// Copy a block of bytes at an offset between files. Uses Posix pread/pwrite.
pub(crate) fn copy_range_uspace(reader: &File, writer: &File, nbytes: usize, off: usize) -> Result<usize> {
    // FIXME: For larger buffers we should use a pre-allocated thread-local?
    let mut buf = vec![0; nbytes];

    let mut written: usize = 0;
    while written < nbytes {
        let next = cmp::min(nbytes - written, nbytes);
        let noff = off + written;

        let rlen = match read_bytes(reader, &mut buf[..next], noff) {
            Ok(0) => return Err(Error::InvalidSource("Source file ended prematurely.")),
            Ok(len) => len,
            Err(e) => return Err(e),
        };

        let _wlen = match write_bytes(writer, &mut buf[..rlen], noff) {
            Ok(len) if len < rlen => {
                return Err(Error::InvalidSource("Failed write to file."))
            }
            Ok(len) => len,
            Err(e) => return Err(e),
        };

        written += rlen;
    }
    Ok(written)
}

/// Slightly modified version of io::copy() that only copies a set amount of bytes.
pub(crate) fn copy_bytes_uspace(mut reader: &File, mut writer: &File, nbytes: usize) -> Result<usize> {
    let mut buf = vec![0; nbytes];

    let mut written = 0;
    while written < nbytes {
        let next = cmp::min(nbytes - written, nbytes);
        let len = match reader.read(&mut buf[..next]) {
            Ok(0) => return Err(Error::InvalidSource("Source file ended prematurely.")),
            Ok(len) => len,
            Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into())
        };
        writer.write_all(&buf[..len])?;
        written += len;
    }
    Ok(written)
}

/// Allocate file space on disk. Uses Posix ftruncate().
pub fn allocate_file(fd: &File, len: u64) -> Result<()> {
    Ok(ftruncate(fd, len)?)
}

/// Merge any contiguous extents in a list. See [merge_extents].
pub fn merge_extents(extents: Vec<Extent>) -> Result<Vec<Extent>> {
    let mut merged: Vec<Extent> = vec![];

    let mut prev: Option<Extent> = None;
    for e in extents {
        match prev {
            Some(p) => {
                if e.start == p.end + 1 {
                    // Current & prev are contiguous, merge & see what
                    // comes next.
                    prev = Some(Extent {
                        start: p.start,
                        end: e.end,
                        shared: p.shared & e.shared,
                    });
                } else {
                    merged.push(p);
                    prev = Some(e);
                }
            }
            // First iter
            None => prev = Some(e),
        }
    }
    if let Some(p) = prev {
        merged.push(p);
    }

    Ok(merged)
}


/// Determine if two files are the same by examining their inodes.
pub fn is_same_file(src: &Path, dest: &Path) -> Result<bool> {
    let sstat = src.metadata()?;
    let dstat = dest.metadata()?;
    let same = (sstat.ino() == dstat.ino())
        && (sstat.dev() == dstat.dev());

    Ok(same)
}

/// Copy a file. This differs from [std::fs::copy] in that it looks
/// for sparse blocks and skips them.
pub fn copy_file(from: &Path, to: &Path) -> Result<u64> {
    let infd = File::open(from)?;
    let len = infd.metadata()?.len();

    let outfd = File::create(to)?;
    allocate_file(&outfd, len)?;

    let total = if probably_sparse(&infd)? {
        copy_sparse(&infd, &outfd)?
    } else {
        copy_file_bytes(&infd, &outfd, len)? as u64
    };

    Ok(total)
}

/// Sync an open file to disk. Uses `fsync(2)`.
pub fn sync(fd: &File) -> Result<()> {
    Ok(fsync(fd)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::read;
    use std::ops::Range;
    use tempfile::tempdir;

    impl From<Range<u64>> for Extent {
        fn from(r: Range<u64>) -> Self {
            Extent {
                start: r.start,
                end: r.end,
                shared: false,
            }
        }
    }

    #[test]
    fn test_copy_bytes_uspace_large() {
        let dir = tempdir().unwrap();
        let from = dir.path().join("from.bin");
        let to = dir.path().join("to.bin");
        let size = 128 * 1024;
        let data = "X".repeat(size);

        {
            let mut fd: File = File::create(&from).unwrap();
            write!(fd, "{}", data).unwrap();
        }

        {
            let infd = File::open(&from).unwrap();
            let outfd = File::create(&to).unwrap();
            let written = copy_bytes_uspace(&infd, &outfd, size).unwrap();

            assert_eq!(written, size);
        }

        assert_eq!(from.metadata().unwrap().len(), to.metadata().unwrap().len());

        {
            let from_data = read(&from).unwrap();
            let to_data = read(&to).unwrap();
            assert_eq!(from_data, to_data);
        }
    }

    #[test]
    fn test_copy_range_uspace_large() {
        let dir = tempdir().unwrap();
        let from = dir.path().join("from.bin");
        let to = dir.path().join("to.bin");
        let size = 128 * 1024;
        let data = "X".repeat(size);

        {
            let mut fd: File = File::create(&from).unwrap();
            write!(fd, "{}", data).unwrap();
        }

        {
            let infd = File::open(&from).unwrap();
            let outfd = File::create(&to).unwrap();

            let blocksize = size / 4;
            let mut written = 0;

            for off in (0..4).rev() {
                written += copy_range_uspace(&infd, &outfd, blocksize, blocksize * off).unwrap();
            }

            assert_eq!(written, size);
        }

        assert_eq!(from.metadata().unwrap().len(), to.metadata().unwrap().len());

        {
            let from_data = read(&from).unwrap();
            let to_data = read(&to).unwrap();
            assert_eq!(from_data, to_data);
        }
    }

    #[test]
    fn test_extent_merge() -> Result<()> {
        assert_eq!(merge_extents(vec!())?, vec!());
        assert_eq!(merge_extents(
            vec!((0..1).into()))?,
            vec!((0..1).into()));

        assert_eq!(merge_extents(
            vec!((0..1).into(),
                (10..20).into()))?,
            vec!((0..1).into(),
                (10..20).into()));
        assert_eq!(merge_extents(
            vec!((0..10).into(),
                (11..20).into()))?,
            vec!((0..20).into()));
        assert_eq!(
            merge_extents(
                vec!((0..5).into(),
                    (11..20).into(),
                    (21..30).into(),
                    (40..50).into()))?,
            vec!((0..5).into(),
                (11..30).into(),
                (40..50).into())
        );
        assert_eq!(
            merge_extents(vec!((0..5).into(),
                (11..20).into(),
                (21..30).into(),
                (40..50).into(),
                (51..60).into()))?,
            vec!((0..5).into(),
                (11..30).into(),
                (40..60).into())
        );
        assert_eq!(
            merge_extents(
                vec!((0..10).into(),
                    (11..20).into(),
                    (21..30).into(),
                    (31..50).into(),
                    (51..60).into()))?,
            vec!((0..60).into())
        );
        Ok(())
    }


    #[test]
    fn test_copy_file() -> Result<()> {
        let dir = tempdir()?;
        let from = dir.path().join("file.bin");
        let len = 32 * 1024 * 1024;

        {
            let mut fd = File::create(&from)?;
            let data = "X".repeat(len);
            write!(fd, "{}", data).unwrap();
        }

        assert_eq!(len, from.metadata()?.len() as usize);

        let to = dir.path().join("sparse.copy.bin");
        crate::copy_file(&from, &to)?;

        assert_eq!(len, to.metadata()?.len() as usize);

        Ok(())
    }
}
