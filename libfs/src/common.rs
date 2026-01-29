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
use std::fs::{File, FileTimes};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{fchown, MetadataExt};
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
                debug!("Copy xattr {attr:?}");
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
        warn!("Failed to copy xattrs from {infd:?}: {e}");
    }

    // FIXME: ACLs, selinux, etc.

    let inmeta = infd.metadata()?;

    debug!("Performing permissions copy");
    outfd.set_permissions(inmeta.permissions())?;

    Ok(())
}

/// Copy file timestamps.
pub fn copy_timestamps(infd: &File, outfd: &File) -> Result<()> {
    let inmeta = infd.metadata()?;

    debug!("Performing timestamp copy");
    let ftime = FileTimes::new()
        .set_accessed(inmeta.accessed()?)
        .set_modified(inmeta.modified()?);
    outfd.set_times(ftime)?;

    Ok(())
}

pub fn copy_owner(infd: &File, outfd: &File) -> Result<()> {
    let inmeta = infd.metadata()?;
    fchown(outfd, Some(inmeta.uid()), Some(inmeta.gid()))?;

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
